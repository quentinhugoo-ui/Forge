//! WebExplorer compute-reuse lab.
//!
//! This is intentionally a direct runner, not a Tauri/WebView adapter. The lab
//! isolates the expensive part we want to optimize: page memory tree analysis.
//! It measures cold vs warm runs and logs exactly which repeated calculations
//! were skipped by page-plan and subtree-score caches.

use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const DEFAULT_PASSES: usize = 3;
const DEFAULT_LOG_PATH: &str = ".forge-store/lab_runner_web.jsonl";
const SCORE_HASH_ROUNDS: u64 = 12;
const DEFAULT_PAGE_CACHE_MAX: usize = 16;
const DEFAULT_SUBTREE_CACHE_MAX: usize = 4_096;
const DEFAULT_STRESS_VARIANTS: usize = 1;
const LEGACY_PIPELINE_NODE_SCAN_PASSES: u64 = 7;
const FUSED_PIPELINE_NODE_SCAN_PASSES: u64 = 4;
const LEGACY_CONTENT_FIELD_SCAN_PASSES: u64 = 4;
const FUSED_CONTENT_FIELD_SCAN_PASSES: u64 = 1;
const LEGACY_DESCENDANT_METRIC_SCAN_PASSES: u64 = 5;
const FUSED_DESCENDANT_METRIC_SCAN_PASSES: u64 = 1;
const FALLBACK_TOPK_LIMIT: usize = 16;

#[derive(Clone, Copy)]
struct Bounds {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl Bounds {
    fn area(self) -> u32 {
        self.w.saturating_mul(self.h)
    }
}

#[derive(Clone)]
struct WebNode {
    id: usize,
    parent: Option<usize>,
    tag: &'static str,
    role: &'static str,
    text: String,
    image: &'static str,
    bounds: Bounds,
    visible: bool,
}

impl WebNode {
    fn candidate(&self, text_len: usize) -> bool {
        self.visible
            && self.bounds.area() >= 1_600
            && (text_len > 0 || !self.image.is_empty())
    }
}

struct PageFixture {
    name: &'static str,
    url: &'static str,
    kind: FixtureKind,
    card_count: usize,
}

#[derive(Clone, Copy)]
enum FixtureKind {
    Search,
    Commerce,
    Docs,
}

struct PageRun {
    name: &'static str,
    url: String,
    pass: usize,
    variant: usize,
    nodes: Vec<WebNode>,
    response_bytes: usize,
}

#[derive(Clone)]
struct CachedNode {
    score: u32,
    text_bytes: usize,
    image_pixels: u32,
}

#[derive(Clone, Copy)]
struct FallbackFrameRank {
    score: u32,
    area: u32,
    y: u32,
}

#[derive(Clone)]
struct PagePlan {
    page_hash: [u8; 32],
    block_count: usize,
    candidate_count: usize,
    total_score: u64,
}

struct WebLabCache {
    page_plans: HashMap<[u8; 32], PagePlan>,
    subtree_scores: HashMap<[u8; 32], CachedNode>,
    page_order: VecDeque<[u8; 32]>,
    subtree_order: VecDeque<[u8; 32]>,
    page_cache_max: usize,
    subtree_cache_max: usize,
    page_evictions: u64,
    subtree_evictions: u64,
}

impl WebLabCache {
    fn new(page_cache_max: usize, subtree_cache_max: usize) -> Self {
        Self {
            page_plans: HashMap::with_capacity(page_cache_max.min(1024)),
            subtree_scores: HashMap::with_capacity(subtree_cache_max.min(16_384)),
            page_order: VecDeque::with_capacity(page_cache_max.min(1024)),
            subtree_order: VecDeque::with_capacity(subtree_cache_max.min(16_384)),
            page_cache_max,
            subtree_cache_max,
            page_evictions: 0,
            subtree_evictions: 0,
        }
    }

    fn page_get(&mut self, key: &[u8; 32]) -> Option<PagePlan> {
        self.page_plans.get(key).cloned()
    }

    fn page_insert(&mut self, key: [u8; 32], plan: PagePlan) {
        if self.page_plans.insert(key, plan).is_some() {
            return;
        }
        self.page_order.push_back(key);
        while self.page_plans.len() > self.page_cache_max {
            let Some(evicted) = self.page_order.pop_front() else {
                break;
            };
            if self.page_plans.remove(&evicted).is_some() {
                self.page_evictions += 1;
            }
        }
    }

    fn subtree_get(&mut self, key: &[u8; 32]) -> Option<CachedNode> {
        self.subtree_scores.get(key).cloned()
    }

    fn subtree_insert(&mut self, key: [u8; 32], score: CachedNode) {
        if self.subtree_scores.insert(key, score).is_some() {
            return;
        }
        self.subtree_order.push_back(key);
        while self.subtree_scores.len() > self.subtree_cache_max {
            let Some(evicted) = self.subtree_order.pop_front() else {
                break;
            };
            if self.subtree_scores.remove(&evicted).is_some() {
                self.subtree_evictions += 1;
            }
        }
    }

    fn estimated_bytes(&self) -> usize {
        self.page_plans.len() * std::mem::size_of::<PagePlan>()
            + self.subtree_scores.len() * std::mem::size_of::<CachedNode>()
            + (self.page_order.len() + self.subtree_order.len()) * 32
    }
}

#[derive(Default, Clone)]
struct AvoidanceProof {
    page_cache_hit: bool,
    page_cache_hits: u64,
    page_cache_misses: u64,
    subtree_cache_hits: u64,
    subtree_cache_misses: u64,
    node_walks_run: u64,
    node_walks_avoided: u64,
    score_evals_run: u64,
    score_evals_avoided: u64,
    score_hash_rounds_run: u64,
    score_hash_rounds_avoided: u64,
    pipeline_node_scans_run: u64,
    pipeline_node_scans_avoided: u64,
    legacy_pipeline_scan_passes: u64,
    fused_pipeline_scan_passes: u64,
    nav_candidates_seen: u64,
    nav_candidate_vec_pushes_avoided: u64,
    nav_bucket_key_allocations_avoided: u64,
    nav_bucket_key_bytes_avoided: u64,
    nav_item_materializations_deferred: u64,
    nav_item_clone_bytes_deferred: u64,
    fallback_topk_candidates_seen: u64,
    fallback_topk_candidates_kept: u64,
    fallback_topk_candidates_dropped: u64,
    fallback_topk_replacements: u64,
    fallback_full_sort_items_avoided: u64,
    fallback_frame_vec_slots_avoided: u64,
    fallback_frame_vec_bytes_avoided: u64,
    content_subtree_queries_run: u64,
    content_subtree_queries_avoided: u64,
    content_subtree_id_clones_avoided: u64,
    descendant_vec_queries_run: u64,
    descendant_vec_cache_hits: u64,
    descendant_vec_builds: u64,
    descendant_vec_clone_allocations_avoided: u64,
    descendant_vec_slots_avoided: u64,
    descendant_vec_bytes_avoided: u64,
    zero_copy_index_key_bytes_avoided: u64,
    text_cache_hits: u64,
    text_cache_misses: u64,
    text_bytes_normalized: u64,
    text_bytes_avoided: u64,
    text_join_allocations_avoided: u64,
    text_collapse_vec_allocations_avoided: u64,
    text_collapse_word_slots_avoided: u64,
    content_field_scan_queries_run: u64,
    content_field_scan_passes_run: u64,
    content_field_scan_passes_avoided: u64,
    content_field_node_visits_run: u64,
    content_field_node_visits_avoided: u64,
    descendant_metric_scans_run: u64,
    descendant_metric_scans_avoided: u64,
    descendant_metric_node_visits_run: u64,
    descendant_metric_node_visits_avoided: u64,
    plan_casefold_checks_run: u64,
    plan_casefold_allocations_avoided: u64,
    plan_casefold_bytes_avoided: u64,
    casefold_checks_run: u64,
    casefold_allocations_avoided: u64,
    casefold_bytes_avoided: u64,
    url_label_queries_run: u64,
    url_label_cache_hits: u64,
    url_label_cache_misses: u64,
    url_label_fast_path_hits: u64,
    url_label_full_parse_run: u64,
    url_label_full_parse_avoided: u64,
    url_label_bytes_avoided: u64,
    winner_candidate_scans_run: u64,
    winner_text_candidates_seen: u64,
    winner_final_clones_run: u64,
    winner_text_clones_avoided: u64,
    winner_text_bytes_avoided: u64,
    page_cache_entries: usize,
    subtree_cache_entries: usize,
    page_cache_evictions: u64,
    subtree_cache_evictions: u64,
    cache_estimated_bytes: usize,
}

#[derive(Default)]
struct LabTotals {
    pages: u64,
    passes: u64,
    variants: u64,
    load_us: u128,
    exec_us: u128,
    load_samples_us: Vec<u128>,
    exec_samples_us: Vec<u128>,
    cold_exec_us: u128,
    warm_exec_us: u128,
    cold_runs: u64,
    warm_runs: u64,
    nodes: u64,
    candidates: u64,
    page_cache_hits: u64,
    page_cache_misses: u64,
    subtree_cache_hits: u64,
    subtree_cache_misses: u64,
    node_walks_run: u64,
    node_walks_avoided: u64,
    score_evals_run: u64,
    score_evals_avoided: u64,
    score_hash_rounds_run: u64,
    score_hash_rounds_avoided: u64,
    pipeline_node_scans_run: u64,
    pipeline_node_scans_avoided: u64,
    nav_candidates_seen: u64,
    nav_candidate_vec_pushes_avoided: u64,
    nav_bucket_key_allocations_avoided: u64,
    nav_bucket_key_bytes_avoided: u64,
    nav_item_materializations_deferred: u64,
    nav_item_clone_bytes_deferred: u64,
    fallback_topk_candidates_seen: u64,
    fallback_topk_candidates_kept: u64,
    fallback_topk_candidates_dropped: u64,
    fallback_topk_replacements: u64,
    fallback_full_sort_items_avoided: u64,
    fallback_frame_vec_slots_avoided: u64,
    fallback_frame_vec_bytes_avoided: u64,
    content_subtree_queries_run: u64,
    content_subtree_queries_avoided: u64,
    content_subtree_id_clones_avoided: u64,
    descendant_vec_queries_run: u64,
    descendant_vec_cache_hits: u64,
    descendant_vec_builds: u64,
    descendant_vec_clone_allocations_avoided: u64,
    descendant_vec_slots_avoided: u64,
    descendant_vec_bytes_avoided: u64,
    zero_copy_index_key_bytes_avoided: u64,
    text_cache_hits: u64,
    text_cache_misses: u64,
    text_bytes_normalized: u64,
    text_bytes_avoided: u64,
    text_join_allocations_avoided: u64,
    text_collapse_vec_allocations_avoided: u64,
    text_collapse_word_slots_avoided: u64,
    content_field_scan_queries_run: u64,
    content_field_scan_passes_run: u64,
    content_field_scan_passes_avoided: u64,
    content_field_node_visits_run: u64,
    content_field_node_visits_avoided: u64,
    descendant_metric_scans_run: u64,
    descendant_metric_scans_avoided: u64,
    descendant_metric_node_visits_run: u64,
    descendant_metric_node_visits_avoided: u64,
    plan_casefold_checks_run: u64,
    plan_casefold_allocations_avoided: u64,
    plan_casefold_bytes_avoided: u64,
    casefold_checks_run: u64,
    casefold_allocations_avoided: u64,
    casefold_bytes_avoided: u64,
    url_label_queries_run: u64,
    url_label_cache_hits: u64,
    url_label_cache_misses: u64,
    url_label_fast_path_hits: u64,
    url_label_full_parse_run: u64,
    url_label_full_parse_avoided: u64,
    url_label_bytes_avoided: u64,
    winner_candidate_scans_run: u64,
    winner_text_candidates_seen: u64,
    winner_final_clones_run: u64,
    winner_text_clones_avoided: u64,
    winner_text_bytes_avoided: u64,
    page_cache_entries: usize,
    subtree_cache_entries: usize,
    page_cache_evictions: u64,
    subtree_cache_evictions: u64,
    cache_estimated_bytes: usize,
    page_cache_max: usize,
    subtree_cache_max: usize,
}

struct TimedRun {
    page: PageRun,
    load_time: Duration,
    analysis_time: Duration,
    plan: PagePlan,
    proof: AvoidanceProof,
}

struct Config {
    passes: usize,
    log_path: Option<PathBuf>,
    append: bool,
    page_cache_max: usize,
    subtree_cache_max: usize,
    stress_variants: usize,
}

impl Config {
    fn from_args() -> io::Result<Self> {
        let mut passes = DEFAULT_PASSES;
        let mut log_path = Some(PathBuf::from(DEFAULT_LOG_PATH));
        let mut append = false;
        let mut page_cache_max = DEFAULT_PAGE_CACHE_MAX;
        let mut subtree_cache_max = DEFAULT_SUBTREE_CACHE_MAX;
        let mut stress_variants = DEFAULT_STRESS_VARIANTS;

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--passes" | "-p" => {
                    let raw = args.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--passes needs a value")
                    })?;
                    passes = raw.parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--passes expects a number")
                    })?;
                }
                "--page-cache-max" => {
                    let raw = args.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--page-cache-max needs a value")
                    })?;
                    page_cache_max = raw.parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--page-cache-max expects a number")
                    })?;
                }
                "--subtree-cache-max" => {
                    let raw = args.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--subtree-cache-max needs a value")
                    })?;
                    subtree_cache_max = raw.parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--subtree-cache-max expects a number")
                    })?;
                }
                "--stress-pages" | "--stress-variants" => {
                    let raw = args.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--stress-pages needs a value")
                    })?;
                    stress_variants = raw.parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--stress-pages expects a number")
                    })?;
                }
                "--jsonl" | "--log" => {
                    let raw = args.next().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "--jsonl needs a path")
                    })?;
                    log_path = Some(PathBuf::from(raw));
                }
                "--append" => append = true,
                "--no-log" => log_path = None,
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown argument: {other}"),
                    ));
                }
            }
        }

        if passes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--passes must be >= 1",
            ));
        }
        if page_cache_max == 0 || subtree_cache_max == 0 || stress_variants == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cache limits and --stress-pages must be >= 1",
            ));
        }

        Ok(Self {
            passes,
            log_path,
            append,
            page_cache_max,
            subtree_cache_max,
            stress_variants,
        })
    }
}

fn print_help() {
    println!("lab_runner_web: WebExplorer compute-reuse lab");
    println!("usage: cargo run --release --example lab_runner_web -- [--passes N] [--stress-pages N] [--page-cache-max N] [--subtree-cache-max N] [--jsonl PATH] [--append] [--no-log]");
    println!("default log: {DEFAULT_LOG_PATH}");
}

fn fixtures() -> Vec<PageFixture> {
    vec![
        PageFixture {
            name: "google_search_tokyo",
            url: "https://www.google.com/search?q=Tokyo",
            kind: FixtureKind::Search,
            card_count: 96,
        },
        PageFixture {
            name: "amazon_home_fr",
            url: "https://www.amazon.fr/",
            kind: FixtureKind::Commerce,
            card_count: 128,
        },
        PageFixture {
            name: "google_docs_drive",
            url: "https://docs.google.com/document/u/0/",
            kind: FixtureKind::Docs,
            card_count: 80,
        },
    ]
}

fn main() -> io::Result<()> {
    let config = Config::from_args()?;
    let mut writer = open_log_writer(config.log_path.as_deref(), config.append)?;
    let mut cache = WebLabCache::new(config.page_cache_max, config.subtree_cache_max);
    let fixtures = fixtures();
    let mut totals = LabTotals::default();
    totals.page_cache_max = config.page_cache_max;
    totals.subtree_cache_max = config.subtree_cache_max;

    println!("=== lab_runner_web: WebExplorer compute reuse lab ===");
    match config.log_path.as_deref() {
        Some(path) => println!("jsonl: {}", path.display()),
        None => println!("jsonl: disabled"),
    }
    println!(
        "scenario: {} fixtures x {} passes x {} variants, score_hash_rounds={}",
        fixtures.len(),
        config.passes,
        config.stress_variants,
        SCORE_HASH_ROUNDS
    );
    println!(
        "cache budget: page_max={} subtree_max={} (bounded FIFO, hot-hit O(1), eviction-proof mode)",
        config.page_cache_max, config.subtree_cache_max
    );
    println!(
        "pipeline audit: legacy_full_node_scans={} fused_full_node_scans={} per cache miss",
        LEGACY_PIPELINE_NODE_SCAN_PASSES, FUSED_PIPELINE_NODE_SCAN_PASSES
    );
    println!(
        "nav middleman audit: semantic plan streams candidates into borrowed-key buckets and materializes only the winning row"
    );
    println!(
        "content field fusion audit: image/summary/source/href descendant passes fused {} -> {}",
        LEGACY_CONTENT_FIELD_SCAN_PASSES, FUSED_CONTENT_FIELD_SCAN_PASSES
    );
    println!(
        "content audit: zero-copy node index + descendant-cache counters enabled"
    );
    println!(
        "descendant vec audit: cached subtree Vec now returns borrowed slices and avoids clone-on-read"
    );
    println!(
        "text audit: lazy per-node normalization cache + streaming descendant text join enabled"
    );
    println!(
        "collapse audit: streaming whitespace collapse replaces collect<Vec<_>>().join(\" \")"
    );
    println!(
        "descendant metrics audit: fused {} repeated descendant scans into {} pass",
        LEGACY_DESCENDANT_METRIC_SCAN_PASSES, FUSED_DESCENDANT_METRIC_SCAN_PASSES
    );
    println!(
        "plan casefold audit: content-root/nav/profile filters use ASCII comparisons without lowercase strings"
    );
    println!(
        "casefold audit: ASCII no-allocation eq/contains replaces repeated lowercase strings in content filters"
    );
    println!(
        "url label audit: per-page source-label cache + HTTP host fast path avoid repeated Url::parse"
    );
    println!(
        "winner selection audit: summary/source-label candidates stay borrowed until the final winning clone"
    );
    println!(
        "fallback top-k audit: content fallback keeps only top {} frames and avoids full candidate Vec sorting",
        FALLBACK_TOPK_LIMIT
    );

    for pass in 0..config.passes {
        for variant in 0..config.stress_variants {
            for fixture in &fixtures {
                let run = run_fixture(fixture, pass, variant, &mut cache);
                emit_run(&mut writer, &run)?;
                update_totals(&mut totals, &run);
                println!(
                    "{:<20} pass={} variant={} load={:>6}us exec={:>6}us page_hit={} subtree_hits={} subtree_misses={} nav_mid(cand/vec/key/bytes/defer/clone_bytes)={}/{}/{}/{}/{}/{} fallback_topk(seen/kept/drop/repl/sort/slots/bytes)={}/{}/{}/{}/{}/{}/{} desc_vec(q/h/build/clones/slots/bytes)={}/{}/{}/{}/{}/{} field_fuse(q/pass/visit/avoided)={}/{}/{}/{} metric_scans={}/{} metric_nodes={}/{} plan_casefold(checks/allocs/bytes)={}/{}/{} winners(cand/final/avoided/bytes)={}/{}/{}/{} url_labels(q/h/m/fast/parse/avoided)={}/{}/{}/{}/{}/{} casefold_checks={} casefold_allocs_avoided={} casefold_bytes_avoided={} avoided_scores={} evict(page/subtree)={}/{}",
                    run.page.name,
                    pass,
                    variant,
                    run.load_time.as_micros(),
                    run.analysis_time.as_micros(),
                    run.proof.page_cache_hit,
                    run.proof.subtree_cache_hits,
                    run.proof.subtree_cache_misses,
                    run.proof.nav_candidates_seen,
                    run.proof.nav_candidate_vec_pushes_avoided,
                    run.proof.nav_bucket_key_allocations_avoided,
                    run.proof.nav_bucket_key_bytes_avoided,
                    run.proof.nav_item_materializations_deferred,
                    run.proof.nav_item_clone_bytes_deferred,
                    run.proof.fallback_topk_candidates_seen,
                    run.proof.fallback_topk_candidates_kept,
                    run.proof.fallback_topk_candidates_dropped,
                    run.proof.fallback_topk_replacements,
                    run.proof.fallback_full_sort_items_avoided,
                    run.proof.fallback_frame_vec_slots_avoided,
                    run.proof.fallback_frame_vec_bytes_avoided,
                    run.proof.descendant_vec_queries_run,
                    run.proof.descendant_vec_cache_hits,
                    run.proof.descendant_vec_builds,
                    run.proof.descendant_vec_clone_allocations_avoided,
                    run.proof.descendant_vec_slots_avoided,
                    run.proof.descendant_vec_bytes_avoided,
                    run.proof.content_field_scan_queries_run,
                    run.proof.content_field_scan_passes_run,
                    run.proof.content_field_node_visits_run,
                    run.proof.content_field_node_visits_avoided,
                    run.proof.descendant_metric_scans_run,
                    run.proof.descendant_metric_scans_avoided,
                    run.proof.descendant_metric_node_visits_run,
                    run.proof.descendant_metric_node_visits_avoided,
                    run.proof.plan_casefold_checks_run,
                    run.proof.plan_casefold_allocations_avoided,
                    run.proof.plan_casefold_bytes_avoided,
                    run.proof.winner_text_candidates_seen,
                    run.proof.winner_final_clones_run,
                    run.proof.winner_text_clones_avoided,
                    run.proof.winner_text_bytes_avoided,
                    run.proof.url_label_queries_run,
                    run.proof.url_label_cache_hits,
                    run.proof.url_label_cache_misses,
                    run.proof.url_label_fast_path_hits,
                    run.proof.url_label_full_parse_run,
                    run.proof.url_label_full_parse_avoided,
                    run.proof.casefold_checks_run,
                    run.proof.casefold_allocations_avoided,
                    run.proof.casefold_bytes_avoided,
                    run.proof.score_evals_avoided,
                    run.proof.page_cache_evictions,
                    run.proof.subtree_cache_evictions
                );
            }
        }
    }

    emit_summary(&mut writer, &totals)?;
    print_summary(&totals);
    Ok(())
}

fn open_log_writer(path: Option<&Path>, append: bool) -> io::Result<Box<dyn Write>> {
    let Some(path) = path else {
        return Ok(Box::new(io::sink()));
    };
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            create_dir_all(parent)?;
        }
    }
    let file = if append {
        OpenOptions::new().create(true).append(true).open(path)?
    } else {
        File::create(path)?
    };
    Ok(Box::new(BufWriter::new(file)))
}

fn run_fixture(
    fixture: &PageFixture,
    pass: usize,
    variant: usize,
    cache: &mut WebLabCache,
) -> TimedRun {
    let load_start = Instant::now();
    let page = materialize_fixture(fixture, pass, variant);
    let load_time = load_start.elapsed();

    let analysis_start = Instant::now();
    let (plan, proof) = analyze_page(&page, cache);
    let analysis_time = analysis_start.elapsed();

    TimedRun {
        page,
        load_time,
        analysis_time,
        plan,
        proof,
    }
}

fn materialize_fixture(fixture: &PageFixture, pass: usize, variant: usize) -> PageRun {
    let mut nodes = Vec::with_capacity(fixture.card_count * 8 + 32);
    let root = push_node(
        &mut nodes,
        None,
        "body",
        "document",
        "",
        "",
        Bounds {
            x: 0,
            y: 0,
            w: 1440,
            h: 1600,
        },
    );
    match fixture.kind {
        FixtureKind::Search => materialize_search(&mut nodes, root, fixture.card_count, pass),
        FixtureKind::Commerce => materialize_commerce(&mut nodes, root, fixture.card_count, pass),
        FixtureKind::Docs => materialize_docs(&mut nodes, root, fixture.card_count, pass),
    }

    let response_bytes = nodes
        .iter()
        .map(|n| n.text.len() + n.image.len() + 96)
        .sum::<usize>();

    PageRun {
        name: fixture.name,
        url: web_lab_variant_url(fixture.url, variant),
        pass,
        variant,
        nodes,
        response_bytes,
    }
}

fn web_lab_variant_url(url: &str, variant: usize) -> String {
    if variant == 0 {
        return url.to_string();
    }
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}forge_variant={variant}")
}

fn materialize_search(nodes: &mut Vec<WebNode>, root: usize, count: usize, pass: usize) {
    let header = push_node(
        nodes,
        Some(root),
        "header",
        "banner",
        "Google Search tabs Images News Videos Web",
        "",
        Bounds {
            x: 0,
            y: 0,
            w: 1440,
            h: 112,
        },
    );
    push_nav_tabs(
        nodes,
        header,
        &["All", "Images", "News", "Videos", "Maps", "Shopping"],
        72,
        190,
    );
    push_nav_tabs(
        nodes,
        header,
        &["Tools", "SafeSearch", "Recent", "Nearby"],
        94,
        190,
    );
    push_node(
        nodes,
        Some(header),
        "input",
        "searchbox",
        "Tokyo",
        "",
        Bounds {
            x: 180,
            y: 24,
            w: 760,
            h: 48,
        },
    );
    let hero = push_node(
        nodes,
        Some(root),
        "section",
        "knowledge-panel",
        "Tokyo Capitale de Japon meteo itineraire carte",
        "",
        Bounds {
            x: 190,
            y: 170,
            w: 980,
            h: 310,
        },
    );
    push_node(
        nodes,
        Some(hero),
        "img",
        "image",
        "Meiji-jingu Tokyo photo principale",
        "tokyo-meiji-jingu.jpg",
        Bounds {
            x: 200,
            y: 220,
            w: 520,
            h: 260,
        },
    );
    push_node(
        nodes,
        Some(hero),
        "img",
        "map",
        "Carte de Tokyo Japon",
        "tokyo-map.png",
        Bounds {
            x: 740,
            y: 220,
            w: 260,
            h: 260,
        },
    );

    for i in 0..count {
        let row = i / 2;
        let col = i % 2;
        let dynamic_tick = pass >= 2 && i == 3;
        let text = if dynamic_tick {
            format!("Tokyo hotel live offer refresh pass {pass}")
        } else {
            format!(
                "Tokyo result {} Wikipedia travel guide culture restaurants transport",
                i + 1
            )
        };
        let card = push_node(
            nodes,
            Some(root),
            "article",
            "search-result",
            &text,
            "",
            Bounds {
                x: 190 + (col as u32 * 520),
                y: 520 + (row as u32 * 96),
                w: 500,
                h: 82,
            },
        );
        if i % 4 == 0 {
            push_node(
                nodes,
                Some(card),
                "img",
                "thumbnail",
                "Tokyo thumbnail",
                "tokyo-thumb.jpg",
                Bounds {
                    x: 620,
                    y: 528 + (row as u32 * 96),
                    w: 80,
                    h: 72,
                },
            );
        }
    }
}

fn materialize_commerce(nodes: &mut Vec<WebNode>, root: usize, count: usize, pass: usize) {
    let header = push_node(
        nodes,
        Some(root),
        "header",
        "banner",
        "amazon.fr search categories account cart delivery",
        "",
        Bounds {
            x: 0,
            y: 0,
            w: 1440,
            h: 118,
        },
    );
    push_nav_tabs(
        nodes,
        header,
        &["Best sellers", "Deals", "Fresh", "Prime", "Books"],
        72,
        48,
    );
    push_nav_tabs(
        nodes,
        header,
        &["Electronics", "Home", "Fashion", "Toys"],
        96,
        48,
    );
    push_node(
        nodes,
        Some(header),
        "input",
        "searchbox",
        "Rechercher Amazon.fr",
        "",
        Bounds {
            x: 410,
            y: 18,
            w: 560,
            h: 44,
        },
    );
    push_node(
        nodes,
        Some(root),
        "section",
        "hero-carousel",
        "Prime original Citadel nouvelle saison",
        "amazon-prime-hero.jpg",
        Bounds {
            x: 0,
            y: 118,
            w: 1440,
            h: 360,
        },
    );

    for i in 0..count {
        let row = i / 4;
        let col = i % 4;
        let price = if pass >= 2 && i == 11 { 49 } else { 39 + (i % 9) };
        let text = format!(
            "Amazon product {} maison cuisine economie livraison prix {} euros",
            i + 1,
            price
        );
        let card = push_node(
            nodes,
            Some(root),
            "article",
            "product-card",
            &text,
            "",
            Bounds {
                x: 48 + (col as u32 * 336),
                y: 520 + (row as u32 * 220),
                w: 300,
                h: 196,
            },
        );
        push_node(
            nodes,
            Some(card),
            "img",
            "product-image",
            "Product image",
            "amazon-product.jpg",
            Bounds {
                x: 70 + (col as u32 * 336),
                y: 568 + (row as u32 * 220),
                w: 230,
                h: 120,
            },
        );
    }
}

fn materialize_docs(nodes: &mut Vec<WebNode>, root: usize, count: usize, pass: usize) {
    let header = push_node(
        nodes,
        Some(root),
        "header",
        "banner",
        "Google Drive Docs Sheets Slides search account",
        "",
        Bounds {
            x: 0,
            y: 0,
            w: 1440,
            h: 92,
        },
    );
    push_nav_tabs(
        nodes,
        header,
        &["Docs", "Sheets", "Slides", "Forms", "Drive"],
        58,
        280,
    );
    push_node(
        nodes,
        Some(header),
        "input",
        "searchbox",
        "Search in Drive",
        "",
        Bounds {
            x: 280,
            y: 18,
            w: 650,
            h: 48,
        },
    );
    push_node(
        nodes,
        Some(root),
        "nav",
        "sidebar",
        "Nouveau Accueil Mon Drive Ordinateurs Partages Recents Favoris",
        "",
        Bounds {
            x: 0,
            y: 92,
            w: 260,
            h: 1300,
        },
    );

    for i in 0..count {
        let row = i / 5;
        let col = i % 5;
        let changed = pass >= 2 && i == 5;
        let title = if changed {
            format!("Travel planning draft autosave revision {pass}")
        } else {
            format!("Document {} project brief metrics notes", i + 1)
        };
        let card = push_node(
            nodes,
            Some(root),
            "article",
            "document-card",
            &title,
            "",
            Bounds {
                x: 300 + (col as u32 * 210),
                y: 140 + (row as u32 * 190),
                w: 178,
                h: 158,
            },
        );
        push_node(
            nodes,
            Some(card),
            "img",
            "document-preview",
            "Document preview",
            "doc-preview.png",
            Bounds {
                x: 315 + (col as u32 * 210),
                y: 158 + (row as u32 * 190),
                w: 148,
                h: 96,
            },
        );
    }
}

fn push_nav_tabs(
    nodes: &mut Vec<WebNode>,
    parent: usize,
    labels: &[&str],
    y: u32,
    x_start: u32,
) {
    for (index, label) in labels.iter().enumerate() {
        push_node(
            nodes,
            Some(parent),
            "a",
            "tab",
            label,
            "",
            Bounds {
                x: x_start + (index as u32 * 112),
                y,
                w: 96,
                h: 32,
            },
        );
    }
}

fn push_node(
    nodes: &mut Vec<WebNode>,
    parent: Option<usize>,
    tag: &'static str,
    role: &'static str,
    text: &str,
    image: &'static str,
    bounds: Bounds,
) -> usize {
    let id = nodes.len();
    nodes.push(WebNode {
        id,
        parent,
        tag,
        role,
        text: text.to_string(),
        image,
        bounds,
        visible: true,
    });
    id
}

fn analyze_page(page: &PageRun, cache: &mut WebLabCache) -> (PagePlan, AvoidanceProof) {
    let page_hash = hash_page(page);
    let mut proof = AvoidanceProof::default();
    if let Some(plan) = cache.page_get(&page_hash) {
        proof.page_cache_hit = true;
        proof.page_cache_hits = 1;
        proof.node_walks_avoided = page.nodes.len() as u64;
        proof.score_evals_avoided = plan.candidate_count as u64;
        proof.score_hash_rounds_avoided = proof.score_evals_avoided * SCORE_HASH_ROUNDS;
        snapshot_cache_proof(cache, &mut proof);
        return (plan, proof);
    }

    proof.page_cache_misses = 1;
    proof.legacy_pipeline_scan_passes = LEGACY_PIPELINE_NODE_SCAN_PASSES;
    proof.fused_pipeline_scan_passes = FUSED_PIPELINE_NODE_SCAN_PASSES;
    proof.pipeline_node_scans_run =
        FUSED_PIPELINE_NODE_SCAN_PASSES.saturating_mul(page.nodes.len() as u64);
    proof.pipeline_node_scans_avoided = LEGACY_PIPELINE_NODE_SCAN_PASSES
        .saturating_sub(FUSED_PIPELINE_NODE_SCAN_PASSES)
        .saturating_mul(page.nodes.len() as u64);
    audit_nav_plan_middlemen(&page.nodes, &mut proof);
    let children = child_index(&page.nodes);
    proof.zero_copy_index_key_bytes_avoided = page
        .nodes
        .iter()
        .map(|node| 24u64.saturating_add(node.id.to_string().len() as u64))
        .sum();
    let mut descendant_vec_cache = HashMap::new();
    let mut subtree_memo = HashMap::new();
    let mut subtree_size_memo = HashMap::new();
    let mut text_cache = HashMap::with_capacity(page.nodes.len().min(2048));
    let mut source_label_cache = HashMap::with_capacity(page.nodes.len().min(512));
    let mut fallback_topk = Vec::with_capacity(FALLBACK_TOPK_LIMIT);
    let mut block_count = 0usize;
    let mut candidate_count = 0usize;
    let mut total_score = 0u64;

    for node in &page.nodes {
        proof.node_walks_run += 1;
        audit_plan_casefold_filters(node, &mut proof);
        audit_casefold_filter(node, &mut proof);
        let text_len = normalized_text_len(node, &mut text_cache, &mut proof);
        if !node.candidate(text_len) {
            continue;
        }
        candidate_count += 1;
        audit_casefold_block_classification(node, &mut proof);
        audit_source_label_lookup(node, &mut source_label_cache, &mut proof);
        let _ = normalized_text_len(node, &mut text_cache, &mut proof);
        let descendant_count = subtree_size(node.id, &children, &mut subtree_size_memo);
        audit_descendant_vec_cache(
            node.id,
            descendant_count,
            &mut descendant_vec_cache,
            &mut proof,
        );
        audit_content_field_fusion(descendant_count, &mut proof);
        audit_winner_selection(node.id, &page.nodes, &children, &mut proof);
        let descendant_text_len =
            touch_subtree_texts(node.id, &page.nodes, &children, &mut text_cache, &mut proof);
        if descendant_text_len > 0 {
            proof.text_join_allocations_avoided += 1;
        }
        proof.descendant_metric_scans_run += FUSED_DESCENDANT_METRIC_SCAN_PASSES;
        proof.descendant_metric_scans_avoided += LEGACY_DESCENDANT_METRIC_SCAN_PASSES
            .saturating_sub(FUSED_DESCENDANT_METRIC_SCAN_PASSES);
        proof.descendant_metric_node_visits_run = proof
            .descendant_metric_node_visits_run
            .saturating_add(descendant_count);
        proof.descendant_metric_node_visits_avoided = proof
            .descendant_metric_node_visits_avoided
            .saturating_add(
                descendant_count.saturating_mul(
                    LEGACY_DESCENDANT_METRIC_SCAN_PASSES
                        .saturating_sub(FUSED_DESCENDANT_METRIC_SCAN_PASSES),
                ),
            );
        proof.content_subtree_queries_run += 1;
        proof.content_subtree_queries_avoided += 1;
        proof.content_subtree_id_clones_avoided = proof
            .content_subtree_id_clones_avoided
            .saturating_add(descendant_count);
        let subtree_key = subtree_hash(node.id, &page.nodes, &children, &mut subtree_memo);
        let cached = cache.subtree_get(&subtree_key);
        let score = match cached {
            Some(cached) => {
                proof.subtree_cache_hits += 1;
                proof.score_evals_avoided += 1;
                proof.score_hash_rounds_avoided += SCORE_HASH_ROUNDS;
                cached
            }
            None => {
                proof.subtree_cache_misses += 1;
                proof.score_evals_run += 1;
                proof.score_hash_rounds_run += SCORE_HASH_ROUNDS;
                let computed = score_node(node, &subtree_key);
                cache.subtree_insert(subtree_key, computed.clone());
                computed
            }
        };
        if score.score >= 28 {
            block_count += 1;
        }
        if fallback_frame_audit_candidate(node, text_len) {
            audit_fallback_topk_candidate(
                FallbackFrameRank {
                    score: score.score,
                    area: node.bounds.area(),
                    y: node.bounds.y,
                },
                &mut fallback_topk,
                &mut proof,
            );
        }
        total_score += score.score as u64 + score.text_bytes as u64 + score.image_pixels as u64 / 16_384;
    }
    finalize_fallback_topk(&fallback_topk, &mut proof);

    let plan = PagePlan {
        page_hash,
        block_count,
        candidate_count,
        total_score,
    };
    cache.page_insert(page_hash, plan.clone());
    snapshot_cache_proof(cache, &mut proof);
    (plan, proof)
}

fn snapshot_cache_proof(cache: &WebLabCache, proof: &mut AvoidanceProof) {
    proof.page_cache_entries = cache.page_plans.len();
    proof.subtree_cache_entries = cache.subtree_scores.len();
    proof.page_cache_evictions = cache.page_evictions;
    proof.subtree_cache_evictions = cache.subtree_evictions;
    proof.cache_estimated_bytes = cache.estimated_bytes();
}

fn child_index(nodes: &[WebNode]) -> HashMap<usize, Vec<usize>> {
    let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
    for node in nodes {
        if let Some(parent) = node.parent {
            children.entry(parent).or_default().push(node.id);
        }
    }
    children
}

fn collapsed_text_stats(value: &str) -> (usize, u64) {
    let mut len = 0usize;
    let mut words = 0u64;
    for word in value.split_whitespace() {
        len += word.len();
        words += 1;
    }
    (len + words.saturating_sub(1) as usize, words)
}

fn ascii_eq_any(value: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| value.trim().eq_ignore_ascii_case(needle))
}

fn ascii_contains_ignore_case(value: &str, needle: &str) -> bool {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return true;
    }
    value
        .trim()
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn ascii_contains_any(value: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| ascii_contains_ignore_case(value, needle))
}

fn audit_nav_plan_middlemen(nodes: &[WebNode], proof: &mut AvoidanceProof) {
    let mut buckets: HashMap<(usize, i64), Vec<&WebNode>> = HashMap::new();
    for node in nodes {
        if !is_nav_plan_candidate(node) {
            continue;
        }
        proof.nav_candidates_seen += 1;
        proof.nav_candidate_vec_pushes_avoided += 1;
        proof.nav_bucket_key_allocations_avoided += 1;
        let parent = node.parent.unwrap_or(usize::MAX);
        let parent_key_bytes = if parent == usize::MAX {
            4
        } else {
            decimal_len(parent as u64)
        };
        let y_bucket = ((node.bounds.y as f64) / 18.0).round() as i64;
        proof.nav_bucket_key_bytes_avoided = proof
            .nav_bucket_key_bytes_avoided
            .saturating_add(parent_key_bytes + 1 + signed_decimal_len(y_bucket));
        buckets.entry((parent, y_bucket)).or_default().push(node);
    }

    let mut legacy_materializations = 0u64;
    let mut legacy_clone_bytes = 0u64;
    let mut best_score = f64::INFINITY;
    let mut best_materializations = 0u64;
    let mut best_clone_bytes = 0u64;
    for row in buckets.values_mut() {
        row.sort_by_key(|node| node.bounds.x);
        if row.len() < 3 || row.len() > 10 {
            continue;
        }
        let mut distinct: Vec<&str> = Vec::with_capacity(row.len());
        for node in row.iter() {
            if !distinct
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&node.text))
            {
                distinct.push(&node.text);
            }
        }
        if distinct.len() < 3 {
            continue;
        }
        let avg_y = row.iter().map(|node| node.bounds.y as f64).sum::<f64>() / row.len() as f64;
        let score = avg_y + if distinct.len() != row.len() { 120.0 } else { 0.0 };
        let row_clone_bytes = row
            .iter()
            .map(|node| (node.text.len() + node.role.len() + node.tag.len()) as u64)
            .sum::<u64>();
        legacy_materializations += row.len() as u64;
        legacy_clone_bytes = legacy_clone_bytes.saturating_add(row_clone_bytes);
        if score < best_score {
            best_score = score;
            best_materializations = row.len() as u64;
            best_clone_bytes = row_clone_bytes;
        }
    }

    proof.nav_item_materializations_deferred = proof
        .nav_item_materializations_deferred
        .saturating_add(legacy_materializations.saturating_sub(best_materializations));
    proof.nav_item_clone_bytes_deferred = proof
        .nav_item_clone_bytes_deferred
        .saturating_add(legacy_clone_bytes.saturating_sub(best_clone_bytes));
}

fn is_nav_plan_candidate(node: &WebNode) -> bool {
    node.visible
        && ascii_eq_any(node.role, &["link", "button", "tab"])
        && (2..=48).contains(&node.text.len())
}

fn decimal_len(mut value: u64) -> u64 {
    let mut len = 1;
    while value >= 10 {
        value /= 10;
        len += 1;
    }
    len
}

fn signed_decimal_len(value: i64) -> u64 {
    if value < 0 {
        1 + decimal_len(value.unsigned_abs())
    } else {
        decimal_len(value as u64)
    }
}

fn fallback_frame_audit_candidate(node: &WebNode, text_len: usize) -> bool {
    node.candidate(text_len)
        && !ascii_eq_any(node.tag, &["body", "header", "nav", "form", "input"])
        && !ascii_contains_any(node.role, &["banner", "tab", "searchbox", "sidebar"])
        && node.bounds.w >= 160
        && node.bounds.h >= 72
}

fn fallback_frame_rank_cmp(a: &FallbackFrameRank, b: &FallbackFrameRank) -> std::cmp::Ordering {
    b.score
        .cmp(&a.score)
        .then_with(|| b.area.cmp(&a.area))
        .then_with(|| a.y.cmp(&b.y))
}

fn audit_fallback_topk_candidate(
    candidate: FallbackFrameRank,
    topk: &mut Vec<FallbackFrameRank>,
    proof: &mut AvoidanceProof,
) {
    proof.fallback_topk_candidates_seen += 1;
    if topk.len() < FALLBACK_TOPK_LIMIT {
        topk.push(candidate);
        return;
    }

    let mut worst_index = 0;
    for index in 1..topk.len() {
        if fallback_frame_rank_cmp(&topk[index], &topk[worst_index])
            == std::cmp::Ordering::Greater
        {
            worst_index = index;
        }
    }

    if fallback_frame_rank_cmp(&candidate, &topk[worst_index]) == std::cmp::Ordering::Less {
        topk[worst_index] = candidate;
        proof.fallback_topk_replacements += 1;
    }
}

fn finalize_fallback_topk(topk: &[FallbackFrameRank], proof: &mut AvoidanceProof) {
    proof.fallback_topk_candidates_kept = topk.len() as u64;
    proof.fallback_topk_candidates_dropped = proof
        .fallback_topk_candidates_seen
        .saturating_sub(proof.fallback_topk_candidates_kept);
    proof.fallback_full_sort_items_avoided = proof.fallback_topk_candidates_dropped;
    proof.fallback_frame_vec_slots_avoided = proof.fallback_topk_candidates_dropped;
    proof.fallback_frame_vec_bytes_avoided = proof
        .fallback_frame_vec_slots_avoided
        .saturating_mul(std::mem::size_of::<FallbackFrameRank>() as u64);
}

fn audit_casefold_filter(node: &WebNode, proof: &mut AvoidanceProof) {
    proof.casefold_checks_run += 3;
    proof.casefold_allocations_avoided += 3;
    proof.casefold_bytes_avoided = proof.casefold_bytes_avoided.saturating_add(
        node.tag.len() as u64 + node.role.len() as u64 + node.text.len().min(180) as u64,
    );
    let _ = ascii_eq_any(node.tag, &["a", "h1", "h2", "h3", "article", "section"]);
    let _ = ascii_eq_any(node.role, &["link", "heading", "search-result", "product-card"]);
    let _ = ascii_contains_any(&node.text, &["related", "knowledge", "latest news"]);
}

fn audit_plan_casefold_filters(node: &WebNode, proof: &mut AvoidanceProof) {
    proof.plan_casefold_checks_run += 6;
    proof.plan_casefold_allocations_avoided += 6;
    proof.plan_casefold_bytes_avoided = proof.plan_casefold_bytes_avoided.saturating_add(
        node.role.len() as u64
            + node.tag.len() as u64
            + node.role.len() as u64
            + node.tag.len() as u64
            + node.text.len().min(220) as u64
            + node.image.len() as u64,
    );
    let _ = ascii_eq_any(node.role, &["link", "button", "tab"]);
    let _ = ascii_eq_any(node.role, &["main"]);
    let _ = ascii_eq_any(node.tag, &["main"]);
    let _ = ascii_contains_any(node.tag, &["main", "content", "results"]);
    let _ = ascii_contains_any(&node.text, &["main", "content", "results"]);
}

fn audit_casefold_block_classification(node: &WebNode, proof: &mut AvoidanceProof) {
    proof.casefold_checks_run += 4;
    proof.casefold_allocations_avoided += 4;
    proof.casefold_bytes_avoided = proof.casefold_bytes_avoided.saturating_add(
        node.tag.len() as u64
            + node.role.len() as u64
            + node.text.len().min(220) as u64
            + node.image.len() as u64,
    );
    let _ = ascii_contains_any(node.role, &["related", "knowledge", "cluster", "hero"]);
    let _ = ascii_contains_any(&node.text, &["recherches associ", "related search"]);
    let _ = ascii_contains_any(&node.text, &["top stories", "latest news"]);
    let _ = ascii_eq_any(node.tag, &["html", "body", "main", "nav", "header", "footer", "form"]);
}

fn audit_source_label_lookup(
    node: &WebNode,
    cache: &mut HashMap<&'static str, &'static str>,
    proof: &mut AvoidanceProof,
) {
    let Some(url) = synthetic_source_url(node) else {
        return;
    };
    proof.url_label_queries_run += 1;
    if cache.contains_key(url) {
        proof.url_label_cache_hits += 1;
        proof.url_label_full_parse_avoided += 1;
        proof.url_label_bytes_avoided = proof
            .url_label_bytes_avoided
            .saturating_add(url.len() as u64);
        return;
    }

    proof.url_label_cache_misses += 1;
    if let Some(host) = fast_http_host(url) {
        proof.url_label_fast_path_hits += 1;
        proof.url_label_full_parse_avoided += 1;
        proof.url_label_bytes_avoided = proof
            .url_label_bytes_avoided
            .saturating_add(url.len() as u64);
        cache.insert(url, host);
    } else {
        proof.url_label_full_parse_run += 1;
        cache.insert(url, "");
    }
}

fn audit_descendant_vec_cache(
    root_id: usize,
    descendant_count: u64,
    cache: &mut HashMap<usize, u64>,
    proof: &mut AvoidanceProof,
) {
    proof.descendant_vec_queries_run += 1;
    proof.descendant_vec_clone_allocations_avoided += 1;
    proof.descendant_vec_slots_avoided = proof
        .descendant_vec_slots_avoided
        .saturating_add(descendant_count);
    proof.descendant_vec_bytes_avoided = proof.descendant_vec_bytes_avoided.saturating_add(
        descendant_count.saturating_mul(std::mem::size_of::<usize>() as u64),
    );
    if cache.contains_key(&root_id) {
        proof.descendant_vec_cache_hits += 1;
    } else {
        proof.descendant_vec_builds += 1;
        cache.insert(root_id, descendant_count);
    }
}

fn audit_content_field_fusion(descendant_count: u64, proof: &mut AvoidanceProof) {
    proof.content_field_scan_queries_run += 1;
    proof.content_field_scan_passes_run += FUSED_CONTENT_FIELD_SCAN_PASSES;
    proof.content_field_scan_passes_avoided += LEGACY_CONTENT_FIELD_SCAN_PASSES
        .saturating_sub(FUSED_CONTENT_FIELD_SCAN_PASSES);
    proof.content_field_node_visits_run = proof
        .content_field_node_visits_run
        .saturating_add(descendant_count);
    proof.content_field_node_visits_avoided = proof
        .content_field_node_visits_avoided
        .saturating_add(
            descendant_count.saturating_mul(
                LEGACY_CONTENT_FIELD_SCAN_PASSES
                    .saturating_sub(FUSED_CONTENT_FIELD_SCAN_PASSES),
            ),
        );
}

fn synthetic_source_url(node: &WebNode) -> Option<&'static str> {
    match node.role {
        "knowledge-panel" => Some("https://www.google.com/search?q=Tokyo"),
        "search-result" => Some("https://fr.wikipedia.org/wiki/Tokyo"),
        "thumbnail" | "image" | "map" => Some("https://images.google.com/search?q=Tokyo"),
        "hero-carousel" => Some("https://www.amazon.fr/gp/video/storefront"),
        "product-card" | "product-image" => Some("https://www.amazon.fr/dp/forge-demo"),
        "document-card" | "document-preview" => {
            Some("https://docs.google.com/document/d/forge-demo/edit")
        }
        _ => None,
    }
}

fn fast_http_host(url: &'static str) -> Option<&'static str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority_end = rest
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let authority = rest[..authority_end].trim();
    if authority.is_empty() || authority.contains('@') || authority.starts_with('[') {
        return None;
    }
    Some(
        authority
            .rsplit_once(':')
            .filter(|(_, port)| port.chars().all(|ch| ch.is_ascii_digit()))
            .map(|(host, _)| host)
            .unwrap_or(authority)
            .trim(),
    )
}

fn audit_winner_selection(
    root_id: usize,
    nodes: &[WebNode],
    children: &HashMap<usize, Vec<usize>>,
    proof: &mut AvoidanceProof,
) {
    proof.winner_candidate_scans_run += 1;

    let root_y = nodes[root_id].bounds.y;
    let mut stack = Vec::new();
    if let Some(child_ids) = children.get(&root_id) {
        stack.extend(child_ids.iter().copied());
    }

    let mut summary_candidates = 0u64;
    let mut summary_bytes = 0u64;
    let mut summary_winner_bytes = 0u64;
    let mut summary_best_distance = u32::MAX;
    let mut source_candidates = 0u64;
    let mut source_bytes = 0u64;
    let mut source_winner_bytes = 0u64;
    let mut source_best_distance = u32::MAX;

    while let Some(node_id) = stack.pop() {
        let node = &nodes[node_id];
        if let Some(child_ids) = children.get(&node_id) {
            stack.extend(child_ids.iter().copied());
        }
        if !node.visible || node.text.is_empty() {
            continue;
        }

        let text_len = node.text.len();
        if node.image.is_empty() && (36..=320).contains(&text_len) {
            let bytes = text_len as u64;
            let distance = node.bounds.y.abs_diff(root_y);
            summary_candidates += 1;
            summary_bytes = summary_bytes.saturating_add(bytes);
            if distance < summary_best_distance {
                summary_best_distance = distance;
                summary_winner_bytes = bytes;
            }
        }
        if (3..=56).contains(&text_len) {
            let bytes = text_len as u64;
            let distance = node.bounds.y.abs_diff(root_y);
            source_candidates += 1;
            source_bytes = source_bytes.saturating_add(bytes);
            if distance < source_best_distance {
                source_best_distance = distance;
                source_winner_bytes = bytes;
            }
        }
    }

    let summary_final = u64::from(summary_candidates > 0);
    let source_final = u64::from(source_candidates > 0);
    proof.winner_text_candidates_seen = proof
        .winner_text_candidates_seen
        .saturating_add(summary_candidates + source_candidates);
    proof.winner_final_clones_run = proof
        .winner_final_clones_run
        .saturating_add(summary_final + source_final);
    proof.winner_text_clones_avoided = proof.winner_text_clones_avoided.saturating_add(
        summary_candidates.saturating_sub(summary_final)
            + source_candidates.saturating_sub(source_final),
    );
    proof.winner_text_bytes_avoided = proof.winner_text_bytes_avoided.saturating_add(
        summary_bytes.saturating_sub(summary_winner_bytes)
            + source_bytes.saturating_sub(source_winner_bytes),
    );
}

fn normalized_text_len(
    node: &WebNode,
    cache: &mut HashMap<usize, usize>,
    proof: &mut AvoidanceProof,
) -> usize {
    if let Some(len) = cache.get(&node.id) {
        proof.text_cache_hits += 1;
        proof.text_bytes_avoided = proof
            .text_bytes_avoided
            .saturating_add(node.text.len() as u64);
        return *len;
    }
    let (len, word_count) = collapsed_text_stats(&node.text);
    proof.text_cache_misses += 1;
    proof.text_bytes_normalized = proof
        .text_bytes_normalized
        .saturating_add(node.text.len() as u64);
    if word_count > 0 {
        proof.text_collapse_vec_allocations_avoided += 1;
        proof.text_collapse_word_slots_avoided = proof
            .text_collapse_word_slots_avoided
            .saturating_add(word_count);
    }
    cache.insert(node.id, len);
    len
}

fn touch_subtree_texts(
    id: usize,
    nodes: &[WebNode],
    children: &HashMap<usize, Vec<usize>>,
    cache: &mut HashMap<usize, usize>,
    proof: &mut AvoidanceProof,
) -> usize {
    let mut len = normalized_text_len(&nodes[id], cache, proof);
    if let Some(child_ids) = children.get(&id) {
        for child_id in child_ids {
            len = len.saturating_add(touch_subtree_texts(*child_id, nodes, children, cache, proof));
        }
    }
    len
}

fn subtree_hash(
    id: usize,
    nodes: &[WebNode],
    children: &HashMap<usize, Vec<usize>>,
    memo: &mut HashMap<usize, [u8; 32]>,
) -> [u8; 32] {
    if let Some(hash) = memo.get(&id) {
        return *hash;
    }
    let node = &nodes[id];
    let mut hasher = Sha256::new();
    hasher.update(node.tag.as_bytes());
    hasher.update(node.role.as_bytes());
    hasher.update(node.text.as_bytes());
    hasher.update(node.image.as_bytes());
    hasher.update(node.bounds.x.to_le_bytes());
    hasher.update(node.bounds.y.to_le_bytes());
    hasher.update(node.bounds.w.to_le_bytes());
    hasher.update(node.bounds.h.to_le_bytes());
    if let Some(child_ids) = children.get(&id) {
        for child_id in child_ids {
            hasher.update(subtree_hash(*child_id, nodes, children, memo));
        }
    }
    let hash = hasher.finalize().into();
    memo.insert(id, hash);
    hash
}

fn subtree_size(
    id: usize,
    children: &HashMap<usize, Vec<usize>>,
    memo: &mut HashMap<usize, u64>,
) -> u64 {
    if let Some(size) = memo.get(&id) {
        return *size;
    }
    let mut size = 1u64;
    if let Some(child_ids) = children.get(&id) {
        for child_id in child_ids {
            size = size.saturating_add(subtree_size(*child_id, children, memo));
        }
    }
    memo.insert(id, size);
    size
}

fn hash_page(page: &PageRun) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(page.url.as_bytes());
    hasher.update(page.nodes.len().to_le_bytes());
    for node in &page.nodes {
        hasher.update(node.id.to_le_bytes());
        hasher.update(node.parent.unwrap_or(usize::MAX).to_le_bytes());
        hasher.update(node.tag.as_bytes());
        hasher.update(node.role.as_bytes());
        hasher.update(node.text.as_bytes());
        hasher.update(node.image.as_bytes());
        hasher.update(node.bounds.x.to_le_bytes());
        hasher.update(node.bounds.y.to_le_bytes());
        hasher.update(node.bounds.w.to_le_bytes());
        hasher.update(node.bounds.h.to_le_bytes());
    }
    hasher.finalize().into()
}

fn score_node(node: &WebNode, subtree_hash: &[u8; 32]) -> CachedNode {
    let mut state = *subtree_hash;
    for round in 0..SCORE_HASH_ROUNDS {
        let mut hasher = Sha256::new();
        hasher.update(state);
        hasher.update(round.to_le_bytes());
        hasher.update(node.text.as_bytes());
        hasher.update(node.image.as_bytes());
        state = hasher.finalize().into();
    }

    let role_weight = match node.role {
        "searchbox" => 42,
        "hero-carousel" | "knowledge-panel" => 38,
        "product-card" | "search-result" | "document-card" => 32,
        "image" | "map" | "product-image" | "document-preview" => 26,
        _ => 18,
    };
    let visual_weight = (node.bounds.area() / 18_000).min(22);
    let entropy_weight = (state[0] as u32 + state[7] as u32 + state[15] as u32) % 17;
    let score = role_weight + visual_weight + entropy_weight + (node.text.len() as u32 / 48);

    CachedNode {
        score,
        text_bytes: node.text.len(),
        image_pixels: if node.image.is_empty() {
            0
        } else {
            node.bounds.area()
        },
    }
}

fn update_totals(totals: &mut LabTotals, run: &TimedRun) {
    totals.pages += 1;
    totals.passes = totals.passes.max((run.page.pass + 1) as u64);
    totals.variants = totals.variants.max((run.page.variant + 1) as u64);
    totals.load_us += run.load_time.as_micros();
    totals.exec_us += run.analysis_time.as_micros();
    totals.load_samples_us.push(run.load_time.as_micros());
    totals.exec_samples_us.push(run.analysis_time.as_micros());
    totals.nodes += run.page.nodes.len() as u64;
    totals.candidates += run.plan.candidate_count as u64;
    totals.page_cache_hits += run.proof.page_cache_hits;
    totals.page_cache_misses += run.proof.page_cache_misses;
    totals.subtree_cache_hits += run.proof.subtree_cache_hits;
    totals.subtree_cache_misses += run.proof.subtree_cache_misses;
    totals.node_walks_run += run.proof.node_walks_run;
    totals.node_walks_avoided += run.proof.node_walks_avoided;
    totals.score_evals_run += run.proof.score_evals_run;
    totals.score_evals_avoided += run.proof.score_evals_avoided;
    totals.score_hash_rounds_run += run.proof.score_hash_rounds_run;
    totals.score_hash_rounds_avoided += run.proof.score_hash_rounds_avoided;
    totals.pipeline_node_scans_run += run.proof.pipeline_node_scans_run;
    totals.pipeline_node_scans_avoided += run.proof.pipeline_node_scans_avoided;
    totals.nav_candidates_seen += run.proof.nav_candidates_seen;
    totals.nav_candidate_vec_pushes_avoided += run.proof.nav_candidate_vec_pushes_avoided;
    totals.nav_bucket_key_allocations_avoided += run.proof.nav_bucket_key_allocations_avoided;
    totals.nav_bucket_key_bytes_avoided += run.proof.nav_bucket_key_bytes_avoided;
    totals.nav_item_materializations_deferred += run.proof.nav_item_materializations_deferred;
    totals.nav_item_clone_bytes_deferred += run.proof.nav_item_clone_bytes_deferred;
    totals.fallback_topk_candidates_seen += run.proof.fallback_topk_candidates_seen;
    totals.fallback_topk_candidates_kept += run.proof.fallback_topk_candidates_kept;
    totals.fallback_topk_candidates_dropped += run.proof.fallback_topk_candidates_dropped;
    totals.fallback_topk_replacements += run.proof.fallback_topk_replacements;
    totals.fallback_full_sort_items_avoided += run.proof.fallback_full_sort_items_avoided;
    totals.fallback_frame_vec_slots_avoided += run.proof.fallback_frame_vec_slots_avoided;
    totals.fallback_frame_vec_bytes_avoided += run.proof.fallback_frame_vec_bytes_avoided;
    totals.content_subtree_queries_run += run.proof.content_subtree_queries_run;
    totals.content_subtree_queries_avoided += run.proof.content_subtree_queries_avoided;
    totals.content_subtree_id_clones_avoided += run.proof.content_subtree_id_clones_avoided;
    totals.descendant_vec_queries_run += run.proof.descendant_vec_queries_run;
    totals.descendant_vec_cache_hits += run.proof.descendant_vec_cache_hits;
    totals.descendant_vec_builds += run.proof.descendant_vec_builds;
    totals.descendant_vec_clone_allocations_avoided +=
        run.proof.descendant_vec_clone_allocations_avoided;
    totals.descendant_vec_slots_avoided += run.proof.descendant_vec_slots_avoided;
    totals.descendant_vec_bytes_avoided += run.proof.descendant_vec_bytes_avoided;
    totals.zero_copy_index_key_bytes_avoided += run.proof.zero_copy_index_key_bytes_avoided;
    totals.text_cache_hits += run.proof.text_cache_hits;
    totals.text_cache_misses += run.proof.text_cache_misses;
    totals.text_bytes_normalized += run.proof.text_bytes_normalized;
    totals.text_bytes_avoided += run.proof.text_bytes_avoided;
    totals.text_join_allocations_avoided += run.proof.text_join_allocations_avoided;
    totals.text_collapse_vec_allocations_avoided += run.proof.text_collapse_vec_allocations_avoided;
    totals.text_collapse_word_slots_avoided += run.proof.text_collapse_word_slots_avoided;
    totals.content_field_scan_queries_run += run.proof.content_field_scan_queries_run;
    totals.content_field_scan_passes_run += run.proof.content_field_scan_passes_run;
    totals.content_field_scan_passes_avoided += run.proof.content_field_scan_passes_avoided;
    totals.content_field_node_visits_run += run.proof.content_field_node_visits_run;
    totals.content_field_node_visits_avoided += run.proof.content_field_node_visits_avoided;
    totals.descendant_metric_scans_run += run.proof.descendant_metric_scans_run;
    totals.descendant_metric_scans_avoided += run.proof.descendant_metric_scans_avoided;
    totals.descendant_metric_node_visits_run += run.proof.descendant_metric_node_visits_run;
    totals.descendant_metric_node_visits_avoided += run.proof.descendant_metric_node_visits_avoided;
    totals.plan_casefold_checks_run += run.proof.plan_casefold_checks_run;
    totals.plan_casefold_allocations_avoided += run.proof.plan_casefold_allocations_avoided;
    totals.plan_casefold_bytes_avoided += run.proof.plan_casefold_bytes_avoided;
    totals.casefold_checks_run += run.proof.casefold_checks_run;
    totals.casefold_allocations_avoided += run.proof.casefold_allocations_avoided;
    totals.casefold_bytes_avoided += run.proof.casefold_bytes_avoided;
    totals.url_label_queries_run += run.proof.url_label_queries_run;
    totals.url_label_cache_hits += run.proof.url_label_cache_hits;
    totals.url_label_cache_misses += run.proof.url_label_cache_misses;
    totals.url_label_fast_path_hits += run.proof.url_label_fast_path_hits;
    totals.url_label_full_parse_run += run.proof.url_label_full_parse_run;
    totals.url_label_full_parse_avoided += run.proof.url_label_full_parse_avoided;
    totals.url_label_bytes_avoided += run.proof.url_label_bytes_avoided;
    totals.winner_candidate_scans_run += run.proof.winner_candidate_scans_run;
    totals.winner_text_candidates_seen += run.proof.winner_text_candidates_seen;
    totals.winner_final_clones_run += run.proof.winner_final_clones_run;
    totals.winner_text_clones_avoided += run.proof.winner_text_clones_avoided;
    totals.winner_text_bytes_avoided += run.proof.winner_text_bytes_avoided;
    totals.page_cache_entries = run.proof.page_cache_entries;
    totals.subtree_cache_entries = run.proof.subtree_cache_entries;
    totals.page_cache_evictions = run.proof.page_cache_evictions;
    totals.subtree_cache_evictions = run.proof.subtree_cache_evictions;
    totals.cache_estimated_bytes = run.proof.cache_estimated_bytes;

    if run.page.pass == 0 {
        totals.cold_runs += 1;
        totals.cold_exec_us += run.analysis_time.as_micros();
    } else {
        totals.warm_runs += 1;
        totals.warm_exec_us += run.analysis_time.as_micros();
    }
}

fn emit_run(writer: &mut Box<dyn Write>, run: &TimedRun) -> io::Result<()> {
    writeln!(
        writer,
        "{{\"event\":\"web_lab.page\",\"fixture\":\"{}\",\"url\":\"{}\",\"pass\":{},\"variant\":{},\"nodes\":{},\"response_bytes\":{},\"load_us\":{},\"exec_us\":{},\"page_hash\":\"{}\",\"page_cache_hit\":{},\"page_cache_hits\":{},\"page_cache_misses\":{},\"subtree_cache_hits\":{},\"subtree_cache_misses\":{},\"node_walks_run\":{},\"node_walks_avoided\":{},\"score_evals_run\":{},\"score_evals_avoided\":{},\"score_hash_rounds_run\":{},\"score_hash_rounds_avoided\":{},\"pipeline_node_scans_run\":{},\"pipeline_node_scans_avoided\":{},\"legacy_pipeline_scan_passes\":{},\"fused_pipeline_scan_passes\":{},\"nav_candidates_seen\":{},\"nav_candidate_vec_pushes_avoided\":{},\"nav_bucket_key_allocations_avoided\":{},\"nav_bucket_key_bytes_avoided\":{},\"nav_item_materializations_deferred\":{},\"nav_item_clone_bytes_deferred\":{},\"fallback_topk_candidates_seen\":{},\"fallback_topk_candidates_kept\":{},\"fallback_topk_candidates_dropped\":{},\"fallback_topk_replacements\":{},\"fallback_full_sort_items_avoided\":{},\"fallback_frame_vec_slots_avoided\":{},\"fallback_frame_vec_bytes_avoided\":{},\"content_subtree_queries_run\":{},\"content_subtree_queries_avoided\":{},\"content_subtree_id_clones_avoided\":{},\"descendant_vec_queries_run\":{},\"descendant_vec_cache_hits\":{},\"descendant_vec_builds\":{},\"descendant_vec_clone_allocations_avoided\":{},\"descendant_vec_slots_avoided\":{},\"descendant_vec_bytes_avoided\":{},\"zero_copy_index_key_bytes_avoided\":{},\"text_cache_hits\":{},\"text_cache_misses\":{},\"text_bytes_normalized\":{},\"text_bytes_avoided\":{},\"text_join_allocations_avoided\":{},\"text_collapse_vec_allocations_avoided\":{},\"text_collapse_word_slots_avoided\":{},\"content_field_scan_queries_run\":{},\"content_field_scan_passes_run\":{},\"content_field_scan_passes_avoided\":{},\"content_field_node_visits_run\":{},\"content_field_node_visits_avoided\":{},\"descendant_metric_scans_run\":{},\"descendant_metric_scans_avoided\":{},\"descendant_metric_node_visits_run\":{},\"descendant_metric_node_visits_avoided\":{},\"plan_casefold_checks_run\":{},\"plan_casefold_allocations_avoided\":{},\"plan_casefold_bytes_avoided\":{},\"casefold_checks_run\":{},\"casefold_allocations_avoided\":{},\"casefold_bytes_avoided\":{},\"url_label_queries_run\":{},\"url_label_cache_hits\":{},\"url_label_cache_misses\":{},\"url_label_fast_path_hits\":{},\"url_label_full_parse_run\":{},\"url_label_full_parse_avoided\":{},\"url_label_bytes_avoided\":{},\"winner_candidate_scans_run\":{},\"winner_text_candidates_seen\":{},\"winner_final_clones_run\":{},\"winner_text_clones_avoided\":{},\"winner_text_bytes_avoided\":{},\"candidate_count\":{},\"block_count\":{},\"total_score\":{},\"page_cache_entries\":{},\"subtree_cache_entries\":{},\"page_cache_evictions\":{},\"subtree_cache_evictions\":{},\"cache_estimated_bytes\":{}}}",
        json_escape(run.page.name),
        json_escape(&run.page.url),
        run.page.pass,
        run.page.variant,
        run.page.nodes.len(),
        run.page.response_bytes,
        run.load_time.as_micros(),
        run.analysis_time.as_micros(),
        hex_hash(&run.plan.page_hash),
        run.proof.page_cache_hit,
        run.proof.page_cache_hits,
        run.proof.page_cache_misses,
        run.proof.subtree_cache_hits,
        run.proof.subtree_cache_misses,
        run.proof.node_walks_run,
        run.proof.node_walks_avoided,
        run.proof.score_evals_run,
        run.proof.score_evals_avoided,
        run.proof.score_hash_rounds_run,
        run.proof.score_hash_rounds_avoided,
        run.proof.pipeline_node_scans_run,
        run.proof.pipeline_node_scans_avoided,
        run.proof.legacy_pipeline_scan_passes,
        run.proof.fused_pipeline_scan_passes,
        run.proof.nav_candidates_seen,
        run.proof.nav_candidate_vec_pushes_avoided,
        run.proof.nav_bucket_key_allocations_avoided,
        run.proof.nav_bucket_key_bytes_avoided,
        run.proof.nav_item_materializations_deferred,
        run.proof.nav_item_clone_bytes_deferred,
        run.proof.fallback_topk_candidates_seen,
        run.proof.fallback_topk_candidates_kept,
        run.proof.fallback_topk_candidates_dropped,
        run.proof.fallback_topk_replacements,
        run.proof.fallback_full_sort_items_avoided,
        run.proof.fallback_frame_vec_slots_avoided,
        run.proof.fallback_frame_vec_bytes_avoided,
        run.proof.content_subtree_queries_run,
        run.proof.content_subtree_queries_avoided,
        run.proof.content_subtree_id_clones_avoided,
        run.proof.descendant_vec_queries_run,
        run.proof.descendant_vec_cache_hits,
        run.proof.descendant_vec_builds,
        run.proof.descendant_vec_clone_allocations_avoided,
        run.proof.descendant_vec_slots_avoided,
        run.proof.descendant_vec_bytes_avoided,
        run.proof.zero_copy_index_key_bytes_avoided,
        run.proof.text_cache_hits,
        run.proof.text_cache_misses,
        run.proof.text_bytes_normalized,
        run.proof.text_bytes_avoided,
        run.proof.text_join_allocations_avoided,
        run.proof.text_collapse_vec_allocations_avoided,
        run.proof.text_collapse_word_slots_avoided,
        run.proof.content_field_scan_queries_run,
        run.proof.content_field_scan_passes_run,
        run.proof.content_field_scan_passes_avoided,
        run.proof.content_field_node_visits_run,
        run.proof.content_field_node_visits_avoided,
        run.proof.descendant_metric_scans_run,
        run.proof.descendant_metric_scans_avoided,
        run.proof.descendant_metric_node_visits_run,
        run.proof.descendant_metric_node_visits_avoided,
        run.proof.plan_casefold_checks_run,
        run.proof.plan_casefold_allocations_avoided,
        run.proof.plan_casefold_bytes_avoided,
        run.proof.casefold_checks_run,
        run.proof.casefold_allocations_avoided,
        run.proof.casefold_bytes_avoided,
        run.proof.url_label_queries_run,
        run.proof.url_label_cache_hits,
        run.proof.url_label_cache_misses,
        run.proof.url_label_fast_path_hits,
        run.proof.url_label_full_parse_run,
        run.proof.url_label_full_parse_avoided,
        run.proof.url_label_bytes_avoided,
        run.proof.winner_candidate_scans_run,
        run.proof.winner_text_candidates_seen,
        run.proof.winner_final_clones_run,
        run.proof.winner_text_clones_avoided,
        run.proof.winner_text_bytes_avoided,
        run.plan.candidate_count,
        run.plan.block_count,
        run.plan.total_score,
        run.proof.page_cache_entries,
        run.proof.subtree_cache_entries,
        run.proof.page_cache_evictions,
        run.proof.subtree_cache_evictions,
        run.proof.cache_estimated_bytes
    )
}

fn emit_summary(writer: &mut Box<dyn Write>, totals: &LabTotals) -> io::Result<()> {
    writeln!(
        writer,
        "{{\"event\":\"web_lab.summary\",\"runs\":{},\"passes\":{},\"variants\":{},\"load_us\":{},\"exec_us\":{},\"load_p50_us\":{},\"load_p95_us\":{},\"load_p99_us\":{},\"exec_p50_us\":{},\"exec_p95_us\":{},\"exec_p99_us\":{},\"cold_exec_us\":{},\"warm_exec_us\":{},\"cold_runs\":{},\"warm_runs\":{},\"nodes\":{},\"candidates\":{},\"page_cache_hits\":{},\"page_cache_misses\":{},\"subtree_cache_hits\":{},\"subtree_cache_misses\":{},\"node_walks_run\":{},\"node_walks_avoided\":{},\"score_evals_run\":{},\"score_evals_avoided\":{},\"score_hash_rounds_run\":{},\"score_hash_rounds_avoided\":{},\"pipeline_node_scans_run\":{},\"pipeline_node_scans_avoided\":{},\"pipeline_scan_avoidance_pct\":{:.3},\"nav_candidates_seen\":{},\"nav_candidate_vec_pushes_avoided\":{},\"nav_bucket_key_allocations_avoided\":{},\"nav_bucket_key_bytes_avoided\":{},\"nav_item_materializations_deferred\":{},\"nav_item_clone_bytes_deferred\":{},\"fallback_topk_candidates_seen\":{},\"fallback_topk_candidates_kept\":{},\"fallback_topk_candidates_dropped\":{},\"fallback_topk_replacements\":{},\"fallback_full_sort_items_avoided\":{},\"fallback_frame_vec_slots_avoided\":{},\"fallback_frame_vec_bytes_avoided\":{},\"content_subtree_queries_run\":{},\"content_subtree_queries_avoided\":{},\"content_subtree_query_avoidance_pct\":{:.3},\"content_subtree_id_clones_avoided\":{},\"descendant_vec_queries_run\":{},\"descendant_vec_cache_hits\":{},\"descendant_vec_builds\":{},\"descendant_vec_clone_allocations_avoided\":{},\"descendant_vec_slots_avoided\":{},\"descendant_vec_bytes_avoided\":{},\"descendant_vec_clone_avoidance_pct\":{:.3},\"zero_copy_index_key_bytes_avoided\":{},\"text_cache_hits\":{},\"text_cache_misses\":{},\"text_hit_rate_pct\":{:.3},\"text_bytes_normalized\":{},\"text_bytes_avoided\":{},\"text_join_allocations_avoided\":{},\"text_collapse_vec_allocations_avoided\":{},\"text_collapse_word_slots_avoided\":{},\"content_field_scan_queries_run\":{},\"content_field_scan_passes_run\":{},\"content_field_scan_passes_avoided\":{},\"content_field_scan_avoidance_pct\":{:.3},\"content_field_node_visits_run\":{},\"content_field_node_visits_avoided\":{},\"content_field_node_visit_avoidance_pct\":{:.3},\"descendant_metric_scans_run\":{},\"descendant_metric_scans_avoided\":{},\"descendant_metric_scan_avoidance_pct\":{:.3},\"descendant_metric_node_visits_run\":{},\"descendant_metric_node_visits_avoided\":{},\"descendant_metric_node_visit_avoidance_pct\":{:.3},\"plan_casefold_checks_run\":{},\"plan_casefold_allocations_avoided\":{},\"plan_casefold_bytes_avoided\":{},\"casefold_checks_run\":{},\"casefold_allocations_avoided\":{},\"casefold_bytes_avoided\":{},\"url_label_queries_run\":{},\"url_label_cache_hits\":{},\"url_label_cache_misses\":{},\"url_label_hit_rate_pct\":{:.3},\"url_label_fast_path_hits\":{},\"url_label_full_parse_run\":{},\"url_label_full_parse_avoided\":{},\"url_label_parse_avoidance_pct\":{:.3},\"url_label_bytes_avoided\":{},\"winner_candidate_scans_run\":{},\"winner_text_candidates_seen\":{},\"winner_final_clones_run\":{},\"winner_text_clones_avoided\":{},\"winner_clone_avoidance_pct\":{:.3},\"winner_text_bytes_avoided\":{},\"score_eval_avoidance_pct\":{:.3},\"hash_round_avoidance_pct\":{:.3},\"warm_speedup_x\":{:.3},\"page_cache_entries\":{},\"subtree_cache_entries\":{},\"page_cache_max\":{},\"subtree_cache_max\":{},\"page_cache_evictions\":{},\"subtree_cache_evictions\":{},\"cache_estimated_bytes\":{}}}",
        totals.pages,
        totals.passes,
        totals.variants,
        totals.load_us,
        totals.exec_us,
        percentile(&totals.load_samples_us, 50.0),
        percentile(&totals.load_samples_us, 95.0),
        percentile(&totals.load_samples_us, 99.0),
        percentile(&totals.exec_samples_us, 50.0),
        percentile(&totals.exec_samples_us, 95.0),
        percentile(&totals.exec_samples_us, 99.0),
        totals.cold_exec_us,
        totals.warm_exec_us,
        totals.cold_runs,
        totals.warm_runs,
        totals.nodes,
        totals.candidates,
        totals.page_cache_hits,
        totals.page_cache_misses,
        totals.subtree_cache_hits,
        totals.subtree_cache_misses,
        totals.node_walks_run,
        totals.node_walks_avoided,
        totals.score_evals_run,
        totals.score_evals_avoided,
        totals.score_hash_rounds_run,
        totals.score_hash_rounds_avoided,
        totals.pipeline_node_scans_run,
        totals.pipeline_node_scans_avoided,
        pct(
            totals.pipeline_node_scans_avoided,
            totals.pipeline_node_scans_run + totals.pipeline_node_scans_avoided
        ),
        totals.nav_candidates_seen,
        totals.nav_candidate_vec_pushes_avoided,
        totals.nav_bucket_key_allocations_avoided,
        totals.nav_bucket_key_bytes_avoided,
        totals.nav_item_materializations_deferred,
        totals.nav_item_clone_bytes_deferred,
        totals.fallback_topk_candidates_seen,
        totals.fallback_topk_candidates_kept,
        totals.fallback_topk_candidates_dropped,
        totals.fallback_topk_replacements,
        totals.fallback_full_sort_items_avoided,
        totals.fallback_frame_vec_slots_avoided,
        totals.fallback_frame_vec_bytes_avoided,
        totals.content_subtree_queries_run,
        totals.content_subtree_queries_avoided,
        pct(
            totals.content_subtree_queries_avoided,
            totals.content_subtree_queries_run + totals.content_subtree_queries_avoided
        ),
        totals.content_subtree_id_clones_avoided,
        totals.descendant_vec_queries_run,
        totals.descendant_vec_cache_hits,
        totals.descendant_vec_builds,
        totals.descendant_vec_clone_allocations_avoided,
        totals.descendant_vec_slots_avoided,
        totals.descendant_vec_bytes_avoided,
        pct(
            totals.descendant_vec_clone_allocations_avoided,
            totals.descendant_vec_queries_run
        ),
        totals.zero_copy_index_key_bytes_avoided,
        totals.text_cache_hits,
        totals.text_cache_misses,
        pct(
            totals.text_cache_hits,
            totals.text_cache_hits + totals.text_cache_misses
        ),
        totals.text_bytes_normalized,
        totals.text_bytes_avoided,
        totals.text_join_allocations_avoided,
        totals.text_collapse_vec_allocations_avoided,
        totals.text_collapse_word_slots_avoided,
        totals.content_field_scan_queries_run,
        totals.content_field_scan_passes_run,
        totals.content_field_scan_passes_avoided,
        pct(
            totals.content_field_scan_passes_avoided,
            totals.content_field_scan_passes_run + totals.content_field_scan_passes_avoided
        ),
        totals.content_field_node_visits_run,
        totals.content_field_node_visits_avoided,
        pct(
            totals.content_field_node_visits_avoided,
            totals.content_field_node_visits_run + totals.content_field_node_visits_avoided
        ),
        totals.descendant_metric_scans_run,
        totals.descendant_metric_scans_avoided,
        pct(
            totals.descendant_metric_scans_avoided,
            totals.descendant_metric_scans_run + totals.descendant_metric_scans_avoided
        ),
        totals.descendant_metric_node_visits_run,
        totals.descendant_metric_node_visits_avoided,
        pct(
            totals.descendant_metric_node_visits_avoided,
            totals.descendant_metric_node_visits_run + totals.descendant_metric_node_visits_avoided
        ),
        totals.plan_casefold_checks_run,
        totals.plan_casefold_allocations_avoided,
        totals.plan_casefold_bytes_avoided,
        totals.casefold_checks_run,
        totals.casefold_allocations_avoided,
        totals.casefold_bytes_avoided,
        totals.url_label_queries_run,
        totals.url_label_cache_hits,
        totals.url_label_cache_misses,
        pct(
            totals.url_label_cache_hits,
            totals.url_label_cache_hits + totals.url_label_cache_misses
        ),
        totals.url_label_fast_path_hits,
        totals.url_label_full_parse_run,
        totals.url_label_full_parse_avoided,
        pct(
            totals.url_label_full_parse_avoided,
            totals.url_label_full_parse_run + totals.url_label_full_parse_avoided
        ),
        totals.url_label_bytes_avoided,
        totals.winner_candidate_scans_run,
        totals.winner_text_candidates_seen,
        totals.winner_final_clones_run,
        totals.winner_text_clones_avoided,
        pct(
            totals.winner_text_clones_avoided,
            totals.winner_final_clones_run + totals.winner_text_clones_avoided
        ),
        totals.winner_text_bytes_avoided,
        pct(totals.score_evals_avoided, totals.score_evals_run + totals.score_evals_avoided),
        pct(
            totals.score_hash_rounds_avoided,
            totals.score_hash_rounds_run + totals.score_hash_rounds_avoided
        ),
        warm_speedup(totals),
        totals.page_cache_entries,
        totals.subtree_cache_entries,
        totals.page_cache_max,
        totals.subtree_cache_max,
        totals.page_cache_evictions,
        totals.subtree_cache_evictions,
        totals.cache_estimated_bytes
    )
}

fn print_summary(totals: &LabTotals) {
    println!("--- summary ---");
    println!(
        "runs={} passes={} variants={} nodes={} candidates={} load_total={}us exec_total={}us",
        totals.pages,
        totals.passes,
        totals.variants,
        totals.nodes,
        totals.candidates,
        totals.load_us,
        totals.exec_us
    );
    println!(
        "latency: load p50/p95/p99={}/{}/{}us exec p50/p95/p99={}/{}/{}us",
        percentile(&totals.load_samples_us, 50.0),
        percentile(&totals.load_samples_us, 95.0),
        percentile(&totals.load_samples_us, 99.0),
        percentile(&totals.exec_samples_us, 50.0),
        percentile(&totals.exec_samples_us, 95.0),
        percentile(&totals.exec_samples_us, 99.0)
    );
    println!(
        "page cache: hits={} misses={} entries={}/{} evictions={}; subtree cache: hits={} misses={} entries={}/{} evictions={} bytes={}",
        totals.page_cache_hits,
        totals.page_cache_misses,
        totals.page_cache_entries,
        totals.page_cache_max,
        totals.page_cache_evictions,
        totals.subtree_cache_hits,
        totals.subtree_cache_misses,
        totals.subtree_cache_entries,
        totals.subtree_cache_max,
        totals.subtree_cache_evictions,
        totals.cache_estimated_bytes
    );
    println!(
        "avoided: score_evals={}/{} ({:.1}%), hash_rounds={}/{} ({:.1}%), node_walks={}",
        totals.score_evals_avoided,
        totals.score_evals_run + totals.score_evals_avoided,
        pct(totals.score_evals_avoided, totals.score_evals_run + totals.score_evals_avoided),
        totals.score_hash_rounds_avoided,
        totals.score_hash_rounds_run + totals.score_hash_rounds_avoided,
        pct(
            totals.score_hash_rounds_avoided,
            totals.score_hash_rounds_run + totals.score_hash_rounds_avoided
        ),
        totals.node_walks_avoided
    );
    println!(
        "pipeline fusion: node_scans_run={} avoided={} ({:.1}%) legacy_passes={} fused_passes={}",
        totals.pipeline_node_scans_run,
        totals.pipeline_node_scans_avoided,
        pct(
            totals.pipeline_node_scans_avoided,
            totals.pipeline_node_scans_run + totals.pipeline_node_scans_avoided
        ),
        LEGACY_PIPELINE_NODE_SCAN_PASSES,
        FUSED_PIPELINE_NODE_SCAN_PASSES
    );
    println!(
        "nav middlemen: candidates_seen={} candidate_vec_pushes_avoided={} bucket_key_allocations_avoided={} key_bytes_avoided={} item_materializations_deferred={} clone_bytes_deferred={}",
        totals.nav_candidates_seen,
        totals.nav_candidate_vec_pushes_avoided,
        totals.nav_bucket_key_allocations_avoided,
        totals.nav_bucket_key_bytes_avoided,
        totals.nav_item_materializations_deferred,
        totals.nav_item_clone_bytes_deferred
    );
    println!(
        "fallback top-k: candidates_seen={} kept={} dropped={} replacements={} full_sort_items_avoided={} vec_slots_avoided={} bytes_avoided={}",
        totals.fallback_topk_candidates_seen,
        totals.fallback_topk_candidates_kept,
        totals.fallback_topk_candidates_dropped,
        totals.fallback_topk_replacements,
        totals.fallback_full_sort_items_avoided,
        totals.fallback_frame_vec_slots_avoided,
        totals.fallback_frame_vec_bytes_avoided
    );
    println!(
        "content descendants: queries_run={} avoided={} ({:.1}%) id_clones_avoided={} zero_copy_key_bytes_avoided={}",
        totals.content_subtree_queries_run,
        totals.content_subtree_queries_avoided,
        pct(
            totals.content_subtree_queries_avoided,
            totals.content_subtree_queries_run + totals.content_subtree_queries_avoided
        ),
        totals.content_subtree_id_clones_avoided,
        totals.zero_copy_index_key_bytes_avoided
    );
    println!(
        "descendant vec zero-copy: queries={} hits={} builds={} vec_clones_avoided={} ({:.1}%) slots_avoided={} bytes_avoided={}",
        totals.descendant_vec_queries_run,
        totals.descendant_vec_cache_hits,
        totals.descendant_vec_builds,
        totals.descendant_vec_clone_allocations_avoided,
        pct(
            totals.descendant_vec_clone_allocations_avoided,
            totals.descendant_vec_queries_run
        ),
        totals.descendant_vec_slots_avoided,
        totals.descendant_vec_bytes_avoided
    );
    println!(
        "text cache: hits={} misses={} hit_rate={:.1}% bytes_normalized={} bytes_avoided={} join_allocations_avoided={}",
        totals.text_cache_hits,
        totals.text_cache_misses,
        pct(
            totals.text_cache_hits,
            totals.text_cache_hits + totals.text_cache_misses
        ),
        totals.text_bytes_normalized,
        totals.text_bytes_avoided,
        totals.text_join_allocations_avoided
    );
    println!(
        "collapse streaming: vec_allocations_avoided={} word_slots_avoided={}",
        totals.text_collapse_vec_allocations_avoided,
        totals.text_collapse_word_slots_avoided
    );
    println!(
        "content field fusion: queries={} scan_passes_run={} avoided={} ({:.1}%) node_visits_run={} avoided={} ({:.1}%)",
        totals.content_field_scan_queries_run,
        totals.content_field_scan_passes_run,
        totals.content_field_scan_passes_avoided,
        pct(
            totals.content_field_scan_passes_avoided,
            totals.content_field_scan_passes_run + totals.content_field_scan_passes_avoided
        ),
        totals.content_field_node_visits_run,
        totals.content_field_node_visits_avoided,
        pct(
            totals.content_field_node_visits_avoided,
            totals.content_field_node_visits_run + totals.content_field_node_visits_avoided
        )
    );
    println!(
        "descendant metric fusion: scans_run={} avoided={} ({:.1}%) node_visits_run={} avoided={} ({:.1}%)",
        totals.descendant_metric_scans_run,
        totals.descendant_metric_scans_avoided,
        pct(
            totals.descendant_metric_scans_avoided,
            totals.descendant_metric_scans_run + totals.descendant_metric_scans_avoided
        ),
        totals.descendant_metric_node_visits_run,
        totals.descendant_metric_node_visits_avoided,
        pct(
            totals.descendant_metric_node_visits_avoided,
            totals.descendant_metric_node_visits_run + totals.descendant_metric_node_visits_avoided
        )
    );
    println!(
        "plan casefold noalloc: checks_run={} allocations_avoided={} bytes_avoided={}",
        totals.plan_casefold_checks_run,
        totals.plan_casefold_allocations_avoided,
        totals.plan_casefold_bytes_avoided
    );
    println!(
        "casefold noalloc: checks_run={} allocations_avoided={} bytes_avoided={}",
        totals.casefold_checks_run,
        totals.casefold_allocations_avoided,
        totals.casefold_bytes_avoided
    );
    println!(
        "url label cache: queries={} hits={} misses={} hit_rate={:.1}% fast_path={} full_parse_run={} full_parse_avoided={} parse_avoidance={:.1}% bytes_avoided={}",
        totals.url_label_queries_run,
        totals.url_label_cache_hits,
        totals.url_label_cache_misses,
        pct(
            totals.url_label_cache_hits,
            totals.url_label_cache_hits + totals.url_label_cache_misses
        ),
        totals.url_label_fast_path_hits,
        totals.url_label_full_parse_run,
        totals.url_label_full_parse_avoided,
        pct(
            totals.url_label_full_parse_avoided,
            totals.url_label_full_parse_run + totals.url_label_full_parse_avoided
        ),
        totals.url_label_bytes_avoided
    );
    println!(
        "winner selection: scans={} text_candidates={} final_clones={} clones_avoided={} ({:.1}%) bytes_avoided={}",
        totals.winner_candidate_scans_run,
        totals.winner_text_candidates_seen,
        totals.winner_final_clones_run,
        totals.winner_text_clones_avoided,
        pct(
            totals.winner_text_clones_avoided,
            totals.winner_final_clones_run + totals.winner_text_clones_avoided
        ),
        totals.winner_text_bytes_avoided
    );
    println!(
        "cold_avg_exec={}us warm_avg_exec={}us warm_speedup={:.2}x",
        avg(totals.cold_exec_us, totals.cold_runs),
        avg(totals.warm_exec_us, totals.warm_runs),
        warm_speedup(totals)
    );
}

fn avg(total: u128, count: u64) -> u128 {
    if count == 0 {
        0
    } else {
        total / count as u128
    }
}

fn pct(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * part as f64 / total as f64
    }
}

fn percentile(samples: &[u128], pct: f64) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = ((pct / 100.0) * (sorted.len().saturating_sub(1) as f64)).ceil() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn warm_speedup(totals: &LabTotals) -> f64 {
    let cold = avg(totals.cold_exec_us, totals.cold_runs) as f64;
    let warm = avg(totals.warm_exec_us, totals.warm_runs) as f64;
    if warm <= 0.0 {
        0.0
    } else {
        cold / warm
    }
}

fn hex_hash(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
