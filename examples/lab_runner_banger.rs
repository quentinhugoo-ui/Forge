//! BOOM/Banger lab runner.
//!
//! This is a content-addressed audit harness for Blender-like workloads:
//! import normalization, topology extraction, modifier previews, slicing, and
//! picking indexes. It intentionally runs the same pipeline twice so the log can
//! prove that identical calculations are avoided on the warm pass.

use scan::{
    atlas::Atlas,
    compute_core::{compact_hash, ComputeCacheStats, ComputeSurface},
    Hash, Store,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const VERSION: &str = "lab-runner-banger-v1";
const FRAME_60HZ_MS: f64 = 16.667;
const INTERACTION_BUDGET_MS: f64 = 50.0;
const ASSET_PAGE_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug)]
struct Config {
    mode: Mode,
    triangles: usize,
    layers: usize,
    passes: usize,
    tag: String,
    focus: String,
    cache_bytes: usize,
    store_dir: PathBuf,
    blender_bin: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Audit,
    BlenderProbe,
    Help,
}

type SourceFocusRunner = fn(&mut BangerLab, &Config, usize, &str, &[u8]) -> io::Result<Vec<u8>>;
type TopologyFocusRunner =
    fn(&mut BangerLab, &Config, usize, &str, &[u8], &[u8]) -> io::Result<Vec<u8>>;

const SOURCE_FOCUS_ROUTES: &[(&[&str], SourceFocusRunner)] = &[
    (&["kasm", "kasm-spine", "command-spine", "command-spec"], run_kasm_spine_focus),
    (&["mcp", "mcp-facade", "kasm-mcp", "mcp-spine"], run_mcp_facade_focus),
    (&["world", "world-patch", "worldpatch", "patch"], run_world_patch_focus),
    (&["hash-time", "rollback", "explain", "explain-hash"], run_hash_time_focus),
    (&["metric", "metrics", "metric-spine", "metric-hash"], run_metric_spine_focus),
    (&["program", "program-spec", "matrix", "matrix-run", "program-matrix"], run_program_matrix_focus),
    (&["compute", "compute-ir", "compute-ir-spine", "gpu-compute"], run_compute_ir_spine_focus),
    (&["skill", "skills", "skill-spec", "skill-spine"], run_skill_spine_focus),
    (&["import-hash", "source-key", "source-fingerprint"], run_import_hash_focus),
    (&["import-bounds", "bounds", "bounds-hint"], run_import_bounds_focus),
    (&["import", "import-view", "normalize", "normalize-import"], run_import_view_focus),
];

const TOPOLOGY_FOCUS_ROUTES: &[(&[&str], TopologyFocusRunner)] = &[
    (&["modifier", "modifiers", "modifier-plan"], run_modifier_plan_focus),
    (&["asset-page", "asset-pages", "asset-page-spine"], run_asset_page_spine_focus),
    (
        &["asset-residency", "asset-memory", "virtual-asset-memory", "asset-residency-spine"],
        run_asset_residency_spine_focus,
    ),
    (&["geocluster", "geo-cluster", "geocluster-spine", "meshletize"], run_geocluster_spine_focus),
];

const VIEWPORT_FOCUS_ALIASES: &[&str] = &[
    "viewport", "viewport-cache", "slicer-reuse", "gpu", "gpu-resource", "gpu-resources",
    "frame-loop", "idle-loop", "dirty-frame", "ui", "ui-render", "ui-coalesce",
    "ui-contract", "pick", "picking", "pick-handle", "render", "render-ir", "render-asset",
    "render-asset-spine",
];
const RENDER_ASSET_FOCUS_ALIASES: &[&str] =
    &["render", "render-ir", "render-asset", "render-asset-spine"];
const PICK_FOCUS_ALIASES: &[&str] = &["pick", "picking", "pick-handle"];
const UI_FOCUS_ALIASES: &[&str] = &["ui", "ui-render", "ui-coalesce", "ui-contract"];
const FRAME_LOOP_FOCUS_ALIASES: &[&str] = &["frame-loop", "idle-loop", "dirty-frame"];
const GPU_RESOURCE_FOCUS_ALIASES: &[&str] = &["gpu", "gpu-resource", "gpu-resources"];
const BANGER_PRIMARY_FOCI: &[&str] = &[
    "pipeline", "kasm-spine", "mcp-facade", "world-patch", "hash-time", "metric-spine",
    "program-matrix", "compute-ir-spine", "skill-spine", "render-asset-spine",
    "asset-page-spine", "asset-residency-spine", "geocluster-spine", "import-view",
    "import-hash", "import-bounds", "viewport", "gpu-resource", "frame-loop", "ui-coalesce",
    "pick-handle", "modifier-plan",
];

#[derive(Clone, Debug)]
struct StageRecord {
    pass: usize,
    label: String,
    stage: &'static str,
    status: &'static str,
    elapsed: Duration,
    compute_elapsed: Duration,
    input_hash: String,
    output_hash: String,
    output_bytes: usize,
    work_units: u64,
    unit: &'static str,
    cache_bytes: usize,
    evicted: usize,
    evicted_bytes: usize,
}

struct BangerLab {
    store: Store,
    atlas: Atlas,
    log_path: PathBuf,
    ram: HashMap<[u8; 32], CacheEntry>,
    ram_lru: VecDeque<[u8; 32]>,
    ram_bytes: usize,
    ram_max_bytes: usize,
    ram_evictions: usize,
    ram_evicted_bytes: usize,
    compute_stats: ComputeCacheStats,
    records: Vec<StageRecord>,
}

struct CacheEntry {
    output_hash: Hash,
    output: Vec<u8>,
    bytes: usize,
}

#[derive(Clone)]
struct Geometry {
    pos: Vec<f32>,
    nrm: Vec<f32>,
}

impl Geometry {
    fn tri_count(&self) -> usize {
        self.pos.len() / 9
    }
}

#[derive(Clone, Copy, Debug)]
struct ImportNormalizeView {
    min: [f32; 3],
    max: [f32; 3],
    center: [f32; 3],
    span: [f32; 3],
    scale: f32,
    pos_floats: u32,
    nrm_floats: u32,
    triangles: u32,
}

fn main() -> io::Result<()> {
    let config = parse_args(env::args().skip(1).collect());
    match config.mode {
        Mode::Help => {
            print_help();
            Ok(())
        }
        Mode::BlenderProbe => run_blender_probe(&config),
        Mode::Audit => run_audit(config),
    }
}

fn parse_args(args: Vec<String>) -> Config {
    let mut mode = Mode::Audit;
    let mut index = 0usize;
    if let Some(first) = args.first() {
        match first.as_str() {
            "audit" => index = 1,
            "blender-probe" | "blender_probe" | "probe-blender" => {
                mode = Mode::BlenderProbe;
                index = 1;
            }
            "help" | "-h" | "--help" => {
                mode = Mode::Help;
                index = 1;
            }
            _ => {}
        }
    }

    let mut config = Config {
        mode,
        triangles: 5_000,
        layers: 96,
        passes: 2,
        tag: "default".to_string(),
        focus: "pipeline".to_string(),
        cache_bytes: 96 * 1024 * 1024,
        store_dir: default_store_dir(),
        blender_bin: None,
    };

    while index < args.len() {
        match args[index].as_str() {
            "--triangles" => {
                if let Some(value) = args.get(index + 1).and_then(|v| v.parse::<usize>().ok()) {
                    config.triangles = value.max(1);
                }
                index += 2;
            }
            "--layers" => {
                if let Some(value) = args.get(index + 1).and_then(|v| v.parse::<usize>().ok()) {
                    config.layers = value.max(1);
                }
                index += 2;
            }
            "--passes" => {
                if let Some(value) = args.get(index + 1).and_then(|v| v.parse::<usize>().ok()) {
                    config.passes = value.max(1);
                }
                index += 2;
            }
            "--tag" => {
                if let Some(value) = args.get(index + 1) {
                    config.tag = value.clone();
                }
                index += 2;
            }
            "--focus" => {
                if let Some(value) = args.get(index + 1) {
                    config.focus = value.clone();
                }
                index += 2;
            }
            "--cache-mb" => {
                if let Some(value) = args.get(index + 1).and_then(|v| v.parse::<usize>().ok()) {
                    config.cache_bytes = value.max(1) * 1024 * 1024;
                }
                index += 2;
            }
            "--fresh" => {
                let stamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                config.tag = format!("fresh-{stamp}");
                index += 1;
            }
            "--store" => {
                if let Some(value) = args.get(index + 1) {
                    config.store_dir = PathBuf::from(value);
                }
                index += 2;
            }
            "--blender" => {
                if let Some(value) = args.get(index + 1) {
                    config.blender_bin = Some(PathBuf::from(value));
                }
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }

    config
}

fn print_help() {
    println!("BOOM/Banger content-addressed lab runner");
    println!();
    println!("Usage:");
    println!(
        "  cargo run --example lab_runner_banger -- audit [--focus {}] [--cache-mb N] [--triangles N] [--passes N] [--tag NAME]",
        BANGER_PRIMARY_FOCI.join("|")
    );
    println!("  cargo run --example lab_runner_banger -- blender-probe [--blender PATH]");
    println!();
    println!("Primary target: measure Banger's own Blender-class power, latency, and recompute avoidance.");
    println!("The Blender probe is only an optional external baseline when Blender is installed.");
    println!(
        "The audit writes JSONL proof logs under {}",
        default_store_dir().display()
    );
}

fn default_store_dir() -> PathBuf {
    if let Some(raw) = env::var_os("FORGE_STORE_DIR") {
        let trimmed = raw.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("banger-lab");
        }
    }
    if let Some(appdata) = env::var_os("APPDATA") {
        return PathBuf::from(appdata)
            .join("com.forge.ui")
            .join("forge-store")
            .join("banger-lab");
    }
    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".forge-store")
        .join("banger-lab")
}

fn focus_matches(focus: &str, aliases: &[&str]) -> bool {
    aliases.iter().any(|alias| *alias == focus)
}

fn run_source_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> Option<io::Result<Vec<u8>>> {
    SOURCE_FOCUS_ROUTES
        .iter()
        .find(|(aliases, _)| focus_matches(&config.focus, aliases))
        .map(|(_, runner)| runner(lab, config, pass, label, source_bytes))
}

fn run_topology_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    normalized: &[u8],
    topology: &[u8],
) -> Option<io::Result<Vec<u8>>> {
    TOPOLOGY_FOCUS_ROUTES
        .iter()
        .find(|(aliases, _)| focus_matches(&config.focus, aliases))
        .map(|(_, runner)| runner(lab, config, pass, label, normalized, topology))
}

fn run_audit(config: Config) -> io::Result<()> {
    fs::create_dir_all(&config.store_dir)?;
    let cas_dir = config.store_dir.join("cas");
    let atlas_path = config.store_dir.join("banger-atlas.bin");
    let log_path = config.store_dir.join("lab_runner_banger.jsonl");
    let mut lab = BangerLab {
        store: Store::open(cas_dir)?,
        atlas: Atlas::open(atlas_path)?,
        log_path,
        ram: HashMap::new(),
        ram_lru: VecDeque::new(),
        ram_bytes: 0,
        ram_max_bytes: config.cache_bytes,
        ram_evictions: 0,
        ram_evicted_bytes: 0,
        compute_stats: ComputeCacheStats::default(),
        records: Vec::new(),
    };

    println!("BANGER_LAB_START version={VERSION}");
    println!(
        "config triangles={} layers={} passes={} focus={} cache_mb={} tag={} store={}",
        config.triangles,
        config.layers,
        config.passes,
        config.focus,
        config.cache_bytes / (1024 * 1024),
        config.tag,
        config.store_dir.display()
    );

    let source_start = Instant::now();
    let source = generate_mesh(config.triangles, &config.tag);
    let source_bytes = serialize_geometry(&source);
    let source_elapsed = source_start.elapsed();
    println!(
        "SOURCE stage=synthetic_mesh elapsed_ms={:.3} triangles={} bytes={} hash={}",
        ms(source_elapsed),
        source.tri_count(),
        source_bytes.len(),
        Hash::for_blob(&source_bytes).as_hex()
    );

    for pass in 0..config.passes {
        let label = if pass == 0 { "cold-or-persisted" } else { "warm-repeat" };
        println!("PASS_BEGIN pass={} label={}", pass + 1, label);
        let pass_start = Instant::now();
        let record_start = lab.records.len();
        let final_bytes = run_pipeline(&mut lab, &config, pass + 1, label, &source_bytes)?;
        let pass_elapsed = pass_start.elapsed();
        let stage_ms: f64 = lab.records[record_start..]
            .iter()
            .map(|record| ms(record.elapsed))
            .sum();
        println!(
            "PASS_END pass={} elapsed_ms={:.3} stage_ms={:.3} harness_overhead_ms={:.3} final_bytes={} final_hash={}",
            pass + 1,
            ms(pass_elapsed),
            stage_ms,
            (ms(pass_elapsed) - stage_ms).max(0.0),
            final_bytes.len(),
            Hash::for_blob(&final_bytes).as_hex()
        );
    }
    lab.atlas.flush()?;
    print_summary(&lab.records, lab.compute_stats);
    println!("BANGER_LAB_LOG path={}", lab.log_path.display());
    Ok(())
}

fn run_pipeline(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    if let Some(result) = run_source_focus(lab, config, pass, label, source_bytes) {
        return result;
    }

    let normalize_schema = format!("normalize|{VERSION}|scale=6.0|tag={}", config.tag);
    let normalized = lab.cached_stage(
        pass,
        label,
        "normalize_import",
        b"BNORM001",
        &normalize_schema,
        source_bytes,
        config.triangles as u64,
        "triangles",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(serialize_geometry(&normalize_geometry(&geom)))
        },
    )?;

    let topology_schema = format!("topology|{VERSION}|quant=1e-5|cells=1,4,16");
    let topology = lab.cached_stage(
        pass,
        label,
        "kasm_topology",
        b"BTOPO001",
        &topology_schema,
        &normalized,
        config.triangles as u64,
        "triangles",
        || {
            let geom = deserialize_geometry(&normalized)?;
            Ok(build_topology_summary(&geom).into_bytes())
        },
    )?;

    if let Some(result) = run_topology_focus(lab, config, pass, label, &normalized, &topology) {
        return result;
    }

    let bevel_schema = format!("modifier_bevel|{VERSION}|width=0.14");
    let beveled = lab.cached_stage(
        pass,
        label,
        "modifier_bevel",
        b"BBEVL001",
        &bevel_schema,
        &normalized,
        config.triangles as u64,
        "triangles",
        || {
            let geom = deserialize_geometry(&normalized)?;
            Ok(serialize_geometry(&bevel_geometry(&geom, 0.14)))
        },
    )?;

    let solid_schema = format!("modifier_solidify|{VERSION}|thickness=0.20");
    let solid = lab.cached_stage(
        pass,
        label,
        "modifier_solidify",
        b"BSOLD001",
        &solid_schema,
        &beveled,
        (deserialize_geometry_header(&beveled)? / 9) as u64,
        "triangles",
        || {
            let geom = deserialize_geometry(&beveled)?;
            Ok(serialize_geometry(&solidify_geometry(&geom, 0.20)))
        },
    )?;

    let solid_tris = (deserialize_geometry_header(&solid)? / 9) as u64;
    let slice = lab.cached_slicer_preview(
        pass,
        label,
        &solid,
        config.layers,
        solid_tris.saturating_mul(config.layers as u64),
    )?;
    if focus_matches(&config.focus, VIEWPORT_FOCUS_ALIASES) {
        return run_viewport_focus(lab, config, pass, label, &solid, &topology, &slice, solid_tris);
    }

    let mut pick_input = Vec::with_capacity(solid.len() + topology.len() + 32);
    pick_input.extend_from_slice(&solid);
    pick_input.extend_from_slice(b"\n--topology--\n");
    pick_input.extend_from_slice(&topology);
    pick_input.extend_from_slice(b"\n--slice--\n");
    pick_input.extend_from_slice(&slice[..slice.len().min(256)]);
    let pick_schema = format!("pick_index|{VERSION}|bbox=triangles");
    lab.cached_stage(
        pass,
        label,
        "pick_index",
        b"BPICK001",
        &pick_schema,
        &pick_input,
        solid_tris,
        "triangles",
        || {
            let geom = deserialize_geometry(&solid)?;
            Ok(build_pick_index(&geom))
        },
    )
}

fn run_kasm_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let command_count = 8u64;
    let legacy_schema =
        format!("legacy_direct_tool_dispatch|{VERSION}|paths=llm,ui,mcp,slash|no_command_spec=true");
    let legacy = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_direct_tool_dispatch",
        b"BKLEG001",
        &legacy_schema,
        source_hash,
        command_count,
        "direct_paths",
        || Ok(build_legacy_direct_tool_dispatch_payload(command_count, &source_hash)),
    )?;

    let spec_schema =
        format!("kasm_command_spec|{VERSION}|all_inputs_compile_to_command_spec=true");
    let command_spec = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_command_spec",
        b"BKSPEC01",
        &spec_schema,
        source_hash,
        command_count,
        "commands",
        || Ok(build_kasm_command_spec_payload(command_count, &source_hash)),
    )?;

    let program_schema = format!("kasm_bytecode_program|{VERSION}|templates=world,asset,metric,skill");
    let program = lab.cached_stage(
        pass,
        label,
        "kasm_bytecode_program",
        b"BKBCP001",
        &program_schema,
        &command_spec,
        command_count,
        "program_templates",
        || Ok(build_kasm_bytecode_program_payload(&command_spec)),
    )?;

    let sandbox_schema =
        format!("kasm_sandbox_matrix|{VERSION}|llm_direct_fs=false|llm_direct_shell=false");
    let sandbox = lab.cached_stage(
        pass,
        label,
        "kasm_sandbox_matrix",
        b"BKSBOX01",
        &sandbox_schema,
        &command_spec,
        1,
        "sandbox_matrix",
        || Ok(build_kasm_sandbox_matrix_payload(&command_spec)),
    )?;

    let run_schema = format!("kasm_run_record|{VERSION}|records=command,program,inputs,outputs,logs");
    let mut run_input = Vec::with_capacity(command_spec.len() + program.len() + sandbox.len());
    run_input.extend_from_slice(&command_spec);
    run_input.extend_from_slice(&program);
    run_input.extend_from_slice(&sandbox);
    let run_record = lab.cached_stage(
        pass,
        label,
        "kasm_run_record",
        b"BKRUN001",
        &run_schema,
        &run_input,
        command_count,
        "run_records",
        || Ok(build_kasm_run_record_payload(&command_spec, &program, &sandbox)),
    )?;

    let proof_schema =
        format!("kasm_proof_record|{VERSION}|proof=inputs+program+sandbox+outputs+env");
    let proof = lab.cached_stage(
        pass,
        label,
        "kasm_proof_record",
        b"BKPRF001",
        &proof_schema,
        &run_record,
        command_count,
        "proof_records",
        || Ok(build_kasm_proof_record_payload(&command_spec, &program, &sandbox, &run_record)),
    )?;

    let removed_seed = hash_seed(&[Hash::for_blob(&legacy), Hash::for_blob(&proof)], command_count);
    let removed_schema =
        format!("tool_wrapper_middlemen_removed|{VERSION}|single_spine=command_spec_to_proof");
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "tool_wrapper_middlemen_removed",
        b"BKREM001",
        &removed_schema,
        Hash::for_blob(&removed_seed),
        command_count.saturating_mul(4),
        "wrappers_removed",
        || Ok(build_tool_wrapper_middlemen_removed_payload(&legacy, &proof)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&legacy),
            Hash::for_blob(&command_spec),
            Hash::for_blob(&program),
            Hash::for_blob(&sandbox),
            Hash::for_blob(&run_record),
            Hash::for_blob(&proof),
            Hash::for_blob(&removed),
        ],
        command_count,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "kasm_spine_manifest|{VERSION}|cache_bytes={}|direct_pipeline=intent->command_spec->program->sandbox->run->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_spine_manifest",
        b"BKMAN001",
        &manifest_schema,
        manifest_hash,
        7,
        "spine_artifacts",
        || {
            Ok(build_kasm_spine_manifest(
                &legacy,
                &command_spec,
                &program,
                &sandbox,
                &run_record,
                &proof,
                &removed,
            ))
        },
    )
}

fn run_mcp_facade_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let tool_count = 15u64;
    let resource_count = 10u64;
    let prompt_count = 5u64;

    let legacy_schema =
        format!("legacy_many_mcp_servers|{VERSION}|specialized_servers_bypass_kasm=true");
    let legacy = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_many_mcp_servers",
        b"BMCPLEG1",
        &legacy_schema,
        source_hash,
        tool_count.saturating_add(resource_count).saturating_add(prompt_count),
        "middlemen",
        || Ok(build_legacy_mcp_middlemen_payload(&source_hash)),
    )?;

    let facade_schema =
        format!("kasm_mcp_facade|{VERSION}|tools+resources+prompts_compile_to_kasm=true");
    let facade = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_mcp_facade",
        b"BMCPFCD1",
        &facade_schema,
        source_hash,
        tool_count.saturating_add(resource_count).saturating_add(prompt_count),
        "mcp_entries",
        || Ok(build_kasm_mcp_facade_payload(&source_hash)),
    )?;

    let tool_schema =
        format!("mcp_tool_command_specs|{VERSION}|each_tool_maps_to_slash_then_command_spec=true");
    let tool_specs = lab.cached_stage(
        pass,
        label,
        "mcp_tool_command_specs",
        b"BMCPTOOL",
        &tool_schema,
        &facade,
        tool_count,
        "tools",
        || Ok(build_mcp_tool_command_specs_payload(&facade)),
    )?;

    let resource_schema =
        format!("mcp_resource_command_specs|{VERSION}|resource_reads_are_hash_proven=true");
    let resource_specs = lab.cached_stage(
        pass,
        label,
        "mcp_resource_command_specs",
        b"BMCPRES1",
        &resource_schema,
        &facade,
        resource_count,
        "resources",
        || Ok(build_mcp_resource_command_specs_payload(&facade)),
    )?;

    let prompt_schema =
        format!("mcp_prompt_command_specs|{VERSION}|prompts_emit_command_spec_sequences=true");
    let prompt_specs = lab.cached_stage(
        pass,
        label,
        "mcp_prompt_command_specs",
        b"BMCPPRM1",
        &prompt_schema,
        &facade,
        prompt_count,
        "prompts",
        || Ok(build_mcp_prompt_command_specs_payload(&facade)),
    )?;

    let mut bytecode_input =
        Vec::with_capacity(tool_specs.len() + resource_specs.len() + prompt_specs.len());
    bytecode_input.extend_from_slice(&tool_specs);
    bytecode_input.extend_from_slice(&resource_specs);
    bytecode_input.extend_from_slice(&prompt_specs);
    let bytecode_schema =
        format!("mcp_facade_bytecode_programs|{VERSION}|no_external_tool_wrappers=true");
    let bytecode = lab.cached_stage(
        pass,
        label,
        "mcp_facade_bytecode_programs",
        b"BMCPBC01",
        &bytecode_schema,
        &bytecode_input,
        tool_count.saturating_add(resource_count).saturating_add(prompt_count),
        "bytecode_programs",
        || Ok(build_mcp_facade_bytecode_payload(&tool_specs, &resource_specs, &prompt_specs)),
    )?;

    let sandbox_schema =
        format!("mcp_facade_sandbox_matrix|{VERSION}|direct_fs=false|direct_shell=false|direct_external_tools=false");
    let sandbox = lab.cached_stage(
        pass,
        label,
        "mcp_facade_sandbox_matrix",
        b"BMCPSBX1",
        &sandbox_schema,
        &bytecode,
        1,
        "sandbox_matrix",
        || Ok(build_mcp_facade_sandbox_payload(&facade, &bytecode)),
    )?;

    let proof_schema =
        format!("mcp_facade_proof|{VERSION}|facade+entries+bytecode+sandbox_hashes=true");
    let mut proof_input = Vec::with_capacity(facade.len() + bytecode.len() + sandbox.len());
    proof_input.extend_from_slice(&facade);
    proof_input.extend_from_slice(&bytecode);
    proof_input.extend_from_slice(&sandbox);
    let proof = lab.cached_stage(
        pass,
        label,
        "mcp_facade_proof",
        b"BMCPPRF1",
        &proof_schema,
        &proof_input,
        1,
        "proof_records",
        || Ok(build_mcp_facade_proof_payload(&facade, &bytecode, &sandbox)),
    )?;

    let removed_schema =
        format!("mcp_middlemen_removed|{VERSION}|single_facade_only=true|no_specialized_servers=true");
    let removed = lab.cached_stage(
        pass,
        label,
        "mcp_middlemen_removed",
        b"BMCPREM1",
        &removed_schema,
        &legacy,
        4,
        "middlemen_removed",
        || Ok(build_mcp_middlemen_removed_payload(&legacy, &facade, &proof)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&facade),
            Hash::for_blob(&tool_specs),
            Hash::for_blob(&resource_specs),
            Hash::for_blob(&prompt_specs),
            Hash::for_blob(&bytecode),
            Hash::for_blob(&sandbox),
            Hash::for_blob(&proof),
            Hash::for_blob(&removed),
        ],
        tool_count.saturating_add(resource_count).saturating_add(prompt_count),
    );
    let manifest_schema = format!(
        "mcp_facade_manifest|{VERSION}|cache_bytes={}|direct_pipeline=mcp_tools_resources_prompts->command_spec->bytecode->sandbox->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "mcp_facade_manifest",
        b"BMCPMAN1",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        8,
        "mcp_artifacts",
        || {
            Ok(build_mcp_facade_manifest(
                &legacy,
                &facade,
                &tool_specs,
                &resource_specs,
                &prompt_specs,
                &bytecode,
                &sandbox,
                &proof,
                &removed,
            ))
        },
    )
}

fn run_world_patch_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let command_count = 10u64;
    let legacy_schema =
        format!("legacy_direct_scene_mutation|{VERSION}|mutates_scene_without_patch=true");
    let legacy = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_direct_scene_mutation",
        b"BWPLEG01",
        &legacy_schema,
        source_hash,
        command_count,
        "direct_mutations",
        || Ok(build_legacy_scene_mutation_payload(command_count, &source_hash)),
    )?;

    let spec_schema = format!("world_patch_command_spec|{VERSION}|intent=ApplyWorldPatch");
    let command_spec = lab.cached_stage_with_hash(
        pass,
        label,
        "world_patch_command_spec",
        b"BWPSPEC1",
        &spec_schema,
        source_hash,
        command_count,
        "commands",
        || Ok(build_kasm_command_spec_payload(command_count, &source_hash)),
    )?;

    let patch_schema =
        format!("world_patch_ir|{VERSION}|ops=SetProperty+AssignMesh+AssignMaterial|rollback=true");
    let world_patch = lab.cached_stage(
        pass,
        label,
        "world_patch_ir",
        b"BWPATCH1",
        &patch_schema,
        &command_spec,
        command_count,
        "world_ops",
        || Ok(build_world_patch_payload(&command_spec, command_count)),
    )?;

    let metric_schema =
        format!("world_patch_metric_expectations|{VERSION}|cpu+ram+gpu+rollback_budget=true");
    let metrics = lab.cached_stage(
        pass,
        label,
        "world_patch_metric_expectations",
        b"BWPMETR1",
        &metric_schema,
        &world_patch,
        4,
        "metrics",
        || Ok(build_world_patch_metric_payload(&world_patch)),
    )?;

    let rollback_schema = format!("world_patch_rollback|{VERSION}|inverse_ops=hash_only");
    let rollback = lab.cached_stage(
        pass,
        label,
        "world_patch_rollback",
        b"BWPROLL1",
        &rollback_schema,
        &world_patch,
        command_count,
        "rollback_ops",
        || Ok(build_world_patch_rollback_payload(&world_patch)),
    )?;

    let mut apply_input = Vec::with_capacity(world_patch.len() + metrics.len() + rollback.len());
    apply_input.extend_from_slice(&world_patch);
    apply_input.extend_from_slice(&metrics);
    apply_input.extend_from_slice(&rollback);
    let apply_schema =
        format!("world_patch_apply|{VERSION}|preview_metrics_before_apply=true|direct_mutation=false");
    let apply = lab.cached_stage(
        pass,
        label,
        "world_patch_apply",
        b"BWPAPLY1",
        &apply_schema,
        &apply_input,
        command_count,
        "applied_ops",
        || Ok(build_world_patch_apply_payload(&world_patch, &metrics, &rollback)),
    )?;

    let mut proof_input =
        Vec::with_capacity(command_spec.len() + world_patch.len() + apply.len() + rollback.len());
    proof_input.extend_from_slice(&command_spec);
    proof_input.extend_from_slice(&world_patch);
    proof_input.extend_from_slice(&apply);
    proof_input.extend_from_slice(&rollback);
    let proof_schema =
        format!("world_patch_proof|{VERSION}|outputs_include_patch_and_rollback_hash=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "world_patch_proof",
        b"BWPPROF1",
        &proof_schema,
        &proof_input,
        command_count,
        "proof_records",
        || Ok(build_world_patch_proof_payload(&command_spec, &world_patch, &apply, &rollback)),
    )?;

    let removed_seed = hash_seed(&[Hash::for_blob(&legacy), Hash::for_blob(&proof)], command_count);
    let removed_schema =
        format!("direct_scene_mutation_removed|{VERSION}|all_scene_writes_emit_world_patch=true");
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "direct_scene_mutation_removed",
        b"BWPDRM01",
        &removed_schema,
        Hash::for_blob(&removed_seed),
        command_count,
        "removed_paths",
        || Ok(build_direct_scene_mutation_removed_payload(&legacy, &proof)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&legacy),
            Hash::for_blob(&command_spec),
            Hash::for_blob(&world_patch),
            Hash::for_blob(&metrics),
            Hash::for_blob(&rollback),
            Hash::for_blob(&apply),
            Hash::for_blob(&proof),
            Hash::for_blob(&removed),
        ],
        command_count,
    );
    let manifest_schema = format!(
        "world_patch_manifest|{VERSION}|cache_bytes={}|direct_pipeline=command_spec->world_patch->metrics->apply->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "world_patch_manifest",
        b"BWPMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        8,
        "patch_artifacts",
        || {
            Ok(build_world_patch_manifest(
                &legacy,
                &command_spec,
                &world_patch,
                &metrics,
                &rollback,
                &apply,
                &proof,
                &removed,
            ))
        },
    )
}

fn run_hash_time_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let object_count = 12u64;
    let record_schema =
        format!("hash_time_run_record|{VERSION}|records=command+patch+rollback+proof");
    let run_record = lab.cached_stage_with_hash(
        pass,
        label,
        "hash_time_run_record",
        b"BHTRUN01",
        &record_schema,
        source_hash,
        object_count,
        "records",
        || Ok(build_hash_time_run_record_payload(object_count, &source_hash)),
    )?;

    let index_schema =
        format!("kasm_hash_index|{VERSION}|bounded=true|limit=512|roles=run,patch,rollback,proof");
    let hash_index = lab.cached_stage(
        pass,
        label,
        "kasm_hash_index",
        b"BHTIDX01",
        &index_schema,
        &run_record,
        object_count,
        "indexed_hashes",
        || Ok(build_hash_time_index_payload(&run_record)),
    )?;

    let explain_schema = format!("explain_hash|{VERSION}|prefix_lookup=true|llm_reads_summary_only=true");
    let explain = lab.cached_stage(
        pass,
        label,
        "explain_hash",
        b"BHTEXP01",
        &explain_schema,
        &hash_index,
        1,
        "explanations",
        || Ok(build_explain_hash_payload(&hash_index)),
    )?;

    let rollback_schema = format!("rollback_resolve|{VERSION}|target=run_or_patch_or_rollback_hash");
    let rollback = lab.cached_stage(
        pass,
        label,
        "rollback_resolve",
        b"BHTRSLV1",
        &rollback_schema,
        &hash_index,
        1,
        "rollback_targets",
        || Ok(build_rollback_resolve_payload(&hash_index)),
    )?;

    let mut apply_input = Vec::with_capacity(rollback.len() + explain.len());
    apply_input.extend_from_slice(&rollback);
    apply_input.extend_from_slice(&explain);
    let apply_schema =
        format!("rollback_apply|{VERSION}|uses_rollback_patch=true|direct_scene_write=false");
    let apply = lab.cached_stage(
        pass,
        label,
        "rollback_apply",
        b"BHTAPLY1",
        &apply_schema,
        &apply_input,
        object_count,
        "restored_ops",
        || Ok(build_rollback_apply_payload(&rollback, &explain)),
    )?;

    let mut proof_input = Vec::with_capacity(run_record.len() + hash_index.len() + apply.len());
    proof_input.extend_from_slice(&run_record);
    proof_input.extend_from_slice(&hash_index);
    proof_input.extend_from_slice(&apply);
    let proof_schema = format!("rollback_proof|{VERSION}|proves_hash_index_and_scene_restore=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "rollback_proof",
        b"BHTPRF01",
        &proof_schema,
        &proof_input,
        1,
        "proof_records",
        || Ok(build_rollback_proof_payload(&run_record, &hash_index, &apply)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&run_record),
            Hash::for_blob(&hash_index),
            Hash::for_blob(&explain),
            Hash::for_blob(&rollback),
            Hash::for_blob(&apply),
            Hash::for_blob(&proof),
        ],
        object_count,
    );
    let manifest_schema = format!(
        "hash_time_manifest|{VERSION}|cache_bytes={}|direct_pipeline=run_record->hash_index->explain->rollback->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "hash_time_manifest",
        b"BHTMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        6,
        "hash_time_artifacts",
        || {
            Ok(build_hash_time_manifest(
                &run_record,
                &hash_index,
                &explain,
                &rollback,
                &apply,
                &proof,
            ))
        },
    )
}

fn run_metric_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let metric_count = 6u64;
    let spec_schema =
        format!("kasm_metric_spec|{VERSION}|metrics=patch_ops,rollback,scene,draw,ram,latency");
    let metric_spec = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_metric_spec",
        b"BMSPEC01",
        &spec_schema,
        source_hash,
        metric_count,
        "metric_specs",
        || Ok(build_metric_spec_payload(metric_count, &source_hash)),
    )?;

    let program_schema = format!("metric_evaluator_program|{VERSION}|metrics_are_kasm_programs=true");
    let evaluator = lab.cached_stage(
        pass,
        label,
        "metric_evaluator_program",
        b"BMEVAL01",
        &program_schema,
        &metric_spec,
        metric_count,
        "evaluator_programs",
        || Ok(build_metric_evaluator_payload(&metric_spec)),
    )?;

    let target_schema =
        format!("metric_target_snapshot|{VERSION}|target=world_patch_or_output_hash");
    let target = lab.cached_stage(
        pass,
        label,
        "metric_target_snapshot",
        b"BMTARG01",
        &target_schema,
        source_bytes,
        config.triangles as u64,
        "triangles",
        || Ok(build_metric_target_snapshot_payload(source_bytes, &source_hash)),
    )?;

    let mut record_input = Vec::with_capacity(metric_spec.len() + evaluator.len() + target.len());
    record_input.extend_from_slice(&metric_spec);
    record_input.extend_from_slice(&evaluator);
    record_input.extend_from_slice(&target);
    let record_schema = format!("kasm_metric_record|{VERSION}|records=value+unit+threshold");
    let metric_record = lab.cached_stage(
        pass,
        label,
        "kasm_metric_record",
        b"BMREC001",
        &record_schema,
        &record_input,
        metric_count,
        "metric_records",
        || Ok(build_metric_record_payload(&metric_spec, &evaluator, &target)),
    )?;

    let attach_schema = format!("metric_hashes_attached|{VERSION}|run_and_proof_metric_hashes_nonempty=true");
    let attach = lab.cached_stage(
        pass,
        label,
        "metric_hashes_attached",
        b"BMATCH01",
        &attach_schema,
        &metric_record,
        metric_count,
        "metric_hashes",
        || Ok(build_metric_hashes_attached_payload(&metric_record)),
    )?;

    let proof_schema = format!("metric_proof|{VERSION}|proof_includes_metric_hashes=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "metric_proof",
        b"BMPRF001",
        &proof_schema,
        &attach,
        metric_count,
        "proof_metric_hashes",
        || Ok(build_metric_proof_payload(&metric_spec, &metric_record, &attach)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&metric_spec),
            Hash::for_blob(&evaluator),
            Hash::for_blob(&target),
            Hash::for_blob(&metric_record),
            Hash::for_blob(&attach),
            Hash::for_blob(&proof),
        ],
        metric_count,
    );
    let manifest_schema = format!(
        "metric_spine_manifest|{VERSION}|cache_bytes={}|direct_pipeline=metric_spec->evaluator_program->metric_record->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "metric_spine_manifest",
        b"BMMAN001",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        6,
        "metric_artifacts",
        || {
            Ok(build_metric_spine_manifest(
                &metric_spec,
                &evaluator,
                &target,
                &metric_record,
                &attach,
                &proof,
            ))
        },
    )
}

fn run_program_matrix_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let program_count = 4u64;
    let variant_count = 128u64.min(config.triangles.max(1) as u64);
    let spec_schema =
        format!("kasm_program_spec|{VERSION}|source+bytecode+schemas+sandbox+budget_hashes=true");
    let program_spec = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_program_spec",
        b"BPMSPEC1",
        &spec_schema,
        source_hash,
        program_count,
        "program_specs",
        || Ok(build_program_spec_payload(program_count, &source_hash)),
    )?;

    let bytecode_schema =
        format!("program_bytecode_template|{VERSION}|programs_compile_to_bytecode=true");
    let bytecode = lab.cached_stage(
        pass,
        label,
        "program_bytecode_template",
        b"BPMBC001",
        &bytecode_schema,
        &program_spec,
        program_count,
        "bytecode_templates",
        || Ok(build_program_bytecode_template_payload(&program_spec)),
    )?;

    let mut run_input = Vec::with_capacity(program_spec.len() + bytecode.len() + source_bytes.len().min(256));
    run_input.extend_from_slice(&program_spec);
    run_input.extend_from_slice(&bytecode);
    run_input.extend_from_slice(&source_bytes[..source_bytes.len().min(256)]);
    let run_schema =
        format!("program_run_record|{VERSION}|input_hashes+output_hashes+metric_hashes=true");
    let program_run = lab.cached_stage(
        pass,
        label,
        "program_run_record",
        b"BPMRUN01",
        &run_schema,
        &run_input,
        program_count,
        "program_runs",
        || Ok(build_program_run_record_payload(&program_spec, &bytecode, source_bytes)),
    )?;

    let matrix_schema =
        format!("matrix_run_spec|{VERSION}|variants_bounded=true|sandbox_matrix_per_variant=true");
    let matrix_spec = lab.cached_stage(
        pass,
        label,
        "matrix_run_spec",
        b"BPMMAT01",
        &matrix_schema,
        &program_run,
        variant_count,
        "matrix_variants",
        || Ok(build_matrix_run_spec_payload(&program_spec, &program_run, variant_count)),
    )?;

    let variants_schema = format!("matrix_variant_hashes|{VERSION}|raw_variants_stay_hashed=true");
    let variants = lab.cached_stage(
        pass,
        label,
        "matrix_variant_hashes",
        b"BPMVAR01",
        &variants_schema,
        &matrix_spec,
        variant_count,
        "variant_hashes",
        || Ok(build_matrix_variant_hashes_payload(&matrix_spec, variant_count)),
    )?;

    let metric_schema =
        format!("matrix_metric_set|{VERSION}|metrics_are_program_hashes=true|llm_reads_scores_only=true");
    let metric_set = lab.cached_stage(
        pass,
        label,
        "matrix_metric_set",
        b"BPMMET01",
        &metric_schema,
        &variants,
        5,
        "metric_specs",
        || Ok(build_matrix_metric_set_payload(&program_spec, &variants)),
    )?;

    let mut select_input = Vec::with_capacity(variants.len() + metric_set.len());
    select_input.extend_from_slice(&variants);
    select_input.extend_from_slice(&metric_set);
    let select_schema = format!("matrix_select_top|{VERSION}|top8_by_metric_score=true");
    let top = lab.cached_stage(
        pass,
        label,
        "matrix_select_top",
        b"BPMTOP01",
        &select_schema,
        &select_input,
        8,
        "top_variants",
        || Ok(build_matrix_select_top_payload(&variants, &metric_set)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&program_spec),
            Hash::for_blob(&bytecode),
            Hash::for_blob(&program_run),
            Hash::for_blob(&matrix_spec),
            Hash::for_blob(&variants),
            Hash::for_blob(&metric_set),
            Hash::for_blob(&top),
        ],
        variant_count,
    );
    let manifest_schema = format!(
        "program_matrix_manifest|{VERSION}|cache_bytes={}|direct_pipeline=program_spec->program_run->matrix_variants->metrics->top_selection",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "program_matrix_manifest",
        b"BPMMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        7,
        "program_matrix_artifacts",
        || {
            Ok(build_program_matrix_manifest(
                &program_spec,
                &bytecode,
                &program_run,
                &matrix_spec,
                &variants,
                &metric_set,
                &top,
            ))
        },
    )
}

fn run_compute_ir_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let work_items = config.triangles.max(1) as u64;
    let program_schema =
        format!("kasm_compute_program|{VERSION}|shader+buffers+dispatch+backend+budget_hashes=true");
    let compute_program = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_compute_program",
        b"BCMPROG1",
        &program_schema,
        source_hash,
        work_items,
        "work_items",
        || Ok(build_compute_program_payload(&source_hash, work_items)),
    )?;

    let buffers_schema =
        format!("compute_buffer_bindings|{VERSION}|input_output_buffers_are_hashes=true");
    let buffers = lab.cached_stage(
        pass,
        label,
        "compute_buffer_bindings",
        b"BCMBUF01",
        &buffers_schema,
        &compute_program,
        work_items,
        "buffer_bindings",
        || Ok(build_compute_buffer_bindings_payload(&compute_program, source_bytes.len() as u64)),
    )?;

    let dispatch_schema =
        format!("compute_dispatch_spec|{VERSION}|workgroups_bounded=true|no_free_shader=true");
    let dispatch = lab.cached_stage(
        pass,
        label,
        "compute_dispatch_spec",
        b"BCMDISP1",
        &dispatch_schema,
        &buffers,
        work_items,
        "dispatch_items",
        || Ok(build_compute_dispatch_payload(&compute_program, &buffers, work_items)),
    )?;

    let sandbox_schema =
        format!("compute_sandbox_matrix|{VERSION}|direct_renderer=false|direct_fs=false|bytecode_only=true");
    let sandbox = lab.cached_stage(
        pass,
        label,
        "compute_sandbox_matrix",
        b"BCMSBOX1",
        &sandbox_schema,
        &dispatch,
        1,
        "sandbox",
        || Ok(build_compute_sandbox_payload(&compute_program, &dispatch)),
    )?;

    let mut run_input = Vec::with_capacity(compute_program.len() + buffers.len() + dispatch.len() + sandbox.len());
    run_input.extend_from_slice(&compute_program);
    run_input.extend_from_slice(&buffers);
    run_input.extend_from_slice(&dispatch);
    run_input.extend_from_slice(&sandbox);
    let run_schema =
        format!("compute_run_record|{VERSION}|output_buffer_hashes+metric_hashes=true");
    let compute_run = lab.cached_stage(
        pass,
        label,
        "compute_run_record",
        b"BCMRUN01",
        &run_schema,
        &run_input,
        work_items,
        "compute_work",
        || Ok(build_compute_run_record_payload(&compute_program, &buffers, &dispatch, &sandbox)),
    )?;

    let metric_schema =
        format!("compute_metric_records|{VERSION}|dispatch_count+buffer_bytes+vram_cost=true");
    let metrics = lab.cached_stage(
        pass,
        label,
        "compute_metric_records",
        b"BCMMET01",
        &metric_schema,
        &compute_run,
        3,
        "metric_records",
        || Ok(build_compute_metric_records_payload(&compute_program, &buffers, &compute_run)),
    )?;

    let proof_schema =
        format!("compute_proof_record|{VERSION}|program+sandbox+outputs+metrics+env=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "compute_proof_record",
        b"BCMPROOF",
        &proof_schema,
        &metrics,
        1,
        "proof_records",
        || Ok(build_compute_proof_payload(&compute_program, &sandbox, &compute_run, &metrics)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&compute_program),
            Hash::for_blob(&buffers),
            Hash::for_blob(&dispatch),
            Hash::for_blob(&sandbox),
            Hash::for_blob(&compute_run),
            Hash::for_blob(&metrics),
            Hash::for_blob(&proof),
        ],
        work_items,
    );
    let manifest_schema = format!(
        "compute_ir_manifest|{VERSION}|cache_bytes={}|direct_pipeline=compute_program->buffers->dispatch->sandbox->run->metrics->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "compute_ir_manifest",
        b"BCMMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        7,
        "compute_artifacts",
        || {
            Ok(build_compute_ir_manifest(
                &compute_program,
                &buffers,
                &dispatch,
                &sandbox,
                &compute_run,
                &metrics,
                &proof,
            ))
        },
    )
}

fn run_skill_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let skill_count = 3u64;
    let spec_schema =
        format!("kasm_skill_spec|{VERSION}|skills_are_templates_not_plugins=true|versioned=true");
    let skill_spec = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_skill_spec",
        b"BSSPEC01",
        &spec_schema,
        source_hash,
        skill_count,
        "skill_specs",
        || Ok(build_skill_spec_payload(skill_count, &source_hash)),
    )?;

    let graph_schema =
        format!("skill_program_graph|{VERSION}|graph_nodes_are_program_hashes=true");
    let program_graph = lab.cached_stage(
        pass,
        label,
        "skill_program_graph",
        b"BSGRAPH1",
        &graph_schema,
        &skill_spec,
        skill_count,
        "program_nodes",
        || Ok(build_skill_program_graph_payload(&skill_spec)),
    )?;

    let metric_schema =
        format!("skill_metric_set|{VERSION}|metrics_compile_to_programs=true");
    let metric_set = lab.cached_stage(
        pass,
        label,
        "skill_metric_set",
        b"BSMET001",
        &metric_schema,
        &program_graph,
        4,
        "metric_specs",
        || Ok(build_skill_metric_set_payload(&skill_spec, &program_graph)),
    )?;

    let test_schema =
        format!("skill_test_set|{VERSION}|deterministic_replay+proof+budget=true");
    let test_set = lab.cached_stage(
        pass,
        label,
        "skill_test_set",
        b"BSTEST01",
        &test_schema,
        &metric_set,
        3,
        "skill_tests",
        || Ok(build_skill_test_set_payload(&skill_spec, &metric_set)),
    )?;

    let mut run_input = Vec::with_capacity(
        skill_spec.len() + program_graph.len() + metric_set.len() + test_set.len(),
    );
    run_input.extend_from_slice(&skill_spec);
    run_input.extend_from_slice(&program_graph);
    run_input.extend_from_slice(&metric_set);
    run_input.extend_from_slice(&test_set);
    let run_schema =
        format!("skill_run_record|{VERSION}|output=world_patch+metric_report+proof");
    let skill_run = lab.cached_stage(
        pass,
        label,
        "skill_run_record",
        b"BSRUN001",
        &run_schema,
        &run_input,
        skill_count,
        "skill_runs",
        || Ok(build_skill_run_record_payload(&skill_spec, &program_graph, &metric_set, &test_set)),
    )?;

    let proof_schema =
        format!("skill_proof_record|{VERSION}|inputs+program_graph+metrics+tests+env=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "skill_proof_record",
        b"BSPROOF1",
        &proof_schema,
        &skill_run,
        1,
        "proof_records",
        || Ok(build_skill_proof_record_payload(&skill_spec, &program_graph, &skill_run)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&skill_spec),
            Hash::for_blob(&program_graph),
            Hash::for_blob(&metric_set),
            Hash::for_blob(&test_set),
            Hash::for_blob(&skill_run),
            Hash::for_blob(&proof),
        ],
        skill_count,
    );
    let manifest_schema = format!(
        "skill_spine_manifest|{VERSION}|cache_bytes={}|direct_pipeline=skill_spec->program_graph->test_set->skill_run->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "skill_spine_manifest",
        b"BSMAN001",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        6,
        "skill_artifacts",
        || {
            Ok(build_skill_spine_manifest(
                &skill_spec,
                &program_graph,
                &metric_set,
                &test_set,
                &skill_run,
                &proof,
            ))
        },
    )
}

fn run_import_view_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let fingerprint_schema = format!(
        "import_source_fingerprint|{VERSION}|content_hash=true|bytes={}",
        source_bytes.len()
    );
    let fingerprint = lab.cached_stage_with_hash(
        pass,
        label,
        "import_source_fingerprint",
        b"BNFP0001",
        &fingerprint_schema,
        source_hash,
        config.triangles as u64,
        "triangles_hashed",
        || Ok(build_import_source_fingerprint_payload(source_bytes, &source_hash)),
    )?;

    let view_schema = format!("import_normalize_view|{VERSION}|scale=6.0|output=compact-view");
    let view = lab.cached_stage(
        pass,
        label,
        "import_normalize_view",
        b"BNVIEW01",
        &view_schema,
        source_bytes,
        config.triangles as u64,
        "triangles_scanned",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(build_import_normalize_view_payload(&geom))
        },
    )?;

    let legacy_schema =
        format!("legacy_normalize_materialize|{VERSION}|simulate_full_buffer_copy=true");
    let legacy = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_normalize_materialize",
        b"BNLEG001",
        &legacy_schema,
        source_hash,
        config.triangles as u64,
        "triangles_materialized",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(build_legacy_normalize_materialize_payload(&geom))
        },
    )?;

    let view_hash = Hash::for_blob(&view);
    let removed_schema =
        format!("normalized_buffer_write_removed|{VERSION}|store_view_instead_of_full_mesh=true");
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "normalized_buffer_write_removed",
        b"BNREM001",
        &removed_schema,
        view_hash,
        (config.triangles as u64).saturating_mul(18),
        "float_writes_removed",
        || Ok(build_normalized_buffer_removed_payload(&view, &legacy)),
    )?;

    let sample_seed = hash_seed(&[source_hash, view_hash], 64);
    let sample_hash = Hash::for_blob(&sample_seed);
    let sample_schema =
        format!("import_view_position_sample|{VERSION}|sample_vertices=64|no_full_buffer=true");
    let sample = lab.cached_stage_with_hash(
        pass,
        label,
        "import_view_position_sample",
        b"BNSAMP01",
        &sample_schema,
        sample_hash,
        64,
        "sample_vertices",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            let parsed = parse_import_normalize_view_payload(&view)?;
            Ok(build_import_view_position_sample_payload(&geom, &parsed, 64))
        },
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&fingerprint),
            view_hash,
            Hash::for_blob(&legacy),
            Hash::for_blob(&removed),
            Hash::for_blob(&sample),
        ],
        config.triangles as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "import_view_manifest|{VERSION}|cache_bytes={}|direct_pipeline=source->view->sample",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "import_view_manifest",
        b"BNMAN001",
        &manifest_schema,
        manifest_hash,
        5,
        "import_artifacts",
        || Ok(build_import_view_manifest(&fingerprint, &view, &legacy, &removed, &sample)),
    )
}

fn run_import_hash_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let legacy_schema =
        format!("legacy_float_fingerprint_scan|{VERSION}|hash=pos+nrm_after_parse");
    let legacy = lab.cached_stage(
        pass,
        label,
        "legacy_float_fingerprint_scan",
        b"BHLEG001",
        &legacy_schema,
        source_bytes,
        (config.triangles as u64).saturating_mul(6),
        "vertex_float_hashes",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(build_legacy_float_fingerprint_payload(&geom))
        },
    )?;

    let carried_schema = format!(
        "importer_carried_source_key|{VERSION}|reader_hash=true|parser=synthetic-boom"
    );
    let carried = lab.cached_stage_with_hash(
        pass,
        label,
        "importer_carried_source_key",
        b"BHCAR001",
        &carried_schema,
        source_hash,
        source_bytes.len() as u64,
        "source_bytes_already_read",
        || Ok(build_importer_carried_source_key_payload(source_bytes, &source_hash)),
    )?;

    let removed_seed = hash_seed(&[Hash::for_blob(&legacy), Hash::for_blob(&carried)], 1);
    let removed_hash = Hash::for_blob(&removed_seed);
    let removed_schema =
        format!("float_fingerprint_scan_removed|{VERSION}|source_hash_substitutes_buffer_scan");
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "float_fingerprint_scan_removed",
        b"BHREM001",
        &removed_schema,
        removed_hash,
        (config.triangles as u64).saturating_mul(18),
        "float_reads_avoided",
        || Ok(build_float_fingerprint_removed_payload(&legacy, &carried)),
    )?;

    let view_schema =
        format!("normalize_view_from_carried_key|{VERSION}|bounds_scan=true|fingerprint_scan=false");
    let view_input = Hash::for_blob(&hash_seed(&[source_hash, Hash::for_blob(&carried)], 2));
    let view = lab.cached_stage_with_hash(
        pass,
        label,
        "normalize_view_from_carried_key",
        b"BHVIEW01",
        &view_schema,
        view_input,
        config.triangles as u64,
        "triangles_scanned_for_bounds",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(build_import_normalize_view_payload(&geom))
        },
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&legacy),
            Hash::for_blob(&carried),
            Hash::for_blob(&removed),
            Hash::for_blob(&view),
        ],
        config.triangles as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "import_hash_manifest|{VERSION}|cache_bytes={}|direct_pipeline=reader_hash->normalize_view",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "import_hash_manifest",
        b"BHMAN001",
        &manifest_schema,
        manifest_hash,
        4,
        "hash_artifacts",
        || Ok(build_import_hash_manifest(&legacy, &carried, &removed, &view)),
    )
}

fn run_import_bounds_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    source_bytes: &[u8],
) -> io::Result<Vec<u8>> {
    let source_hash = Hash::for_blob(source_bytes);
    let legacy_schema =
        format!("legacy_bounds_rescan|{VERSION}|separate_pass_after_parse=true");
    let legacy_bounds = lab.cached_stage(
        pass,
        label,
        "legacy_bounds_rescan",
        b"BBLEG001",
        &legacy_schema,
        source_bytes,
        config.triangles as u64,
        "triangles_rescanned",
        || {
            let geom = deserialize_geometry(source_bytes)?;
            Ok(build_import_normalize_view_payload(&geom))
        },
    )?;

    let carried_schema =
        format!("importer_carried_bounds|{VERSION}|bounds_tracked_during_parse=true");
    let carried_bounds = lab.cached_stage_with_hash(
        pass,
        label,
        "importer_carried_bounds",
        b"BBCAR001",
        &carried_schema,
        Hash::for_blob(&legacy_bounds),
        config.triangles as u64,
        "triangles_already_seen",
        || Ok(build_importer_carried_bounds_payload(&legacy_bounds)),
    )?;

    let normalize_schema =
        format!("normalize_view_from_carried_bounds|{VERSION}|no_position_rescan=true");
    let normalize_input = Hash::for_blob(&hash_seed(&[source_hash, Hash::for_blob(&carried_bounds)], 3));
    let view = lab.cached_stage_with_hash(
        pass,
        label,
        "normalize_view_from_carried_bounds",
        b"BBVIEW01",
        &normalize_schema,
        normalize_input,
        1,
        "bounds_records",
        || Ok(build_normalize_view_from_carried_bounds_payload(&carried_bounds)),
    )?;

    let removed_seed = hash_seed(&[Hash::for_blob(&legacy_bounds), Hash::for_blob(&view)], 4);
    let removed_schema =
        format!("bounds_rescan_removed|{VERSION}|parse_updates_bounds_incrementally");
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "bounds_rescan_removed",
        b"BBREM001",
        &removed_schema,
        Hash::for_blob(&removed_seed),
        config.triangles as u64,
        "triangles_not_rescanned",
        || Ok(build_bounds_rescan_removed_payload(&legacy_bounds, &carried_bounds)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&legacy_bounds),
            Hash::for_blob(&carried_bounds),
            Hash::for_blob(&view),
            Hash::for_blob(&removed),
        ],
        config.triangles as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "import_bounds_manifest|{VERSION}|cache_bytes={}|direct_pipeline=parse->carried_bounds->normalize_view",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "import_bounds_manifest",
        b"BBMAN001",
        &manifest_schema,
        manifest_hash,
        4,
        "bounds_artifacts",
        || Ok(build_import_bounds_manifest(&legacy_bounds, &carried_bounds, &view, &removed)),
    )
}

fn run_viewport_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    solid: &[u8],
    topology: &[u8],
    slice_manifest: &[u8],
    solid_tris: u64,
) -> io::Result<Vec<u8>> {
    let solid_hash = Hash::for_blob(solid);
    let topology_hash = Hash::for_blob(topology);
    let slice_hash = Hash::for_blob(slice_manifest);
    let render_schema = format!("render_passes|{VERSION}|array=3|mirror=x|matrix=inline");
    let render_passes = lab.cached_stage_with_hash(
        pass,
        label,
        "render_passes_with_matrices",
        b"BRPASS01",
        &render_schema,
        solid_hash,
        6,
        "passes",
        || Ok(build_render_pass_matrix_payload(3, true)),
    )?;
    let render_hash = Hash::for_blob(&render_passes);
    let bounds_seed = hash_seed(&[solid_hash, render_hash], solid_tris);
    let bounds_hash = Hash::for_blob(&bounds_seed);
    let bounds_schema = format!("world_bounds|{VERSION}|render_pass_matrices=true");
    let world_bounds = lab.cached_stage_with_hash(
        pass,
        label,
        "world_bounds",
        b"BWBNDS01",
        &bounds_schema,
        bounds_hash,
        solid_tris.saturating_mul(render_pass_count(&render_passes) as u64),
        "triangle_pass_bounds",
        || {
            let geom = deserialize_geometry(solid)?;
            Ok(build_world_bounds_payload(&geom, &render_passes))
        },
    )?;
    let world_bounds_hash = Hash::for_blob(&world_bounds);

    let preview_seed = hash_seed(&[solid_hash, render_hash, world_bounds_hash, slice_hash], config.layers as u64);
    let preview_hash = Hash::for_blob(&preview_seed);
    let preview_schema = format!(
        "slicer_preview_reuse_gate|{VERSION}|layers={}|region=none|workflow=print",
        config.layers
    );
    let preview_gate = lab.cached_stage_with_hash(
        pass,
        label,
        "slicer_preview_reuse_gate",
        b"BSPRG001",
        &preview_schema,
        preview_hash,
        config.layers as u64,
        "layers",
        || Ok(build_preview_reuse_payload(config.layers, &slice_hash, &render_hash)),
    )?;

    if focus_matches(&config.focus, RENDER_ASSET_FOCUS_ALIASES) {
        return run_render_asset_spine_focus(
            lab,
            config,
            pass,
            label,
            solid_hash,
            render_hash,
            world_bounds_hash,
            slice_hash,
            &render_passes,
            estimate_slicer_upload_bytes(slice_manifest),
            solid.len(),
            solid_tris,
        );
    }

    if focus_matches(&config.focus, PICK_FOCUS_ALIASES) {
        return run_pick_handle_focus(
            lab,
            config,
            pass,
            label,
            solid,
            solid_hash,
            render_hash,
            &render_passes,
            solid_tris,
        );
    }

    let pick_seed = hash_seed(&[solid_hash, topology_hash, render_hash], solid_tris);
    let pick_hash = Hash::for_blob(&pick_seed);
    let pick_schema = format!("screen_pick_index|{VERSION}|broadphase=cell-bins|no-edge-filter-loop");
    let screen_pick = lab.cached_stage_with_hash(
        pass,
        label,
        "screen_pick_index",
        b"BSPICK01",
        &pick_schema,
        pick_hash,
        solid_tris,
        "triangles",
        || {
            let geom = deserialize_geometry(solid)?;
            Ok(build_screen_pick_index(&geom, &render_passes))
        },
    )?;

    let overlay_seed = hash_seed(
        &[Hash::for_blob(&screen_pick), Hash::for_blob(&preview_gate), render_hash],
        96,
    );
    let overlay_hash = Hash::for_blob(&overlay_seed);
    let overlay_schema = format!("selection_overlay_projection|{VERSION}|cached_vertex_map=true");
    let overlay = lab.cached_stage_with_hash(
        pass,
        label,
        "selection_overlay_projection",
        b"BOVRLY01",
        &overlay_schema,
        overlay_hash,
        96,
        "selection_nodes",
        || Ok(build_selection_overlay_projection(&screen_pick, &render_passes)),
    )?;
    if focus_matches(&config.focus, UI_FOCUS_ALIASES) {
        run_ui_coalesce_focus(
            lab,
            config,
            pass,
            label,
            render_hash,
            world_bounds_hash,
            &screen_pick,
            &overlay,
        )
    } else if focus_matches(&config.focus, FRAME_LOOP_FOCUS_ALIASES) {
        run_frame_loop_focus(
            lab,
            config,
            pass,
            label,
            solid_hash,
            render_hash,
            world_bounds_hash,
            &render_passes,
            &screen_pick,
            &overlay,
            solid_tris,
        )
    } else if focus_matches(&config.focus, GPU_RESOURCE_FOCUS_ALIASES) {
        run_gpu_resource_focus(
            lab,
            pass,
            label,
            solid_hash,
            render_hash,
            slice_hash,
            solid.len(),
            estimate_slicer_upload_bytes(slice_manifest),
            &overlay,
        )
    } else {
        Ok(overlay)
    }
}

fn run_render_asset_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    solid_hash: Hash,
    render_hash: Hash,
    world_bounds_hash: Hash,
    slice_hash: Hash,
    render_passes: &[u8],
    slicer_upload_bytes: usize,
    display_bytes: usize,
    solid_tris: u64,
) -> io::Result<Vec<u8>> {
    let asset_seed = hash_seed(&[solid_hash, slice_hash, render_hash], display_bytes as u64);
    let asset_hash = Hash::for_blob(&asset_seed);
    let page_count = asset_page_count(display_bytes)
        .saturating_add(asset_page_count(slicer_upload_bytes))
        .saturating_add(2);
    let asset_schema =
        format!("kasm_asset_pages|{VERSION}|store=single|page_bytes={ASSET_PAGE_BYTES}|pages={page_count}");
    let asset_pages = lab.cached_stage_with_hash(
        pass,
        label,
        "kasm_asset_pages",
        b"BRAPAGE1",
        &asset_schema,
        asset_hash,
        page_count as u64,
        "asset_pages",
        || Ok(build_asset_pages_payload(&solid_hash, &slice_hash, display_bytes, slicer_upload_bytes)),
    )?;

    let residency_schema =
        format!("asset_residency_table|{VERSION}|states=WarmRam+HotVram+Evictable+Pinned");
    let residency = lab.cached_stage(
        pass,
        label,
        "asset_residency_table",
        b"BRARES01",
        &residency_schema,
        &asset_pages,
        4,
        "asset_pages",
        || Ok(build_asset_residency_table_payload(&asset_pages)),
    )?;

    let mut ir_input = Vec::with_capacity(asset_pages.len() + residency.len() + render_passes.len());
    ir_input.extend_from_slice(&asset_pages);
    ir_input.extend_from_slice(&residency);
    ir_input.extend_from_slice(render_passes);
    let ir_schema =
        format!("kasm_render_ir|{VERSION}|scene_hash+entity_soa+asset_pages+render_mode=lit");
    let render_ir = lab.cached_stage(
        pass,
        label,
        "kasm_render_ir",
        b"BRIR0001",
        &ir_schema,
        &ir_input,
        solid_tris.saturating_mul(render_pass_count(render_passes) as u64),
        "render_instances",
        || Ok(build_render_ir_payload(&asset_pages, &residency, render_passes, &world_bounds_hash)),
    )?;

    let frame_schema =
        format!("render_projection_frame|{VERSION}|canvas_projects_render_ir_only=true");
    let frame = lab.cached_stage(
        pass,
        label,
        "render_projection_frame",
        b"BRFRAME1",
        &frame_schema,
        &render_ir,
        render_pass_count(render_passes).max(1) as u64,
        "draw_passes",
        || Ok(build_render_projection_frame_payload(&render_ir, &render_hash)),
    )?;

    let proof_schema =
        format!("render_asset_proof|{VERSION}|outputs_include_render_ir_and_asset_pages=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "render_asset_proof",
        b"BRAPRF01",
        &proof_schema,
        &frame,
        1,
        "proof_records",
        || Ok(build_render_asset_proof_payload(&render_ir, &asset_pages, &frame)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&asset_pages),
            Hash::for_blob(&residency),
            Hash::for_blob(&render_ir),
            Hash::for_blob(&frame),
            Hash::for_blob(&proof),
        ],
        solid_tris,
    );
    let manifest_schema = format!(
        "render_asset_manifest|{VERSION}|cache_bytes={}|direct_pipeline=asset_pages->residency->render_ir->canvas_projection->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "render_asset_manifest",
        b"BRAMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        5,
        "render_asset_artifacts",
        || Ok(build_render_asset_manifest(&asset_pages, &residency, &render_ir, &frame, &proof)),
    )
}

fn run_asset_page_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    normalized: &[u8],
    topology: &[u8],
) -> io::Result<Vec<u8>> {
    let base_hash = Hash::for_blob(normalized);
    let topology_hash = Hash::for_blob(topology);
    let base_tris = (deserialize_geometry_header(normalized)? / 9) as u64;
    let modifier_bytes = estimate_modifier_output_bytes(base_tris);
    let slicer_bytes = estimate_derived_slicer_bytes(base_tris, config.layers);
    let modifier_pages_count = asset_page_count(modifier_bytes).saturating_add(1);
    let slicer_pages_count = asset_page_count(slicer_bytes).saturating_add(1);
    let plan_seed = hash_seed(&[base_hash, topology_hash], base_tris);
    let plan_hash = Hash::for_blob(&plan_seed);

    let plan_schema = format!(
        "derived_modifier_asset_pages|{VERSION}|ops=bevel,solidify|page_bytes={ASSET_PAGE_BYTES}|no_full_mesh_blob=true"
    );
    let modifier_pages = lab.cached_stage_with_hash(
        pass,
        label,
        "derived_modifier_asset_pages",
        b"BAPMOD01",
        &plan_schema,
        plan_hash,
        modifier_pages_count as u64,
        "asset_pages",
        || Ok(build_modifier_asset_pages_payload(&base_hash, &topology_hash, base_tris)),
    )?;

    let slicer_seed = hash_seed(&[Hash::for_blob(&modifier_pages), topology_hash], config.layers as u64);
    let slicer_hash = Hash::for_blob(&slicer_seed);
    let slicer_schema = format!(
        "derived_slicer_asset_pages|{VERSION}|layers={}|segments_hash_only=true|no_layer_blobs=true",
        config.layers
    );
    let slicer_pages = lab.cached_stage_with_hash(
        pass,
        label,
        "derived_slicer_asset_pages",
        b"BAPSLI01",
        &slicer_schema,
        slicer_hash,
        slicer_pages_count as u64,
        "asset_pages",
        || Ok(build_slicer_asset_pages_payload(&modifier_pages, config.layers, base_tris)),
    )?;

    let pack_seed = hash_seed(
        &[Hash::for_blob(&modifier_pages), Hash::for_blob(&slicer_pages)],
        config.cache_bytes as u64,
    );
    let pack_schema = format!(
        "asset_page_pack|{VERSION}|single_store=true|dedup_by_source_hash=true|page_bytes={ASSET_PAGE_BYTES}"
    );
    let asset_pack = lab.cached_stage_with_hash(
        pass,
        label,
        "asset_page_pack",
        b"BAPPACK1",
        &pack_schema,
        Hash::for_blob(&pack_seed),
        modifier_pages_count.saturating_add(slicer_pages_count) as u64,
        "asset_pages",
        || Ok(build_asset_page_pack_payload(&modifier_pages, &slicer_pages)),
    )?;

    let residency_schema =
        format!("asset_page_residency_plan|{VERSION}|ram+vram+evictable=true|single_cache=true");
    let residency = lab.cached_stage(
        pass,
        label,
        "asset_page_residency_plan",
        b"BAPRES01",
        &residency_schema,
        &asset_pack,
        5,
        "residency_states",
        || Ok(build_asset_page_residency_plan_payload(&asset_pack)),
    )?;

    let mut ir_input = Vec::with_capacity(asset_pack.len() + residency.len() + topology.len().min(128));
    ir_input.extend_from_slice(&asset_pack);
    ir_input.extend_from_slice(&residency);
    ir_input.extend_from_slice(&topology[..topology.len().min(128)]);
    let ir_schema =
        format!("asset_page_render_ir_stub|{VERSION}|render_ir_references_pages=true|canvas_owns_no_state=true");
    let render_ir = lab.cached_stage(
        pass,
        label,
        "asset_page_render_ir_stub",
        b"BAPIR001",
        &ir_schema,
        &ir_input,
        estimate_modifier_output_tris(base_tris),
        "render_instances",
        || Ok(build_asset_page_render_ir_stub_payload(&asset_pack, &residency, &topology_hash)),
    )?;

    let proof_schema =
        format!("asset_page_proof|{VERSION}|no_full_buffers=true|outputs_are_hashes=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "asset_page_proof",
        b"BAPPRF01",
        &proof_schema,
        &render_ir,
        1,
        "proof_records",
        || Ok(build_asset_page_proof_payload(&asset_pack, &residency, &render_ir)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&modifier_pages),
            Hash::for_blob(&slicer_pages),
            Hash::for_blob(&asset_pack),
            Hash::for_blob(&residency),
            Hash::for_blob(&render_ir),
            Hash::for_blob(&proof),
        ],
        base_tris,
    );
    let manifest_schema = format!(
        "asset_page_spine_manifest|{VERSION}|cache_bytes={}|direct_pipeline=base_hash->modifier_pages->slicer_pages->asset_pack->residency->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "asset_page_spine_manifest",
        b"BAPMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        6,
        "asset_page_artifacts",
        || {
            Ok(build_asset_page_spine_manifest(
                &modifier_pages,
                &slicer_pages,
                &asset_pack,
                &residency,
                &render_ir,
                &proof,
            ))
        },
    )
}

fn run_asset_residency_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    normalized: &[u8],
    topology: &[u8],
) -> io::Result<Vec<u8>> {
    let base_hash = Hash::for_blob(normalized);
    let topology_hash = Hash::for_blob(topology);
    let base_tris = (deserialize_geometry_header(normalized)? / 9) as u64;
    let mesh_bytes = normalized.len();
    let topology_bytes = topology.len();
    let virtual_texture_bytes = config.layers.max(1).saturating_mul(64 * 1024);
    let source_page_count = asset_page_count(mesh_bytes)
        .saturating_add(asset_page_count(topology_bytes))
        .saturating_add(asset_page_count(virtual_texture_bytes))
        .saturating_add(2);

    let page_seed = hash_seed(&[base_hash, topology_hash], config.layers as u64);
    let page_schema = format!(
        "virtual_asset_memory_pages|{VERSION}|single_asset_store=true|page_bytes={ASSET_PAGE_BYTES}|no_parallel_cache=true"
    );
    let asset_pages = lab.cached_stage_with_hash(
        pass,
        label,
        "virtual_asset_memory_pages",
        b"BVAMPAG1",
        &page_schema,
        Hash::for_blob(&page_seed),
        source_page_count as u64,
        "asset_pages",
        || {
            Ok(build_virtual_asset_memory_pages_payload(
                &base_hash,
                &topology_hash,
                mesh_bytes,
                topology_bytes,
                virtual_texture_bytes,
            ))
        },
    )?;

    let table_schema = format!(
        "virtual_asset_residency_table|{VERSION}|states=ColdDisk+WarmRam+HotVram+Evictable+Pinned|ram_budget={}|vram_budget={}",
        config.cache_bytes,
        config.cache_bytes / 2
    );
    let residency_table = lab.cached_stage(
        pass,
        label,
        "virtual_asset_residency_table",
        b"BVAMTAB1",
        &table_schema,
        &asset_pages,
        5,
        "residency_states",
        || Ok(build_virtual_asset_residency_table_payload(&asset_pages, config.cache_bytes)),
    )?;

    let evict_schema =
        format!("virtual_asset_evict_cold_plan|{VERSION}|policy=keep_hot_pages_evict_cold|rollback_hash=true");
    let evict_plan = lab.cached_stage(
        pass,
        label,
        "virtual_asset_evict_cold_plan",
        b"BVAMEVC1",
        &evict_schema,
        &residency_table,
        source_page_count as u64,
        "asset_pages",
        || Ok(build_virtual_asset_evict_cold_plan_payload(&residency_table, config.cache_bytes)),
    )?;

    let mut pin_input = Vec::with_capacity(residency_table.len() + evict_plan.len());
    pin_input.extend_from_slice(&residency_table);
    pin_input.extend_from_slice(&evict_plan);
    let pin_schema =
        format!("virtual_asset_pin_hot_plan|{VERSION}|policy=pin_hot_working_set|gpu_budget_hash=true");
    let pin_plan = lab.cached_stage(
        pass,
        label,
        "virtual_asset_pin_hot_plan",
        b"BVAMPIN1",
        &pin_schema,
        &pin_input,
        4,
        "hot_pages",
        || Ok(build_virtual_asset_pin_hot_plan_payload(&residency_table, &evict_plan)),
    )?;

    let mut stream_input = Vec::with_capacity(asset_pages.len() + evict_plan.len() + pin_plan.len());
    stream_input.extend_from_slice(&asset_pages);
    stream_input.extend_from_slice(&evict_plan);
    stream_input.extend_from_slice(&pin_plan);
    let stream_schema =
        format!("virtual_asset_stream_plan|{VERSION}|ram_hot_cache+vram_residency=true|renderer_reads_kasm_state=true");
    let stream_plan = lab.cached_stage(
        pass,
        label,
        "virtual_asset_stream_plan",
        b"BVAMSTR1",
        &stream_schema,
        &stream_input,
        source_page_count as u64,
        "stream_pages",
        || Ok(build_virtual_asset_stream_plan_payload(&asset_pages, &evict_plan, &pin_plan)),
    )?;

    let mut proof_input = Vec::with_capacity(residency_table.len() + evict_plan.len() + pin_plan.len() + stream_plan.len());
    proof_input.extend_from_slice(&residency_table);
    proof_input.extend_from_slice(&evict_plan);
    proof_input.extend_from_slice(&pin_plan);
    proof_input.extend_from_slice(&stream_plan);
    let proof_schema =
        format!("virtual_asset_memory_proof|{VERSION}|single_cache=true|all_pages_content_addressed=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "virtual_asset_memory_proof",
        b"BVAMPRF1",
        &proof_schema,
        &proof_input,
        1,
        "proof_records",
        || {
            Ok(build_virtual_asset_memory_proof_payload(
                &asset_pages,
                &residency_table,
                &evict_plan,
                &pin_plan,
                &stream_plan,
            ))
        },
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&asset_pages),
            Hash::for_blob(&residency_table),
            Hash::for_blob(&evict_plan),
            Hash::for_blob(&pin_plan),
            Hash::for_blob(&stream_plan),
            Hash::for_blob(&proof),
        ],
        base_tris,
    );
    let manifest_schema = format!(
        "virtual_asset_memory_manifest|{VERSION}|cache_bytes={}|direct_pipeline=asset_pages->residency_table->evict_cold->pin_hot->stream_plan->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "virtual_asset_memory_manifest",
        b"BVAMMAN1",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        6,
        "asset_memory_artifacts",
        || {
            Ok(build_virtual_asset_memory_manifest(
                &asset_pages,
                &residency_table,
                &evict_plan,
                &pin_plan,
                &stream_plan,
                &proof,
            ))
        },
    )
}

fn run_geocluster_spine_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    normalized: &[u8],
    topology: &[u8],
) -> io::Result<Vec<u8>> {
    let source_mesh_hash = Hash::for_blob(normalized);
    let topology_hash = Hash::for_blob(topology);
    let base_tris = (deserialize_geometry_header(normalized)? / 9) as u64;
    let max_tris = 128u64;
    let cluster_count = base_tris.max(1).div_ceil(max_tris);
    let capped_clusters = cluster_count.min(2048);

    let cluster_schema =
        format!("geocluster_pages|{VERSION}|max_tris={max_tris}|meshlets_hash_only=true|lod=continuous");
    let cluster_pages = lab.cached_stage_with_hash(
        pass,
        label,
        "geocluster_pages",
        b"BGCPAGE1",
        &cluster_schema,
        source_mesh_hash,
        capped_clusters,
        "clusters",
        || Ok(build_geocluster_pages_payload(&source_mesh_hash, &topology_hash, base_tris, max_tris)),
    )?;

    let lod_schema =
        format!("geocluster_lod_tree|{VERSION}|continuous_lod=true|screen_error_hash=true");
    let lod_tree = lab.cached_stage(
        pass,
        label,
        "geocluster_lod_tree",
        b"BGCLOD01",
        &lod_schema,
        &cluster_pages,
        capped_clusters,
        "lod_nodes",
        || Ok(build_geocluster_lod_tree_payload(&cluster_pages, capped_clusters)),
    )?;

    let bounds_schema =
        format!("geocluster_bounds_tree|{VERSION}|aabb_per_cluster=true|gpu_cull_ready=true");
    let bounds_tree = lab.cached_stage(
        pass,
        label,
        "geocluster_bounds_tree",
        b"BGCBND01",
        &bounds_schema,
        &cluster_pages,
        capped_clusters,
        "bounds",
        || Ok(build_geocluster_bounds_tree_payload(&cluster_pages, &topology_hash)),
    )?;

    let mut asset_input = Vec::with_capacity(cluster_pages.len() + lod_tree.len() + bounds_tree.len());
    asset_input.extend_from_slice(&cluster_pages);
    asset_input.extend_from_slice(&lod_tree);
    asset_input.extend_from_slice(&bounds_tree);
    let asset_schema =
        format!("geocluster_asset|{VERSION}|source_mesh+cluster_pages+lod_tree+bounds_tree+materials=true");
    let geocluster_asset = lab.cached_stage(
        pass,
        label,
        "geocluster_asset",
        b"BGCASST1",
        &asset_schema,
        &asset_input,
        capped_clusters,
        "clusters",
        || Ok(build_geocluster_asset_payload(&cluster_pages, &lod_tree, &bounds_tree, &source_mesh_hash)),
    )?;

    let page_schema =
        format!("geocluster_asset_pages|{VERSION}|single_asset_store=true|cluster_pages_are_asset_pages=true");
    let asset_pages = lab.cached_stage(
        pass,
        label,
        "geocluster_asset_pages",
        b"BGCAPG01",
        &page_schema,
        &geocluster_asset,
        capped_clusters,
        "asset_pages",
        || Ok(build_geocluster_asset_pages_payload(&geocluster_asset, capped_clusters)),
    )?;

    let metric_schema =
        format!("geocluster_metric_records|{VERSION}|vram+lod_error+draw+stream_cost=true");
    let metrics = lab.cached_stage(
        pass,
        label,
        "geocluster_metric_records",
        b"BGCMET01",
        &metric_schema,
        &asset_pages,
        4,
        "metric_records",
        || Ok(build_geocluster_metric_records_payload(&geocluster_asset, &asset_pages)),
    )?;

    let proof_schema =
        format!("geocluster_proof|{VERSION}|source+pages+lod+bounds+metrics+environment=true");
    let proof = lab.cached_stage(
        pass,
        label,
        "geocluster_proof",
        b"BGCPRF01",
        &proof_schema,
        &metrics,
        1,
        "proof_records",
        || Ok(build_geocluster_proof_payload(&geocluster_asset, &asset_pages, &metrics)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&cluster_pages),
            Hash::for_blob(&lod_tree),
            Hash::for_blob(&bounds_tree),
            Hash::for_blob(&geocluster_asset),
            Hash::for_blob(&asset_pages),
            Hash::for_blob(&metrics),
            Hash::for_blob(&proof),
        ],
        base_tris,
    );
    let manifest_schema = format!(
        "geocluster_manifest|{VERSION}|cache_bytes={}|direct_pipeline=source_mesh->cluster_pages->lod_tree->bounds_tree->asset_pages->metrics->proof",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "geocluster_manifest",
        b"BGCMAN01",
        &manifest_schema,
        Hash::for_blob(&manifest_seed),
        7,
        "geocluster_artifacts",
        || {
            Ok(build_geocluster_manifest(
                &cluster_pages,
                &lod_tree,
                &bounds_tree,
                &geocluster_asset,
                &asset_pages,
                &metrics,
                &proof,
            ))
        },
    )
}

fn run_modifier_plan_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    normalized: &[u8],
    topology: &[u8],
) -> io::Result<Vec<u8>> {
    let base_hash = Hash::for_blob(normalized);
    let topology_hash = Hash::for_blob(topology);
    let base_tris = (deserialize_geometry_header(normalized)? / 9) as u64;
    let ops = 2u64;
    let plan_seed = hash_seed(&[base_hash, topology_hash], ops);
    let plan_hash = Hash::for_blob(&plan_seed);

    let plan_schema = format!(
        "modifier_stack_plan|{VERSION}|ops=bevel,solidify|lazy=true|content_addressed=true"
    );
    let plan = lab.cached_stage_with_hash(
        pass,
        label,
        "modifier_stack_plan",
        b"BMDPLN01",
        &plan_schema,
        plan_hash,
        ops,
        "modifier_ops",
        || Ok(build_modifier_stack_plan_payload(base_tris, &base_hash, &topology_hash)),
    )?;

    let legacy_schema = format!(
        "legacy_modifier_materialize|{VERSION}|bevel_width=0.14|solidify=0.20|benchmark_only=true"
    );
    let legacy = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_modifier_materialize",
        b"BMDLEG01",
        &legacy_schema,
        plan_hash,
        base_tris.saturating_mul(9),
        "modifier_triangle_ops",
        || {
            let geom = deserialize_geometry(normalized)?;
            Ok(benchmark_legacy_modifier_materialization(&geom))
        },
    )?;

    let removed_schema = format!(
        "modifier_geometry_materialization_removed|{VERSION}|persist_full_buffers=false"
    );
    let removed = lab.cached_stage_with_hash(
        pass,
        label,
        "modifier_geometry_materialization_removed",
        b"BMDREM01",
        &removed_schema,
        Hash::for_blob(&legacy),
        estimate_modifier_output_tris(base_tris),
        "triangles_not_materialized",
        || Ok(build_modifier_materialization_removed_payload(base_tris)),
    )?;

    let bounds_schema = format!(
        "modifier_plan_bounds|{VERSION}|bounds_from_base_plus_ops=true|no_final_mesh=true"
    );
    let bounds = lab.cached_stage_with_hash(
        pass,
        label,
        "modifier_plan_bounds",
        b"BMDBND01",
        &bounds_schema,
        Hash::for_blob(&plan),
        base_tris,
        "base_triangles",
        || {
            let geom = deserialize_geometry(normalized)?;
            Ok(build_modifier_plan_bounds_payload(&geom, 0.20))
        },
    )?;

    let manifest_seed = hash_seed(
        &[Hash::for_blob(&plan), Hash::for_blob(&removed), Hash::for_blob(&bounds)],
        config.cache_bytes as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "modifier_plan_manifest|{VERSION}|cache_bytes={}|direct_pipeline=base->plan->bounds",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "modifier_plan_manifest",
        b"BMDMAN01",
        &manifest_schema,
        manifest_hash,
        ops + base_tris,
        "modifier_plan_units",
        || Ok(build_modifier_plan_manifest(&plan, &legacy, &removed, &bounds)),
    )
}

fn run_pick_handle_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    solid: &[u8],
    solid_hash: Hash,
    render_hash: Hash,
    render_passes: &[u8],
    solid_tris: u64,
) -> io::Result<Vec<u8>> {
    let clicks = 24u64;
    let passes = render_pass_count(render_passes).max(1) as u64;
    let pick_seed = hash_seed(&[solid_hash, render_hash], clicks);
    let pick_hash = Hash::for_blob(&pick_seed);
    let middleman_schema = format!(
        "screen_pick_middleman_removed|{VERSION}|old_stage=screen_pick_index|persist_blob=false"
    );
    let middleman = lab.cached_stage_with_hash(
        pass,
        label,
        "screen_pick_middleman_removed",
        b"BPKMID01",
        &middleman_schema,
        pick_hash,
        solid_tris.saturating_mul(passes),
        "screen_pick_records_avoided",
        || Ok(build_pick_middleman_removed_payload(solid_tris, passes)),
    )?;

    let legacy_schema = format!(
        "legacy_pick_triangle_scan|{VERSION}|clicks={clicks}|ray_triangle=all_faces"
    );
    let legacy_work = solid_tris
        .saturating_mul(passes)
        .saturating_mul(clicks);
    let legacy_scan = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_pick_triangle_scan",
        b"BPKLEG01",
        &legacy_schema,
        pick_hash,
        legacy_work,
        "ray_triangle_tests",
        || Ok(simulate_legacy_pick_scan_direct(solid_tris, passes as usize, clicks as usize, &solid_hash)),
    )?;

    let handle_schema =
        format!("pick_screen_handle_build|{VERSION}|soa=bbox+refs|descriptor_only=true");
    let handle = lab.cached_stage_with_hash(
        pass,
        label,
        "pick_screen_handle_build",
        b"BPKHND01",
        &handle_schema,
        Hash::for_blob(&hash_seed(&[solid_hash, render_hash], solid_tris)),
        solid_tris.saturating_mul(passes),
        "screen_face_boxes",
        || {
            let geom = deserialize_geometry(solid)?;
            Ok(build_pick_handle_payload_from_geometry(&geom, render_passes, &render_hash))
        },
    )?;

    let query_seed = hash_seed(&[Hash::for_blob(&handle), Hash::for_blob(&middleman)], clicks);
    let query_hash = Hash::for_blob(&query_seed);
    let query_schema = format!("pick_handle_candidate_query|{VERSION}|clicks={clicks}|bbox_prune=true");
    let query = lab.cached_stage_with_hash(
        pass,
        label,
        "pick_handle_candidate_query",
        b"BPKQRY01",
        &query_schema,
        query_hash,
        clicks,
        "pick_clicks",
        || Ok(simulate_pick_handle_queries(solid_tris.saturating_mul(passes) as usize, clicks as usize)),
    )?;

    let manifest_seed = hash_seed(
        &[
            Hash::for_blob(&middleman),
            Hash::for_blob(&legacy_scan),
            Hash::for_blob(&handle),
            Hash::for_blob(&query),
        ],
        config.cache_bytes as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "pick_handle_manifest|{VERSION}|legacy_scan_removed=true|cache_bytes={}",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "pick_handle_manifest",
        b"BPKMAN01",
        &manifest_schema,
        manifest_hash,
        clicks + solid_tris.saturating_mul(passes),
        "pick_audit_units",
        || Ok(build_pick_handle_manifest(&middleman, &legacy_scan, &handle, &query)),
    )
}

fn run_ui_coalesce_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    render_hash: Hash,
    world_bounds_hash: Hash,
    screen_pick: &[u8],
    overlay: &[u8],
) -> io::Result<Vec<u8>> {
    let ui_requests = 42u64;
    let duplicate_requests = 31u64;
    let control_count = 96u64;
    let overlay_hash = Hash::for_blob(overlay);
    let screen_hash = Hash::for_blob(screen_pick);
    let ui_seed = hash_seed(&[render_hash, world_bounds_hash, screen_hash, overlay_hash], ui_requests);
    let ui_hash = Hash::for_blob(&ui_seed);

    let legacy_schema = format!(
        "legacy_ui_rerender_fanout|{VERSION}|sidebar+hud+contract|requests={ui_requests}|direct_dom=true"
    );
    let legacy_work_units = ui_requests
        .saturating_mul(control_count)
        .saturating_mul(3);
    let legacy_fanout = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_ui_rerender_fanout",
        b"BUILEG01",
        &legacy_schema,
        ui_hash,
        legacy_work_units,
        "control_dom_ops",
        || Ok(simulate_ui_rerender_fanout(screen_pick, overlay, ui_requests as usize)),
    )?;

    let gate_schema = format!(
        "ui_render_coalesce_gate|{VERSION}|queue=microtask|requests={ui_requests}|flushes=1"
    );
    let gate = lab.cached_stage_with_hash(
        pass,
        label,
        "ui_render_coalesce_gate",
        b"BUIGAT01",
        &gate_schema,
        ui_hash,
        ui_requests,
        "ui_requests",
        || Ok(build_ui_coalesce_gate_payload(ui_requests, 1, duplicate_requests)),
    )?;

    let signature_seed = hash_seed(&[Hash::for_blob(&gate), overlay_hash], duplicate_requests);
    let signature_hash = Hash::for_blob(&signature_seed);
    let signature_schema = format!(
        "ui_html_signature_skip|{VERSION}|content_addressed=true|duplicates={duplicate_requests}"
    );
    let signature = lab.cached_stage_with_hash(
        pass,
        label,
        "ui_html_signature_skip",
        b"BUISIG01",
        &signature_schema,
        signature_hash,
        duplicate_requests,
        "duplicate_renders",
        || Ok(build_ui_signature_payload(overlay, duplicate_requests as usize)),
    )?;

    let contract_seed = hash_seed(&[Hash::for_blob(&signature), screen_hash], control_count);
    let contract_hash = Hash::for_blob(&contract_seed);
    let contract_schema =
        format!("ui_contract_delta_sync|{VERSION}|sync_once_after_dom_change=true");
    let contract = lab.cached_stage_with_hash(
        pass,
        label,
        "ui_contract_delta_sync",
        b"BUICON01",
        &contract_schema,
        contract_hash,
        control_count,
        "visible_controls",
        || Ok(build_ui_contract_delta_payload(screen_pick, control_count as usize)),
    )?;

    let manifest_seed = hash_seed(
        &[Hash::for_blob(&legacy_fanout), Hash::for_blob(&gate), Hash::for_blob(&contract)],
        config.cache_bytes as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "ui_render_manifest|{VERSION}|fanout_removed=true|cache_bytes={}",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "ui_render_manifest",
        b"BUIMAN01",
        &manifest_schema,
        manifest_hash,
        ui_requests + duplicate_requests + control_count,
        "ui_audit_units",
        || Ok(build_ui_render_manifest(&legacy_fanout, &gate, &signature, &contract)),
    )
}

fn run_frame_loop_focus(
    lab: &mut BangerLab,
    config: &Config,
    pass: usize,
    label: &str,
    solid_hash: Hash,
    render_hash: Hash,
    world_bounds_hash: Hash,
    render_passes: &[u8],
    screen_pick: &[u8],
    overlay: &[u8],
    solid_tris: u64,
) -> io::Result<Vec<u8>> {
    let idle_frames = 240u64;
    let dirty_frames = 12u64;
    let overlay_hash = Hash::for_blob(overlay);

    let idle_seed = hash_seed(&[solid_hash, render_hash, world_bounds_hash, overlay_hash], idle_frames);
    let idle_hash = Hash::for_blob(&idle_seed);
    let idle_schema = format!(
        "idle_frame_loop_eliminated|{VERSION}|scheduler=dirty-frame|idle_frames={idle_frames}"
    );
    let idle_gate = lab.cached_stage_with_hash(
        pass,
        label,
        "idle_frame_loop_eliminated",
        b"BIDLEG01",
        &idle_schema,
        idle_hash,
        idle_frames,
        "idle_frames",
        || Ok(build_idle_frame_gate_payload(idle_frames, &render_hash, &overlay_hash)),
    )?;

    let legacy_schema = format!(
        "legacy_viewport_frame_work|{VERSION}|continuous_raf=true|frames={idle_frames}|grid+mesh+slicer+overlay"
    );
    let legacy_work_units = solid_tris
        .saturating_mul(render_pass_count(render_passes) as u64)
        .saturating_mul(idle_frames);
    let legacy_loop = lab.cached_stage_with_hash(
        pass,
        label,
        "legacy_viewport_frame_work",
        b"BLFRM001",
        &legacy_schema,
        idle_hash,
        legacy_work_units,
        "triangle_frame_ops",
        || Ok(simulate_viewport_frame_work(solid_tris, render_passes, screen_pick, idle_frames as usize)),
    )?;

    let dirty_seed = hash_seed(
        &[solid_hash, render_hash, Hash::for_blob(&idle_gate), Hash::for_blob(&legacy_loop)],
        dirty_frames,
    );
    let dirty_hash = Hash::for_blob(&dirty_seed);
    let dirty_schema = format!(
        "dirty_interaction_frame_burst|{VERSION}|continuous_only_while_input=true|frames={dirty_frames}"
    );
    let dirty_work_units = solid_tris
        .saturating_mul(render_pass_count(render_passes) as u64)
        .saturating_mul(dirty_frames);
    let dirty_burst = lab.cached_stage_with_hash(
        pass,
        label,
        "dirty_interaction_frame_burst",
        b"BDFRM001",
        &dirty_schema,
        dirty_hash,
        dirty_work_units,
        "dirty_frame_ops",
        || Ok(simulate_viewport_frame_work(solid_tris, render_passes, screen_pick, dirty_frames as usize)),
    )?;

    let manifest_seed = hash_seed(
        &[Hash::for_blob(&idle_gate), Hash::for_blob(&dirty_burst), overlay_hash],
        config.cache_bytes as u64,
    );
    let manifest_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!(
        "frame_scheduler_manifest|{VERSION}|idle_frames={idle_frames}|dirty_frames={dirty_frames}|cache_bytes={}",
        config.cache_bytes
    );
    lab.cached_stage_with_hash(
        pass,
        label,
        "frame_scheduler_manifest",
        b"BFSUM001",
        &manifest_schema,
        manifest_hash,
        idle_frames + dirty_frames,
        "scheduled_frames",
        || Ok(build_frame_scheduler_manifest(&idle_gate, &dirty_burst, &legacy_loop)),
    )
}

fn run_gpu_resource_focus(
    lab: &mut BangerLab,
    pass: usize,
    label: &str,
    solid_hash: Hash,
    render_hash: Hash,
    slice_hash: Hash,
    display_upload_bytes: usize,
    slicer_upload_bytes: usize,
    overlay: &[u8],
) -> io::Result<Vec<u8>> {
    let display_seed = hash_seed(&[solid_hash, render_hash], display_upload_bytes as u64);
    let display_hash = Hash::for_blob(&display_seed);
    let display_schema = format!("gpu_display_upload|{VERSION}|vao=v1|buffer=pos+nrm");
    let display_handle = lab.cached_stage_with_hash(
        pass,
        label,
        "gpu_display_upload",
        b"BGDSPU01",
        &display_schema,
        display_hash,
        display_upload_bytes as u64,
        "upload_bytes",
        || Ok(build_gpu_handle_payload("display", &solid_hash, display_upload_bytes)),
    )?;

    let slicer_seed = hash_seed(&[slice_hash, render_hash], slicer_upload_bytes as u64);
    let slicer_hash = Hash::for_blob(&slicer_seed);
    let slicer_schema = format!("gpu_slicer_upload|{VERSION}|vao=v1|buffer=pos+color");
    let slicer_handle = lab.cached_stage_with_hash(
        pass,
        label,
        "gpu_slicer_upload",
        b"BGSLUP01",
        &slicer_schema,
        slicer_hash,
        slicer_upload_bytes as u64,
        "upload_bytes",
        || Ok(build_gpu_handle_payload("slicer", &slice_hash, slicer_upload_bytes)),
    )?;

    let manifest_hash = Hash::for_blob(overlay);
    let display_handle_hash = Hash::for_blob(&display_handle);
    let slicer_handle_hash = Hash::for_blob(&slicer_handle);
    let manifest_seed = hash_seed(&[manifest_hash, display_handle_hash, slicer_handle_hash], 2);
    let manifest_input_hash = Hash::for_blob(&manifest_seed);
    let manifest_schema = format!("gpu_resource_manifest|{VERSION}|handles=display,slicer");
    lab.cached_stage_with_hash(
        pass,
        label,
        "gpu_resource_manifest",
        b"BGPMNF01",
        &manifest_schema,
        manifest_input_hash,
        2,
        "gpu_handles",
        || {
            let mut out = Vec::with_capacity(48);
            out.extend_from_slice(b"BGPM1");
            out.extend_from_slice(display_handle_hash.as_bytes());
            out.extend_from_slice(slicer_handle_hash.as_bytes());
            Ok(out)
        },
    )
}

impl BangerLab {
    fn touch_ram_key(&mut self, key: &[u8; 32]) {
        self.ram_lru.retain(|entry| entry != key);
        self.ram_lru.push_back(*key);
    }

    fn insert_ram(&mut self, key: [u8; 32], output_hash: Hash, output: Vec<u8>) -> (usize, usize) {
        let bytes = output.len();
        if bytes > self.ram_max_bytes {
            return (0, 0);
        }
        if let Some(previous) = self.ram.remove(&key) {
            self.ram_bytes = self.ram_bytes.saturating_sub(previous.bytes);
            self.ram_lru.retain(|entry| entry != &key);
        }
        let mut evicted = 0usize;
        let mut evicted_bytes = 0usize;
        while self.ram_bytes + bytes > self.ram_max_bytes {
            let Some(old_key) = self.ram_lru.pop_front() else {
                break;
            };
            if let Some(old) = self.ram.remove(&old_key) {
                self.ram_bytes = self.ram_bytes.saturating_sub(old.bytes);
                evicted += 1;
                evicted_bytes += old.bytes;
            }
        }
        self.ram_lru.push_back(key);
        self.ram_bytes += bytes;
        self.ram.insert(
            key,
            CacheEntry {
                output_hash,
                output,
                bytes,
            },
        );
        self.ram_evictions += evicted;
        self.ram_evicted_bytes += evicted_bytes;
        (evicted, evicted_bytes)
    }

    fn cached_stage(
        &mut self,
        pass: usize,
        label: &str,
        stage: &'static str,
        namespace: &[u8; 8],
        schema: &str,
        input: &[u8],
        work_units: u64,
        unit: &'static str,
        compute: impl FnOnce() -> io::Result<Vec<u8>>,
    ) -> io::Result<Vec<u8>> {
        let input_hash = Hash::for_blob(input);
        self.cached_stage_with_hash(
            pass,
            label,
            stage,
            namespace,
            schema,
            input_hash,
            work_units,
            unit,
            compute,
        )
    }

    fn cached_stage_with_hash(
        &mut self,
        pass: usize,
        label: &str,
        stage: &'static str,
        namespace: &[u8; 8],
        schema: &str,
        input_hash: Hash,
        work_units: u64,
        unit: &'static str,
        compute: impl FnOnce() -> io::Result<Vec<u8>>,
    ) -> io::Result<Vec<u8>> {
        let started = Instant::now();
        let key = cache_key(namespace, schema, &input_hash);

        if self.ram.contains_key(&key) {
            self.touch_ram_key(&key);
            let entry = self.ram.get(&key).expect("ram key exists after contains");
            let output_hash = entry.output_hash;
            let output = entry.output.clone();
            let output_bytes = output.len();
            let record = StageRecord {
                pass,
                label: label.to_string(),
                stage,
                status: "RAM_HIT",
                elapsed: started.elapsed(),
                compute_elapsed: Duration::ZERO,
                input_hash: input_hash.as_hex(),
                output_hash: output_hash.as_hex(),
                output_bytes,
                work_units,
                unit,
                cache_bytes: self.ram_bytes,
                evicted: 0,
                evicted_bytes: 0,
            };
            self.compute_stats
                .record_hit(work_units as usize, record.elapsed);
            self.emit_record(&record)?;
            self.records.push(record);
            return Ok(output);
        }

        if let Some(result_hash_bytes) = self.atlas.lookup_result(&key) {
            let output_hash = Hash::from_bytes(result_hash_bytes);
            if let Some(output) = self.store.load(&output_hash) {
                let eviction = self.insert_ram(key, output_hash, output.clone());
                let record = StageRecord {
                    pass,
                    label: label.to_string(),
                    stage,
                    status: "HIT",
                    elapsed: started.elapsed(),
                    compute_elapsed: Duration::ZERO,
                    input_hash: input_hash.as_hex(),
                    output_hash: output_hash.as_hex(),
                    output_bytes: output.len(),
                    work_units,
                    unit,
                    cache_bytes: self.ram_bytes,
                    evicted: eviction.0,
                    evicted_bytes: eviction.1,
                };
                self.compute_stats
                    .record_hit(work_units as usize, record.elapsed);
                self.emit_record(&record)?;
                self.records.push(record);
                return Ok(output);
            }
        }

        let compute_started = Instant::now();
        let output = compute()?;
        let compute_elapsed = compute_started.elapsed();
        let output_hash = self.store.store(&output)?;
        self.atlas.record_result(&key, output_hash.as_bytes())?;
        let eviction = self.insert_ram(key, output_hash, output.clone());
        let record = StageRecord {
            pass,
            label: label.to_string(),
            stage,
            status: "MISS",
            elapsed: started.elapsed(),
            compute_elapsed,
            input_hash: input_hash.as_hex(),
            output_hash: output_hash.as_hex(),
            output_bytes: output.len(),
            work_units,
            unit,
            cache_bytes: self.ram_bytes,
            evicted: eviction.0,
            evicted_bytes: eviction.1,
        };
        self.compute_stats.record_miss(record.elapsed);
        self.emit_record(&record)?;
        self.records.push(record);
        Ok(output)
    }

    fn cached_slicer_preview(
        &mut self,
        pass: usize,
        label: &str,
        solid: &[u8],
        layers: usize,
        total_work_units: u64,
    ) -> io::Result<Vec<u8>> {
        let bounds_schema = format!("slicer_bounds|{VERSION}");
        let solid_hash = Hash::for_blob(solid);
        let bounds_bytes = self.cached_stage_with_hash(
            pass,
            label,
            "slicer_bounds",
            b"BSLBND01",
            &bounds_schema,
            solid_hash,
            total_work_units / layers.max(1) as u64,
            "triangles",
            || {
                let geom = deserialize_geometry(solid)?;
                Ok(serialize_z_bounds(&z_bounds(&geom)?))
            },
        )?;
        let bounds = deserialize_z_bounds(&bounds_bytes)?;
        let geom = deserialize_geometry(solid)?;
        let step = (bounds.max_z - bounds.min_z) / layers.max(1) as f32;
        let mut layer_refs = Vec::with_capacity(layers);
        for layer in 0..layers {
            let z = bounds.min_z + (layer as f32 + 0.5) * step;
            let layer_schema = format!(
                "slicer_layer|{VERSION}|layers={layers}|layer={layer}|zq={}",
                quantize(z)
            );
            let layer_key = cache_key(b"BSLLYR01", &layer_schema, &solid_hash);
            let layer_bytes = self.cached_stage_with_hash(
                pass,
                label,
                "slicer_layer",
                b"BSLLYR01",
                &layer_schema,
                solid_hash,
                geom.tri_count() as u64,
                "triangle_layer_tests",
                || Ok(compute_slicer_layer(&geom, z)),
            )?;
            let layer_segments = if layer_bytes.len() >= 8 && &layer_bytes[..4] == b"BLL1" {
                u32::from_le_bytes(layer_bytes[4..8].try_into().unwrap())
            } else {
                0
            };
            layer_refs.push((layer_key, layer_segments, layer_bytes.len() as u32));
        }
        let segment_count: u32 = layer_refs
            .iter()
            .map(|(_, segments, _)| *segments)
            .sum();
        let mut manifest_seed = Vec::with_capacity(20 + layer_refs.len() * 40);
        manifest_seed.extend_from_slice(b"BSMSEED1");
        manifest_seed.extend_from_slice(&(layers as u32).to_le_bytes());
        manifest_seed.extend_from_slice(&segment_count.to_le_bytes());
        for (layer_key, layer_segments, layer_bytes) in &layer_refs {
            manifest_seed.extend_from_slice(layer_key);
            manifest_seed.extend_from_slice(&layer_segments.to_le_bytes());
            manifest_seed.extend_from_slice(&layer_bytes.to_le_bytes());
        }
        let manifest_schema = format!("slicer_preview_manifest|{VERSION}|layers={layers}");
        let manifest_input_hash = Hash::for_blob(&manifest_seed);
        self.cached_stage_with_hash(
            pass,
            label,
            "slicer_preview_manifest",
            b"BSLMNF01",
            &manifest_schema,
            manifest_input_hash,
            layers as u64,
            "layers",
            || {
                let mut out = Vec::with_capacity(12 + layer_refs.len() * 40);
                out.extend_from_slice(b"BSM1");
                out.extend_from_slice(&(layers as u32).to_le_bytes());
                out.extend_from_slice(&segment_count.to_le_bytes());
                for (layer_key, layer_segments, layer_bytes) in &layer_refs {
                    out.extend_from_slice(layer_key);
                    out.extend_from_slice(&layer_segments.to_le_bytes());
                    out.extend_from_slice(&layer_bytes.to_le_bytes());
                }
                Ok(out)
            },
        )
    }

    fn emit_record(&self, record: &StageRecord) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(
            file,
            "{{\"kind\":\"banger_lab_stage\",\"version\":\"{}\",\"pass\":{},\"label\":\"{}\",\"stage\":\"{}\",\"status\":\"{}\",\"elapsed_ms\":{:.6},\"compute_ms\":{:.6},\"input_hash\":\"{}\",\"output_hash\":\"{}\",\"output_bytes\":{},\"work_units\":{},\"unit\":\"{}\",\"cache_bytes\":{},\"evicted\":{},\"evicted_bytes\":{},\"avoided\":{},\"within_60hz_frame\":{},\"within_interaction_budget\":{}}}",
            json_escape(VERSION),
            record.pass,
            json_escape(&record.label),
            json_escape(record.stage),
            record.status,
            ms(record.elapsed),
            ms(record.compute_elapsed),
            record.input_hash,
            record.output_hash,
            record.output_bytes,
            record.work_units,
            record.unit,
            record.cache_bytes,
            record.evicted,
            record.evicted_bytes,
            is_avoided_status(record.status),
            ms(record.elapsed) <= FRAME_60HZ_MS,
            ms(record.elapsed) <= INTERACTION_BUDGET_MS
        )?;
        println!(
            "STAGE pass={} stage={} status={} elapsed_ms={:.3} compute_ms={:.3} work={} {} bytes={} cache_mb={:.2} evicted={} out={} frame60={}",
            record.pass,
            record.stage,
            record.status,
            ms(record.elapsed),
            ms(record.compute_elapsed),
            record.work_units,
            record.unit,
            record.output_bytes,
            record.cache_bytes as f64 / (1024.0 * 1024.0),
            record.evicted,
            compact_hash(&record.output_hash),
            if ms(record.elapsed) <= FRAME_60HZ_MS { "OK" } else { "MISS" }
        );
        Ok(())
    }
}

fn cache_key(namespace: &[u8; 8], schema: &str, input_hash: &Hash) -> [u8; 32] {
    let namespace_label = std::str::from_utf8(namespace).unwrap_or("binary");
    let input_hash_hex = input_hash.as_hex();
    let scoped_key = ComputeSurface::Banger.stage_key(
        namespace_label,
        &[schema, &input_hash_hex],
    );
    let mut hasher = Sha256::new();
    hasher.update(scoped_key.as_bytes());
    hasher.finalize().into()
}

fn print_summary(records: &[StageRecord], compute_stats: ComputeCacheStats) {
    let hits = records.iter().filter(|r| is_avoided_status(r.status)).count();
    let ram_hits = records.iter().filter(|r| r.status == "RAM_HIT").count();
    let misses = records.iter().filter(|r| r.status == "MISS").count();
    let direct = records.iter().filter(|r| r.status == "DIRECT").count();
    let avoided_units: u64 = records
        .iter()
        .filter(|r| is_avoided_status(r.status))
        .map(|r| r.work_units)
        .sum();
    let cold_ms: f64 = records
        .iter()
        .filter(|r| r.pass == 1)
        .map(|r| ms(r.elapsed))
        .sum();
    let warm_ms: f64 = records
        .iter()
        .filter(|r| r.pass > 1)
        .map(|r| ms(r.elapsed))
        .sum();
    let speedup = if warm_ms > 0.0 { cold_ms / warm_ms } else { 0.0 };
    let avoided_compute_ms = estimated_avoided_compute_ms(records);
    let latencies: Vec<f64> = records.iter().map(|record| ms(record.elapsed)).collect();
    let evictions: usize = records.iter().map(|record| record.evicted).sum();
    let evicted_bytes: usize = records.iter().map(|record| record.evicted_bytes).sum();
    let peak_cache_bytes = records
        .iter()
        .map(|record| record.cache_bytes)
        .max()
        .unwrap_or(0);
    println!(
        "SUMMARY stages={} hits={} ram_hits={} misses={} direct={} avoided_units={} avoided_compute_ms={:.3} cold_ms={:.3} warm_ms={:.3} speedup_x={:.2} p50_ms={:.3} p95_ms={:.3} p99_ms={:.3} peak_cache_mb={:.2} evictions={} evicted_mb={:.2} core_hit_rate={:.4} core_avoided_units={}",
        records.len(),
        hits,
        ram_hits,
        misses,
        direct,
        avoided_units,
        avoided_compute_ms,
        cold_ms,
        warm_ms,
        speedup,
        percentile(&latencies, 50.0),
        percentile(&latencies, 95.0),
        percentile(&latencies, 99.0),
        peak_cache_bytes as f64 / (1024.0 * 1024.0),
        evictions,
        evicted_bytes as f64 / (1024.0 * 1024.0),
        compute_stats.hit_rate(),
        compute_stats.avoided_units
    );

    let mut by_stage: HashMap<&'static str, (&StageRecord, Option<&StageRecord>)> = HashMap::new();
    for record in records {
        let entry = by_stage.entry(record.stage).or_insert((record, None));
        if record.pass == 1 {
            entry.0 = record;
        } else if entry.1.is_none() {
            entry.1 = Some(record);
        }
    }
    for (stage, (cold, warm)) in by_stage {
        let warm_ms = warm.map(|r| ms(r.elapsed)).unwrap_or(0.0);
        let cold_ms = ms(cold.elapsed);
        let rate = if cold_ms > 0.0 {
            cold.work_units as f64 / (cold_ms / 1000.0)
        } else {
            0.0
        };
        let warm_status = if warm_ms <= FRAME_60HZ_MS {
            "frame-safe"
        } else if warm_ms <= INTERACTION_BUDGET_MS {
            "interactive"
        } else {
            "too-slow-for-frame"
        };
        println!(
            "POWER stage={} cold_ms={:.3} warm_ms={:.3} speedup_x={:.2} throughput={:.1} {}/s latency={}",
            stage,
            cold_ms,
            warm_ms,
            if warm_ms > 0.0 { cold_ms / warm_ms } else { 0.0 },
            rate,
            cold.unit,
            warm_status
        );
    }
    print_power_angles(records);
}

fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let index = (((p / 100.0) * sorted.len() as f64).ceil() as usize).saturating_sub(1);
    sorted[index.min(sorted.len() - 1)]
}

fn estimated_avoided_compute_ms(records: &[StageRecord]) -> f64 {
    let mut miss_sum: HashMap<&'static str, (f64, usize)> = HashMap::new();
    for record in records.iter().filter(|r| r.status == "MISS") {
        let entry = miss_sum.entry(record.stage).or_insert((0.0, 0));
        entry.0 += ms(record.compute_elapsed);
        entry.1 += 1;
    }
    records
        .iter()
        .filter(|r| is_avoided_status(r.status))
        .filter_map(|record| {
            let (sum, count) = miss_sum.get(record.stage)?;
            Some(sum / *count as f64)
        })
        .sum()
}

fn print_power_angles(records: &[StageRecord]) {
    let warm_records: Vec<&StageRecord> = records
        .iter()
        .filter(|r| r.pass > 1 && is_avoided_status(r.status))
        .collect();
    let warm_frame_safe = warm_records
        .iter()
        .filter(|r| ms(r.elapsed) <= FRAME_60HZ_MS)
        .count();
    let warm_interactive = warm_records
        .iter()
        .filter(|r| ms(r.elapsed) <= INTERACTION_BUDGET_MS)
        .count();
    println!(
        "ANGLE cache_effectiveness warm_hits={} frame_safe_hits={} interactive_hits={}",
        warm_records.len(),
        warm_frame_safe,
        warm_interactive
    );
    let direct_pipeline = if records.iter().any(|r| r.stage == "virtual_asset_memory_manifest") {
        "asset_pages->residency_table->evict_cold->pin_hot->stream_plan->proof"
    } else if records.iter().any(|r| r.stage == "asset_page_spine_manifest") {
        "base_hash->modifier_pages->slicer_pages->asset_pack->residency->proof"
    } else if records.iter().any(|r| r.stage == "geocluster_manifest") {
        "source_mesh->cluster_pages->lod_tree->bounds_tree->asset_pages->metrics->proof"
    } else if records.iter().any(|r| r.stage == "render_asset_manifest") {
        "asset_pages->residency->render_ir->canvas_projection->proof"
    } else if records.iter().any(|r| r.stage == "skill_spine_manifest") {
        "skill_spec->program_graph->metric_set->test_set->skill_run->proof"
    } else if records.iter().any(|r| r.stage == "program_matrix_manifest") {
        "program_spec->program_run->matrix_variants->metrics->top_selection"
    } else if records.iter().any(|r| r.stage == "compute_ir_manifest") {
        "compute_program->buffers->dispatch->sandbox->run->metrics->proof"
    } else if records.iter().any(|r| r.stage == "metric_spine_manifest") {
        "metric_spec->evaluator_program->metric_record->run_proof_metric_hashes"
    } else if records.iter().any(|r| r.stage == "hash_time_manifest") {
        "run_record->hash_index->explain_hash->rollback_apply->proof"
    } else if records.iter().any(|r| r.stage == "world_patch_manifest") {
        "command_spec->world_patch->metric_expectations->apply->proof"
    } else if records.iter().any(|r| r.stage == "mcp_facade_manifest") {
        "mcp_tools_resources_prompts->command_spec->bytecode->sandbox->proof"
    } else if records.iter().any(|r| r.stage == "kasm_spine_manifest") {
        "intent->command_spec->bytecode_program->sandbox_matrix->run_record->proof"
    } else if records
        .iter()
        .any(|r| r.stage == "normalize_view_from_carried_bounds")
    {
        "parse_stream->carried_bounds->normalize_view"
    } else if records
        .iter()
        .any(|r| r.stage == "normalize_view_from_carried_key")
    {
        "reader_content_hash->normalize_view->materialize_on_demand"
    } else if records.iter().any(|r| r.stage == "import_normalize_view") {
        "source->fingerprint->normalize_view->sample_or_materialize_on_demand"
    } else if records.iter().any(|r| r.stage == "modifier_stack_plan") {
        "base_mesh->modifier_plan->derived_bounds"
    } else if records.iter().any(|r| r.stage == "pick_handle_manifest") {
        "geometry->screen_pick_handle->component_query"
    } else if records.iter().any(|r| r.stage == "ui_render_manifest") {
        "state_delta->coalesced_ui_flush->contract_delta"
    } else if records.iter().any(|r| r.stage == "frame_scheduler_manifest") {
        "dirty_event->bounded_frame_burst->idle"
    } else if records.iter().any(|r| r.stage == "gpu_resource_manifest") {
        "content_hash->gpu_handle_reuse"
    } else {
        "source->normalize->topology/modifiers->layers->pick"
    };
    println!(
        "ANGLE recompute_surface layers_are_addressed_individually={} direct_pipeline={}",
        records.iter().any(|r| r.stage == "slicer_layer"),
        direct_pipeline
    );
    if let Some(worst_warm) = records
        .iter()
        .filter(|r| r.pass > 1)
        .max_by(|a, b| ms(a.elapsed).total_cmp(&ms(b.elapsed)))
    {
        let reason = if worst_warm.stage == "legacy_direct_tool_dispatch" {
            "benchmark-only direct paths; compile every user/LLM/MCP action to KASM"
        } else if worst_warm.stage == "kasm_run_record" || worst_warm.stage == "kasm_proof_record" {
            "hot proof materializes bytes; keep compact native proof handles next"
        } else if worst_warm.stage == "legacy_bounds_rescan" {
            "benchmark-only bounds rescan; track bounds during parsing"
        } else if worst_warm.stage == "legacy_float_fingerprint_scan" {
            "benchmark-only float fingerprint scan; replace with importer-carried source key"
        } else if worst_warm.stage == "normalize_view_from_carried_bounds" {
            "view is compact; next cut is carrying it as a native handle"
        } else if worst_warm.stage == "normalize_view_from_carried_key" {
            "bounds still require a source scan; persist importer-provided bounds next"
        } else if worst_warm.stage == "legacy_normalize_materialize" {
            "benchmark-only legacy path; keep it out of the interactive pipeline"
        } else if worst_warm.stage == "import_source_fingerprint" {
            "content key still costs a source scan unless the importer carries a stable hash"
        } else if worst_warm.status == "DIRECT" {
            "uncached fan-in assembly still rebuilds a large aggregate"
        } else if worst_warm.status == "RAM_HIT" {
            "hot hit still materializes bytes; keep GPU/native handles alive"
        } else {
            "persistent hit still spends time loading/materializing cached blobs"
        };
        println!(
            "ANGLE next_cut stage={} warm_hit_ms={:.3} reason={}",
            worst_warm.stage,
            ms(worst_warm.elapsed),
            reason
        );
    }
}

fn is_avoided_status(status: &str) -> bool {
    status == "HIT" || status == "RAM_HIT"
}

fn generate_mesh(triangles: usize, tag: &str) -> Geometry {
    let mut pos = Vec::with_capacity(triangles * 9);
    let mut nrm = Vec::with_capacity(triangles * 9);
    let salt = fnv1a64(tag.as_bytes()) as f32 * 0.0000000001;
    let side = (triangles as f32).sqrt().ceil() as usize;
    for i in 0..triangles {
        let x = (i % side) as f32 - side as f32 * 0.5;
        let y = (i / side) as f32 - side as f32 * 0.5;
        let wave = ((i as f32 * 0.017) + salt).sin() * 0.35;
        let twist = ((i as f32 * 0.011) + salt).cos() * 0.18;
        let a = [x * 0.11, y * 0.11, wave];
        let b = [x * 0.11 + 0.08 + twist * 0.03, y * 0.11 + 0.015, wave + 0.04];
        let c = [x * 0.11 + 0.025, y * 0.11 + 0.09 + twist * 0.02, wave - 0.03];
        append_triangle_flat(&mut pos, &mut nrm, a, b, c);
    }
    Geometry { pos, nrm }
}

fn normalize_geometry(geom: &Geometry) -> Geometry {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for chunk in geom.pos.chunks_exact(3) {
        for axis in 0..3 {
            min[axis] = min[axis].min(chunk[axis]);
            max[axis] = max[axis].max(chunk[axis]);
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let span = [
        max[0] - min[0],
        max[1] - min[1],
        max[2] - min[2],
    ];
    let max_span = span[0].max(span[1]).max(span[2]).max(1e-6);
    let scale = 6.0 / max_span;
    let mut pos = Vec::with_capacity(geom.pos.len());
    for chunk in geom.pos.chunks_exact(3) {
        pos.push((chunk[0] - center[0]) * scale);
        pos.push((chunk[1] - center[1]) * scale);
        pos.push((chunk[2] - center[2]) * scale);
    }
    Geometry {
        pos,
        nrm: geom.nrm.clone(),
    }
}

fn build_import_normalize_view(geom: &Geometry) -> ImportNormalizeView {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for chunk in geom.pos.chunks_exact(3) {
        for axis in 0..3 {
            min[axis] = min[axis].min(chunk[axis]);
            max[axis] = max[axis].max(chunk[axis]);
        }
    }
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let span = [
        max[0] - min[0],
        max[1] - min[1],
        max[2] - min[2],
    ];
    let max_span = span[0].max(span[1]).max(span[2]).max(1e-6);
    ImportNormalizeView {
        min,
        max,
        center,
        span,
        scale: 6.0 / max_span,
        pos_floats: geom.pos.len() as u32,
        nrm_floats: geom.nrm.len() as u32,
        triangles: geom.tri_count() as u32,
    }
}

fn build_import_source_fingerprint_payload(source_bytes: &[u8], source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(56);
    out.extend_from_slice(b"BNF1");
    out.extend_from_slice(source_hash.as_bytes());
    out.extend_from_slice(&(source_bytes.len() as u64).to_le_bytes());
    let pos_floats = deserialize_geometry_header(source_bytes).unwrap_or(0);
    out.extend_from_slice(&(pos_floats as u32).to_le_bytes());
    out.extend_from_slice(&((pos_floats / 9) as u32).to_le_bytes());
    out
}

fn build_import_normalize_view_payload(geom: &Geometry) -> Vec<u8> {
    let view = build_import_normalize_view(geom);
    let mut out = Vec::with_capacity(72);
    out.extend_from_slice(b"BNV1");
    out.extend_from_slice(&view.pos_floats.to_le_bytes());
    out.extend_from_slice(&view.nrm_floats.to_le_bytes());
    out.extend_from_slice(&view.triangles.to_le_bytes());
    for value in view
        .min
        .into_iter()
        .chain(view.max)
        .chain(view.center)
        .chain(view.span)
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&view.scale.to_le_bytes());
    out
}

fn parse_import_normalize_view_payload(bytes: &[u8]) -> io::Result<ImportNormalizeView> {
    if bytes.len() < 68 || &bytes[..4] != b"BNV1" {
        return Err(io::Error::other("bad BOOM import normalize view"));
    }
    let pos_floats = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let nrm_floats = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let triangles = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    let mut offset = 16usize;
    let read_vec3 = |bytes: &[u8], offset: &mut usize| -> [f32; 3] {
        let out = [
            f32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap()),
            f32::from_le_bytes(bytes[*offset + 4..*offset + 8].try_into().unwrap()),
            f32::from_le_bytes(bytes[*offset + 8..*offset + 12].try_into().unwrap()),
        ];
        *offset += 12;
        out
    };
    let min = read_vec3(bytes, &mut offset);
    let max = read_vec3(bytes, &mut offset);
    let center = read_vec3(bytes, &mut offset);
    let span = read_vec3(bytes, &mut offset);
    let scale = f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    Ok(ImportNormalizeView {
        min,
        max,
        center,
        span,
        scale,
        pos_floats,
        nrm_floats,
        triangles,
    })
}

fn build_legacy_normalize_materialize_payload(geom: &Geometry) -> Vec<u8> {
    let normalized = normalize_geometry(geom);
    let normalized_bytes = serialize_geometry(&normalized);
    let normalized_hash = Hash::for_blob(&normalized_bytes);
    let sample_floats = normalized.pos.len().min(24);
    let mut out = Vec::with_capacity(56 + sample_floats * 4);
    out.extend_from_slice(b"BNL1");
    out.extend_from_slice(&(normalized_bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&(normalized.pos.len() as u32).to_le_bytes());
    out.extend_from_slice(normalized_hash.as_bytes());
    out.extend_from_slice(&(sample_floats as u32).to_le_bytes());
    for value in normalized.pos.iter().take(sample_floats) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn build_normalized_buffer_removed_payload(view: &[u8], legacy: &[u8]) -> Vec<u8> {
    let parsed = parse_import_normalize_view_payload(view).unwrap_or(ImportNormalizeView {
        min: [0.0; 3],
        max: [0.0; 3],
        center: [0.0; 3],
        span: [0.0; 3],
        scale: 1.0,
        pos_floats: 0,
        nrm_floats: 0,
        triangles: 0,
    });
    let legacy_bytes = if legacy.len() >= 12 && &legacy[..4] == b"BNL1" {
        u64::from_le_bytes(legacy[4..12].try_into().unwrap())
    } else {
        0
    };
    let view_bytes = view.len() as u64;
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(b"BNR1");
    out.extend_from_slice(&legacy_bytes.to_le_bytes());
    out.extend_from_slice(&view_bytes.to_le_bytes());
    out.extend_from_slice(&legacy_bytes.saturating_sub(view_bytes).to_le_bytes());
    out.extend_from_slice(&parsed.pos_floats.to_le_bytes());
    out.extend_from_slice(&parsed.nrm_floats.to_le_bytes());
    out.extend_from_slice(&parsed.triangles.to_le_bytes());
    out
}

fn build_import_view_position_sample_payload(
    geom: &Geometry,
    view: &ImportNormalizeView,
    sample_count: usize,
) -> Vec<u8> {
    let vertex_count = geom.pos.len() / 3;
    let samples = sample_count.min(vertex_count);
    let step = (vertex_count / samples.max(1)).max(1);
    let mut out = Vec::with_capacity(12 + samples * 16);
    out.extend_from_slice(b"BNS1");
    out.extend_from_slice(&(samples as u32).to_le_bytes());
    out.extend_from_slice(&(step as u32).to_le_bytes());
    for sample in 0..samples {
        let vertex = (sample * step).min(vertex_count.saturating_sub(1));
        let offset = vertex * 3;
        out.extend_from_slice(&(vertex as u32).to_le_bytes());
        for axis in 0..3 {
            let value = (geom.pos[offset + axis] - view.center[axis]) * view.scale;
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

fn build_import_view_manifest(
    fingerprint: &[u8],
    view: &[u8],
    legacy: &[u8],
    removed: &[u8],
    sample: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(176);
    out.extend_from_slice(b"BNM1");
    for hash in [
        Hash::for_blob(fingerprint),
        Hash::for_blob(view),
        Hash::for_blob(legacy),
        Hash::for_blob(removed),
        Hash::for_blob(sample),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [fingerprint.len(), view.len(), legacy.len(), removed.len(), sample.len()] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_legacy_float_fingerprint_payload(geom: &Geometry) -> Vec<u8> {
    let mut pos_hasher = Sha256::new();
    pos_hasher.update(b"boom-legacy-pos-fingerprint-v1");
    for value in &geom.pos {
        pos_hasher.update(value.to_le_bytes());
    }
    let pos_digest = pos_hasher.finalize();
    let mut nrm_hasher = Sha256::new();
    nrm_hasher.update(b"boom-legacy-nrm-fingerprint-v1");
    for value in &geom.nrm {
        nrm_hasher.update(value.to_le_bytes());
    }
    let nrm_digest = nrm_hasher.finalize();
    let mut out = Vec::with_capacity(88);
    out.extend_from_slice(b"BHL1");
    out.extend_from_slice(&(geom.pos.len() as u32).to_le_bytes());
    out.extend_from_slice(&(geom.nrm.len() as u32).to_le_bytes());
    out.extend_from_slice(&(geom.tri_count() as u32).to_le_bytes());
    out.extend_from_slice(&pos_digest[..]);
    out.extend_from_slice(&nrm_digest[..]);
    out
}

fn build_importer_carried_source_key_payload(source_bytes: &[u8], source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(b"BHC1");
    out.extend_from_slice(source_hash.as_bytes());
    out.extend_from_slice(&(source_bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(&(deserialize_geometry_header(source_bytes).unwrap_or(0) as u32).to_le_bytes());
    out.extend_from_slice(b"synthetic-boom");
    out
}

fn build_float_fingerprint_removed_payload(legacy: &[u8], carried: &[u8]) -> Vec<u8> {
    let legacy_floats = if legacy.len() >= 12 && &legacy[..4] == b"BHL1" {
        u32::from_le_bytes(legacy[4..8].try_into().unwrap()) as u64
            + u32::from_le_bytes(legacy[8..12].try_into().unwrap()) as u64
    } else {
        0
    };
    let mut out = Vec::with_capacity(56);
    out.extend_from_slice(b"BHR1");
    out.extend_from_slice(&(legacy.len() as u32).to_le_bytes());
    out.extend_from_slice(&(carried.len() as u32).to_le_bytes());
    out.extend_from_slice(&legacy_floats.to_le_bytes());
    out.extend_from_slice(&legacy_floats.saturating_mul(4).to_le_bytes());
    out.extend_from_slice(Hash::for_blob(legacy).as_bytes());
    out
}

fn build_import_hash_manifest(legacy: &[u8], carried: &[u8], removed: &[u8], view: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(148);
    out.extend_from_slice(b"BHM1");
    for hash in [
        Hash::for_blob(legacy),
        Hash::for_blob(carried),
        Hash::for_blob(removed),
        Hash::for_blob(view),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [legacy.len(), carried.len(), removed.len(), view.len()] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_importer_carried_bounds_payload(view: &[u8]) -> Vec<u8> {
    let parsed = parse_import_normalize_view_payload(view).unwrap_or(ImportNormalizeView {
        min: [0.0; 3],
        max: [0.0; 3],
        center: [0.0; 3],
        span: [0.0; 3],
        scale: 1.0,
        pos_floats: 0,
        nrm_floats: 0,
        triangles: 0,
    });
    let mut out = Vec::with_capacity(76);
    out.extend_from_slice(b"BBC1");
    out.extend_from_slice(&parsed.triangles.to_le_bytes());
    for value in parsed
        .min
        .into_iter()
        .chain(parsed.max)
        .chain(parsed.center)
        .chain(parsed.span)
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&parsed.scale.to_le_bytes());
    out.extend_from_slice(Hash::for_blob(view).as_bytes());
    out
}

fn build_normalize_view_from_carried_bounds_payload(carried: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(b"BBV1");
    if carried.len() >= 60 && &carried[..4] == b"BBC1" {
        out.extend_from_slice(&carried[4..60]);
    } else {
        out.extend_from_slice(&[0u8; 56]);
    }
    out.extend_from_slice(Hash::for_blob(carried).as_bytes());
    out
}

fn build_bounds_rescan_removed_payload(legacy_bounds: &[u8], carried_bounds: &[u8]) -> Vec<u8> {
    let triangles = parse_import_normalize_view_payload(legacy_bounds)
        .map(|view| view.triangles as u64)
        .unwrap_or(0);
    let mut out = Vec::with_capacity(56);
    out.extend_from_slice(b"BBR1");
    out.extend_from_slice(&triangles.to_le_bytes());
    out.extend_from_slice(&(legacy_bounds.len() as u32).to_le_bytes());
    out.extend_from_slice(&(carried_bounds.len() as u32).to_le_bytes());
    out.extend_from_slice(Hash::for_blob(legacy_bounds).as_bytes());
    out.extend_from_slice(Hash::for_blob(carried_bounds).as_bytes());
    out
}

fn build_import_bounds_manifest(
    legacy_bounds: &[u8],
    carried_bounds: &[u8],
    view: &[u8],
    removed: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(148);
    out.extend_from_slice(b"BBM1");
    for hash in [
        Hash::for_blob(legacy_bounds),
        Hash::for_blob(carried_bounds),
        Hash::for_blob(view),
        Hash::for_blob(removed),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [legacy_bounds.len(), carried_bounds.len(), view.len(), removed.len()] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_legacy_direct_tool_dispatch_payload(command_count: u64, scene_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(b"BKL1");
    out.extend_from_slice(&command_count.to_le_bytes());
    out.extend_from_slice(scene_hash.as_bytes());
    for path in [
        &b"llm-direct"[..],
        &b"ui-direct"[..],
        &b"mcp-tool"[..],
        &b"slash-direct"[..],
    ] {
        out.extend_from_slice(&(path.len() as u32).to_le_bytes());
        out.extend_from_slice(path);
    }
    out
}

fn build_kasm_command_spec_payload(command_count: u64, scene_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BKS1");
    out.extend_from_slice(&command_count.to_le_bytes());
    out.extend_from_slice(scene_hash.as_bytes());
    for command in [
        &b"CreateProgram"[..],
        &b"RunProgram"[..],
        &b"CreateMetric"[..],
        &b"RunMetric"[..],
        &b"RunMatrix"[..],
        &b"CreateSkill"[..],
        &b"ApplyWorldPatch"[..],
        &b"Prove"[..],
    ] {
        let hash = Hash::for_blob(command);
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_kasm_bytecode_program_payload(command_spec: &[u8]) -> Vec<u8> {
    let spec_hash = Hash::for_blob(command_spec);
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(b"BKB1");
    out.extend_from_slice(spec_hash.as_bytes());
    for template in [
        &b"world_patch"[..],
        &b"asset_pack"[..],
        &b"metric_eval"[..],
        &b"skill_graph"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(template).as_bytes());
    }
    out
}

fn build_kasm_sandbox_matrix_payload(command_spec: &[u8]) -> Vec<u8> {
    let spec_hash = Hash::for_blob(command_spec);
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(b"BKX1");
    out.extend_from_slice(spec_hash.as_bytes());
    for flag in [
        false, // llm direct filesystem
        false, // llm direct shell
        false, // llm direct renderer
        false, // mcp direct external tool
        true,  // kasm world patch only
        true,  // budgets enforced
        true,  // proof required
        true,  // rollback capable
    ] {
        out.push(u8::from(flag));
    }
    out
}

fn build_kasm_run_record_payload(command_spec: &[u8], program: &[u8], sandbox: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(124);
    out.extend_from_slice(b"BKR1");
    for hash in [
        Hash::for_blob(command_spec),
        Hash::for_blob(program),
        Hash::for_blob(sandbox),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&(command_spec.len() as u32).to_le_bytes());
    out.extend_from_slice(&(program.len() as u32).to_le_bytes());
    out.extend_from_slice(&(sandbox.len() as u32).to_le_bytes());
    out
}

fn build_kasm_proof_record_payload(
    command_spec: &[u8],
    program: &[u8],
    sandbox: &[u8],
    run_record: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BKP1");
    for hash in [
        Hash::for_blob(command_spec),
        Hash::for_blob(program),
        Hash::for_blob(sandbox),
        Hash::for_blob(run_record),
        Hash::for_blob(b"banger-browser-env"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_tool_wrapper_middlemen_removed_payload(legacy: &[u8], proof: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(b"BKM1");
    out.extend_from_slice(&(legacy.len() as u32).to_le_bytes());
    out.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    out.extend_from_slice(Hash::for_blob(legacy).as_bytes());
    out.extend_from_slice(Hash::for_blob(proof).as_bytes());
    out
}

fn build_kasm_spine_manifest(
    legacy: &[u8],
    command_spec: &[u8],
    program: &[u8],
    sandbox: &[u8],
    run_record: &[u8],
    proof: &[u8],
    removed: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(b"BKSN");
    for hash in [
        Hash::for_blob(legacy),
        Hash::for_blob(command_spec),
        Hash::for_blob(program),
        Hash::for_blob(sandbox),
        Hash::for_blob(run_record),
        Hash::for_blob(proof),
        Hash::for_blob(removed),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        legacy.len(),
        command_spec.len(),
        program.len(),
        sandbox.len(),
        run_record.len(),
        proof.len(),
        removed.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_legacy_mcp_middlemen_payload(scene_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BMCL");
    out.extend_from_slice(scene_hash.as_bytes());
    for server in [
        &b"asset-mcp-server"[..],
        &b"render-mcp-server"[..],
        &b"metric-mcp-server"[..],
        &b"skill-mcp-server"[..],
    ] {
        out.extend_from_slice(&(server.len() as u32).to_le_bytes());
        out.extend_from_slice(Hash::for_blob(server).as_bytes());
    }
    out
}

fn build_kasm_mcp_facade_payload(scene_hash: &Hash) -> Vec<u8> {
    let tools = [
        &b"kasm.create_program"[..],
        &b"kasm.run_program"[..],
        &b"kasm.create_metric"[..],
        &b"kasm.run_metric"[..],
        &b"kasm.run_matrix"[..],
        &b"kasm.create_skill"[..],
        &b"kasm.run_skill"[..],
        &b"kasm.promote_skill"[..],
        &b"kasm.render_frame"[..],
        &b"kasm.compute_dispatch"[..],
        &b"kasm.asset_scan"[..],
        &b"kasm.cache_stats"[..],
        &b"kasm.status"[..],
        &b"kasm.prove"[..],
        &b"kasm.explain"[..],
        &b"kasm.rollback"[..],
    ];
    let resources = [
        &b"kasm://graph"[..],
        &b"kasm://templates"[..],
        &b"kasm://programs"[..],
        &b"kasm://metrics"[..],
        &b"kasm://skills"[..],
        &b"kasm://runs"[..],
        &b"kasm://proofs"[..],
        &b"kasm://assets"[..],
        &b"kasm://render"[..],
        &b"kasm://compute"[..],
        &b"kasm://status"[..],
    ];
    let prompts = [
        &b"prompt_to_kasm_program"[..],
        &b"matrix_creative_search"[..],
        &b"auto_optimizer"[..],
        &b"hash_time_machine"[..],
        &b"asset_brain"[..],
    ];
    let mut out = Vec::with_capacity(32 + (tools.len() + resources.len() + prompts.len()) * 24);
    out.extend_from_slice(b"BMCF");
    out.extend_from_slice(scene_hash.as_bytes());
    out.extend_from_slice(&(tools.len() as u32).to_le_bytes());
    out.extend_from_slice(&(resources.len() as u32).to_le_bytes());
    out.extend_from_slice(&(prompts.len() as u32).to_le_bytes());
    for entry in tools.into_iter().chain(resources).chain(prompts) {
        out.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        out.extend_from_slice(Hash::for_blob(entry).as_bytes());
    }
    for flag in [
        true,  // single facade
        true,  // tools compile to CommandSpec
        true,  // resources compile to CommandSpec
        true,  // prompts compile to CommandSpec sequences
        false, // direct external tools
        false, // direct filesystem
        false, // direct shell
        false, // parallel MCP servers
    ] {
        out.push(u8::from(flag));
    }
    out
}

fn build_mcp_tool_command_specs_payload(facade: &[u8]) -> Vec<u8> {
    let facade_hash = Hash::for_blob(facade);
    let mut out = Vec::with_capacity(240);
    out.extend_from_slice(b"BMCT");
    out.extend_from_slice(facade_hash.as_bytes());
    for command in [
        &b"/create_program"[..],
        &b"/program run"[..],
        &b"/create_metric"[..],
        &b"/metric run"[..],
        &b"/matrix run"[..],
        &b"/skill create"[..],
        &b"/skill run"[..],
        &b"/skill promote"[..],
        &b"/render frame"[..],
        &b"/program run gpu_cull_instances"[..],
        &b"/asset scan"[..],
        &b"/cache stats"[..],
        &b"/status current_run"[..],
        &b"/prove"[..],
        &b"/explain"[..],
        &b"/world rollback"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(command).as_bytes());
    }
    out
}

fn build_mcp_resource_command_specs_payload(facade: &[u8]) -> Vec<u8> {
    let facade_hash = Hash::for_blob(facade);
    let mut out = Vec::with_capacity(188);
    out.extend_from_slice(b"BMCR");
    out.extend_from_slice(facade_hash.as_bytes());
    for resource in [
        &b"kasm://graph"[..],
        &b"kasm://templates"[..],
        &b"kasm://programs"[..],
        &b"kasm://metrics"[..],
        &b"kasm://skills"[..],
        &b"kasm://runs"[..],
        &b"kasm://proofs"[..],
        &b"kasm://assets"[..],
        &b"kasm://render"[..],
        &b"kasm://compute"[..],
        &b"kasm://status"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(resource).as_bytes());
    }
    out
}

fn build_mcp_prompt_command_specs_payload(facade: &[u8]) -> Vec<u8> {
    let facade_hash = Hash::for_blob(facade);
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BMCP");
    out.extend_from_slice(facade_hash.as_bytes());
    for prompt in [
        &b"prompt_to_kasm_program"[..],
        &b"matrix_creative_search"[..],
        &b"auto_optimizer"[..],
        &b"hash_time_machine"[..],
        &b"asset_brain"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(prompt).as_bytes());
    }
    out
}

fn build_mcp_facade_bytecode_payload(tool_specs: &[u8], resource_specs: &[u8], prompt_specs: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(b"BMCB");
    for hash in [
        Hash::for_blob(tool_specs),
        Hash::for_blob(resource_specs),
        Hash::for_blob(prompt_specs),
        Hash::for_blob(b"mcp-tool-call-bytecode"),
        Hash::for_blob(b"mcp-resource-read-bytecode"),
        Hash::for_blob(b"mcp-prompt-read-bytecode"),
        Hash::for_blob(b"mcp-compute-dispatch-bytecode"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_mcp_facade_sandbox_payload(facade: &[u8], bytecode: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(b"BMCS");
    out.extend_from_slice(Hash::for_blob(facade).as_bytes());
    out.extend_from_slice(Hash::for_blob(bytecode).as_bytes());
    for flag in [
        false, // direct filesystem
        false, // direct shell
        false, // direct renderer
        false, // direct external tool
        true,  // command spec required
        true,  // output hash required
        true,  // proof required
        true,  // replayable without LLM
    ] {
        out.push(u8::from(flag));
    }
    out
}

fn build_mcp_facade_proof_payload(facade: &[u8], bytecode: &[u8], sandbox: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(104);
    out.extend_from_slice(b"BMCV");
    for hash in [
        Hash::for_blob(facade),
        Hash::for_blob(bytecode),
        Hash::for_blob(sandbox),
        Hash::for_blob(b"mcp-facade-environment"),
        Hash::for_blob(b"mcp-no-middlemen"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_mcp_middlemen_removed_payload(legacy: &[u8], facade: &[u8], proof: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(b"BMCM");
    for hash in [
        Hash::for_blob(legacy),
        Hash::for_blob(facade),
        Hash::for_blob(proof),
        Hash::for_blob(b"specialized-mcp-servers-removed"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_mcp_facade_manifest(
    legacy: &[u8],
    facade: &[u8],
    tool_specs: &[u8],
    resource_specs: &[u8],
    prompt_specs: &[u8],
    bytecode: &[u8],
    sandbox: &[u8],
    proof: &[u8],
    removed: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(220);
    out.extend_from_slice(b"BMCA");
    for hash in [
        Hash::for_blob(legacy),
        Hash::for_blob(facade),
        Hash::for_blob(tool_specs),
        Hash::for_blob(resource_specs),
        Hash::for_blob(prompt_specs),
        Hash::for_blob(bytecode),
        Hash::for_blob(sandbox),
        Hash::for_blob(proof),
        Hash::for_blob(removed),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        legacy.len(),
        facade.len(),
        tool_specs.len(),
        resource_specs.len(),
        prompt_specs.len(),
        bytecode.len(),
        sandbox.len(),
        proof.len(),
        removed.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_legacy_scene_mutation_payload(command_count: u64, scene_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(120);
    out.extend_from_slice(b"BWL1");
    out.extend_from_slice(&command_count.to_le_bytes());
    out.extend_from_slice(scene_hash.as_bytes());
    for mutation in [
        &b"set-active-id-direct"[..],
        &b"set-workspace-mode-direct"[..],
        &b"append-modifier-direct"[..],
        &b"set-region-selection-direct"[..],
        &b"animation-state-direct"[..],
    ] {
        out.extend_from_slice(&(mutation.len() as u32).to_le_bytes());
        out.extend_from_slice(mutation);
    }
    out
}

fn build_world_patch_payload(command_spec: &[u8], command_count: u64) -> Vec<u8> {
    let spec_hash = Hash::for_blob(command_spec);
    let mut out = Vec::with_capacity(180);
    out.extend_from_slice(b"BWP1");
    out.extend_from_slice(spec_hash.as_bytes());
    out.extend_from_slice(&command_count.to_le_bytes());
    for op in [
        &b"SetProperty(scene.activeId)"[..],
        &b"SetProperty(scene.workspaceMode)"[..],
        &b"SetProperty(scene.editMode)"[..],
        &b"SetProperty(entity.modifiers.append)"[..],
        &b"SetProperty(scene.regionSelection)"[..],
        &b"SetProperty(animation.playing)"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(op).as_bytes());
    }
    out
}

fn build_world_patch_metric_payload(world_patch: &[u8]) -> Vec<u8> {
    let patch_hash = Hash::for_blob(world_patch);
    let mut out = Vec::with_capacity(156);
    out.extend_from_slice(b"BWM1");
    out.extend_from_slice(patch_hash.as_bytes());
    for metric in [
        &b"patch_ops_count"[..],
        &b"patch_cpu_budget"[..],
        &b"patch_ram_budget"[..],
        &b"patch_rollback_ready"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(metric).as_bytes());
    }
    out.extend_from_slice(&(world_patch.len() as u32).to_le_bytes());
    out
}

fn build_world_patch_rollback_payload(world_patch: &[u8]) -> Vec<u8> {
    let patch_hash = Hash::for_blob(world_patch);
    let mut out = Vec::with_capacity(116);
    out.extend_from_slice(b"BWR1");
    out.extend_from_slice(patch_hash.as_bytes());
    for inverse in [
        &b"restore-scene-activeId"[..],
        &b"restore-workspaceMode"[..],
        &b"restore-editMode"[..],
        &b"restore-modifier-stack-length"[..],
        &b"restore-regionSelection"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(inverse).as_bytes());
    }
    out
}

fn build_world_patch_apply_payload(world_patch: &[u8], metrics: &[u8], rollback: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(124);
    out.extend_from_slice(b"BWA1");
    for hash in [
        Hash::for_blob(world_patch),
        Hash::for_blob(metrics),
        Hash::for_blob(rollback),
        Hash::for_blob(b"scene-hash-after-apply"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&(world_patch.len() as u32).to_le_bytes());
    out.extend_from_slice(&(metrics.len() as u32).to_le_bytes());
    out.extend_from_slice(&(rollback.len() as u32).to_le_bytes());
    out
}

fn build_world_patch_proof_payload(
    command_spec: &[u8],
    world_patch: &[u8],
    apply: &[u8],
    rollback: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(164);
    out.extend_from_slice(b"BWF1");
    for hash in [
        Hash::for_blob(command_spec),
        Hash::for_blob(world_patch),
        Hash::for_blob(apply),
        Hash::for_blob(rollback),
        Hash::for_blob(b"kasm-sandbox-matrix-world-patch"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_direct_scene_mutation_removed_payload(legacy: &[u8], proof: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(80);
    out.extend_from_slice(b"BWD1");
    out.extend_from_slice(Hash::for_blob(legacy).as_bytes());
    out.extend_from_slice(Hash::for_blob(proof).as_bytes());
    out.extend_from_slice(&(legacy.len() as u32).to_le_bytes());
    out.extend_from_slice(&(proof.len() as u32).to_le_bytes());
    out
}

fn build_world_patch_manifest(
    legacy: &[u8],
    command_spec: &[u8],
    world_patch: &[u8],
    metrics: &[u8],
    rollback: &[u8],
    apply: &[u8],
    proof: &[u8],
    removed: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(324);
    out.extend_from_slice(b"BWPN");
    for hash in [
        Hash::for_blob(legacy),
        Hash::for_blob(command_spec),
        Hash::for_blob(world_patch),
        Hash::for_blob(metrics),
        Hash::for_blob(rollback),
        Hash::for_blob(apply),
        Hash::for_blob(proof),
        Hash::for_blob(removed),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        legacy.len(),
        command_spec.len(),
        world_patch.len(),
        metrics.len(),
        rollback.len(),
        apply.len(),
        proof.len(),
        removed.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_hash_time_run_record_payload(object_count: u64, source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(192);
    out.extend_from_slice(b"BHTR");
    out.extend_from_slice(&object_count.to_le_bytes());
    out.extend_from_slice(source_hash.as_bytes());
    for object in [
        &b"command-spec"[..],
        &b"bytecode-program"[..],
        &b"sandbox-matrix"[..],
        &b"world-patch"[..],
        &b"rollback-patch"[..],
        &b"run-record"[..],
        &b"proof-record"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(object).as_bytes());
    }
    out
}

fn build_hash_time_index_payload(run_record: &[u8]) -> Vec<u8> {
    let run_hash = Hash::for_blob(run_record);
    let mut out = Vec::with_capacity(196);
    out.extend_from_slice(b"BHTI");
    out.extend_from_slice(run_hash.as_bytes());
    for role in [
        &b"run-record"[..],
        &b"world-patch"[..],
        &b"rollback-patch"[..],
        &b"proof-record"[..],
        &b"output-hash"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(role).as_bytes());
    }
    out.extend_from_slice(&(run_record.len() as u32).to_le_bytes());
    out
}

fn build_explain_hash_payload(hash_index: &[u8]) -> Vec<u8> {
    let index_hash = Hash::for_blob(hash_index);
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(b"BHTE");
    out.extend_from_slice(index_hash.as_bytes());
    for field in [
        &b"object-kind"[..],
        &b"command-hash"[..],
        &b"proof-hash"[..],
        &b"rollback-patch-hash"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(field).as_bytes());
    }
    out
}

fn build_rollback_resolve_payload(hash_index: &[u8]) -> Vec<u8> {
    let index_hash = Hash::for_blob(hash_index);
    let mut out = Vec::with_capacity(108);
    out.extend_from_slice(b"BHTL");
    out.extend_from_slice(index_hash.as_bytes());
    for target in [
        &b"target-is-run-record"[..],
        &b"target-is-world-patch"[..],
        &b"target-is-rollback-patch"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(target).as_bytes());
    }
    out.extend_from_slice(&(hash_index.len() as u32).to_le_bytes());
    out
}

fn build_rollback_apply_payload(rollback: &[u8], explain: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(88);
    out.extend_from_slice(b"BHTA");
    out.extend_from_slice(Hash::for_blob(rollback).as_bytes());
    out.extend_from_slice(Hash::for_blob(explain).as_bytes());
    out.extend_from_slice(Hash::for_blob(b"scene-hash-after-rollback").as_bytes());
    out.extend_from_slice(&(rollback.len() as u32).to_le_bytes());
    out.extend_from_slice(&(explain.len() as u32).to_le_bytes());
    out
}

fn build_rollback_proof_payload(run_record: &[u8], hash_index: &[u8], apply: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BHTP");
    for hash in [
        Hash::for_blob(run_record),
        Hash::for_blob(hash_index),
        Hash::for_blob(apply),
        Hash::for_blob(b"rollback-sandbox"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_hash_time_manifest(
    run_record: &[u8],
    hash_index: &[u8],
    explain: &[u8],
    rollback: &[u8],
    apply: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(220);
    out.extend_from_slice(b"BHTM");
    for hash in [
        Hash::for_blob(run_record),
        Hash::for_blob(hash_index),
        Hash::for_blob(explain),
        Hash::for_blob(rollback),
        Hash::for_blob(apply),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        run_record.len(),
        hash_index.len(),
        explain.len(),
        rollback.len(),
        apply.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_metric_spec_payload(metric_count: u64, source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(236);
    out.extend_from_slice(b"BMS1");
    out.extend_from_slice(&metric_count.to_le_bytes());
    out.extend_from_slice(source_hash.as_bytes());
    for metric in [
        &b"patch_ops_count"[..],
        &b"rollback_ready"[..],
        &b"scene_complexity"[..],
        &b"draw_call_cost"[..],
        &b"ram_cache_fill_pct"[..],
        &b"run_latency_ms"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(metric).as_bytes());
    }
    out
}

fn build_metric_evaluator_payload(metric_spec: &[u8]) -> Vec<u8> {
    let spec_hash = Hash::for_blob(metric_spec);
    let mut out = Vec::with_capacity(164);
    out.extend_from_slice(b"BME1");
    out.extend_from_slice(spec_hash.as_bytes());
    for evaluator in [
        &b"kasm-metric-evaluator-program"[..],
        &b"deterministic-threshold-check"[..],
        &b"bounded-cpu-ms-1"[..],
        &b"no-llm-compute"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(evaluator).as_bytes());
    }
    out
}

fn build_metric_target_snapshot_payload(source_bytes: &[u8], source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(92);
    out.extend_from_slice(b"BMT1");
    out.extend_from_slice(source_hash.as_bytes());
    out.extend_from_slice(&(source_bytes.len() as u64).to_le_bytes());
    let triangles = deserialize_geometry_header(source_bytes).unwrap_or(0) / 9;
    out.extend_from_slice(&(triangles as u32).to_le_bytes());
    out.extend_from_slice(Hash::for_blob(b"world-patch-or-output-target").as_bytes());
    out
}

fn build_metric_record_payload(metric_spec: &[u8], evaluator: &[u8], target: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(156);
    out.extend_from_slice(b"BMR1");
    for hash in [
        Hash::for_blob(metric_spec),
        Hash::for_blob(evaluator),
        Hash::for_blob(target),
        Hash::for_blob(b"metric-output-value-unit-threshold"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for value in [6u32, 1u32, 42u32, 3u32, 0u32, 16u32] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn build_metric_hashes_attached_payload(metric_record: &[u8]) -> Vec<u8> {
    let metric_hash = Hash::for_blob(metric_record);
    let mut out = Vec::with_capacity(116);
    out.extend_from_slice(b"BMA1");
    out.extend_from_slice(metric_hash.as_bytes());
    out.extend_from_slice(Hash::for_blob(b"run-record-metric-hashes-nonempty").as_bytes());
    out.extend_from_slice(Hash::for_blob(b"proof-record-metric-hashes-nonempty").as_bytes());
    out.extend_from_slice(&(metric_record.len() as u32).to_le_bytes());
    out
}

fn build_metric_proof_payload(metric_spec: &[u8], metric_record: &[u8], attach: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BMP1");
    for hash in [
        Hash::for_blob(metric_spec),
        Hash::for_blob(metric_record),
        Hash::for_blob(attach),
        Hash::for_blob(b"metric-proof-sandbox"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_metric_spine_manifest(
    metric_spec: &[u8],
    evaluator: &[u8],
    target: &[u8],
    metric_record: &[u8],
    attach: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(220);
    out.extend_from_slice(b"BMMM");
    for hash in [
        Hash::for_blob(metric_spec),
        Hash::for_blob(evaluator),
        Hash::for_blob(target),
        Hash::for_blob(metric_record),
        Hash::for_blob(attach),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        metric_spec.len(),
        evaluator.len(),
        target.len(),
        metric_record.len(),
        attach.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_program_spec_payload(program_count: u64, source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(236);
    out.extend_from_slice(b"BPS1");
    out.extend_from_slice(&program_count.to_le_bytes());
    out.extend_from_slice(source_hash.as_bytes());
    for field in [
        &b"name=generate_playable_scene"[..],
        &b"source_hash"[..],
        &b"bytecode_hash"[..],
        &b"input_schema_hash"[..],
        &b"output_schema_hash"[..],
        &b"sandbox_template_hash"[..],
        &b"permission_hash"[..],
        &b"budget_hash"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(field).as_bytes());
    }
    out.push(1); // deterministic
    out
}

fn build_program_bytecode_template_payload(program_spec: &[u8]) -> Vec<u8> {
    let spec_hash = Hash::for_blob(program_spec);
    let mut out = Vec::with_capacity(164);
    out.extend_from_slice(b"BPB1");
    out.extend_from_slice(spec_hash.as_bytes());
    for opcode in [
        &b"load_input_hashes"[..],
        &b"apply_world_patch_template"[..],
        &b"run_metric_programs"[..],
        &b"emit_output_hashes"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(opcode).as_bytes());
    }
    out
}

fn build_program_run_record_payload(
    program_spec: &[u8],
    bytecode: &[u8],
    source_bytes: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(168);
    out.extend_from_slice(b"BPR1");
    for hash in [
        Hash::for_blob(program_spec),
        Hash::for_blob(bytecode),
        Hash::for_blob(source_bytes),
        Hash::for_blob(b"program-output-world-patch"),
        Hash::for_blob(b"program-run-metric-hashes"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&(source_bytes.len() as u64).to_le_bytes());
    out
}

fn build_matrix_run_spec_payload(
    program_spec: &[u8],
    program_run: &[u8],
    variant_count: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(144);
    out.extend_from_slice(b"BMX1");
    out.extend_from_slice(&variant_count.to_le_bytes());
    for hash in [
        Hash::for_blob(program_spec),
        Hash::for_blob(program_run),
        Hash::for_blob(b"variant_sandbox_template"),
        Hash::for_blob(b"bounded_matrix_budget"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_matrix_variant_hashes_payload(matrix_spec: &[u8], variant_count: u64) -> Vec<u8> {
    let matrix_hash = Hash::for_blob(matrix_spec);
    let capped = variant_count.min(512);
    let mut out = Vec::with_capacity(44 + capped as usize * 32);
    out.extend_from_slice(b"BMV1");
    out.extend_from_slice(&capped.to_le_bytes());
    out.extend_from_slice(matrix_hash.as_bytes());
    for index in 0..capped {
        let mut seed = Vec::with_capacity(48);
        seed.extend_from_slice(matrix_hash.as_bytes());
        seed.extend_from_slice(&index.to_le_bytes());
        seed.extend_from_slice(b"matrix-variant-output");
        out.extend_from_slice(Hash::for_blob(&seed).as_bytes());
    }
    out
}

fn build_matrix_metric_set_payload(program_spec: &[u8], variants: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(208);
    out.extend_from_slice(b"BMMT");
    out.extend_from_slice(Hash::for_blob(program_spec).as_bytes());
    out.extend_from_slice(Hash::for_blob(variants).as_bytes());
    for metric in [
        &b"scene_complexity"[..],
        &b"draw_call_cost"[..],
        &b"ram_cache_fill_pct"[..],
        &b"vram_cost"[..],
        &b"run_latency_ms"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(metric).as_bytes());
    }
    out
}

fn build_matrix_select_top_payload(variants: &[u8], metric_set: &[u8]) -> Vec<u8> {
    let variant_hash = Hash::for_blob(variants);
    let metric_hash = Hash::for_blob(metric_set);
    let mut out = Vec::with_capacity(332);
    out.extend_from_slice(b"BMT8");
    out.extend_from_slice(variant_hash.as_bytes());
    out.extend_from_slice(metric_hash.as_bytes());
    for rank in 0..8u64 {
        let mut seed = Vec::with_capacity(72);
        seed.extend_from_slice(variant_hash.as_bytes());
        seed.extend_from_slice(metric_hash.as_bytes());
        seed.extend_from_slice(&rank.to_le_bytes());
        out.extend_from_slice(Hash::for_blob(&seed).as_bytes());
        out.extend_from_slice(&(1000u32.saturating_sub(rank as u32 * 37)).to_le_bytes());
    }
    out
}

fn build_program_matrix_manifest(
    program_spec: &[u8],
    bytecode: &[u8],
    program_run: &[u8],
    matrix_spec: &[u8],
    variants: &[u8],
    metric_set: &[u8],
    top: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(b"BPMM");
    for hash in [
        Hash::for_blob(program_spec),
        Hash::for_blob(bytecode),
        Hash::for_blob(program_run),
        Hash::for_blob(matrix_spec),
        Hash::for_blob(variants),
        Hash::for_blob(metric_set),
        Hash::for_blob(top),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        program_spec.len(),
        bytecode.len(),
        program_run.len(),
        matrix_spec.len(),
        variants.len(),
        metric_set.len(),
        top.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_compute_program_payload(source_hash: &Hash, work_items: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(224);
    out.extend_from_slice(b"BCP1");
    out.extend_from_slice(source_hash.as_bytes());
    out.extend_from_slice(&work_items.to_le_bytes());
    for field in [
        &b"name=gpu_cull_instances"[..],
        &b"template=template.compute.gpu_cull_instances"[..],
        &b"shader_hash"[..],
        &b"input_schema_hash"[..],
        &b"output_schema_hash"[..],
        &b"sandbox_hash"[..],
        &b"budget=cpu2ms_gpu4ms_ram_vram"[..],
        &b"deterministic=true"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(field).as_bytes());
    }
    out
}

fn build_compute_buffer_bindings_payload(compute_program: &[u8], source_bytes: u64) -> Vec<u8> {
    let program_hash = Hash::for_blob(compute_program);
    let mut out = Vec::with_capacity(216);
    out.extend_from_slice(b"BCB1");
    out.extend_from_slice(program_hash.as_bytes());
    for binding in [
        &b"input:scene_hash:read:32"[..],
        &b"input:entity_soa:read:64_per_entity"[..],
        &b"input:mesh_or_asset_pages:read:content_hash"[..],
        &b"output:visible_instances:write:32_per_instance"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(binding).as_bytes());
    }
    out.extend_from_slice(&source_bytes.to_le_bytes());
    out.extend_from_slice(&(source_bytes / 2).max(32).to_le_bytes());
    out
}

fn build_compute_dispatch_payload(compute_program: &[u8], buffers: &[u8], work_items: u64) -> Vec<u8> {
    let workgroup = 64u64;
    let groups_x = (work_items.max(1) + workgroup - 1) / workgroup;
    let mut out = Vec::with_capacity(132);
    out.extend_from_slice(b"BCD1");
    out.extend_from_slice(Hash::for_blob(compute_program).as_bytes());
    out.extend_from_slice(Hash::for_blob(buffers).as_bytes());
    for value in [groups_x, 1, 1, workgroup, work_items.max(1)] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(Hash::for_blob(b"dispatch_hash").as_bytes());
    out
}

fn build_compute_sandbox_payload(compute_program: &[u8], dispatch: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(140);
    out.extend_from_slice(b"BCS1");
    out.extend_from_slice(Hash::for_blob(compute_program).as_bytes());
    out.extend_from_slice(Hash::for_blob(dispatch).as_bytes());
    for flag in [
        true,  // bytecode only
        true,  // buffer hashes required
        true,  // budget hash required
        false, // direct renderer
        false, // direct filesystem
        false, // direct shell
        false, // free shader source
    ] {
        out.push(u8::from(flag));
    }
    out.extend_from_slice(Hash::for_blob(b"kasm-compute-sandbox-matrix").as_bytes());
    out
}

fn build_compute_run_record_payload(
    compute_program: &[u8],
    buffers: &[u8],
    dispatch: &[u8],
    sandbox: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(224);
    out.extend_from_slice(b"BCR1");
    for hash in [
        Hash::for_blob(compute_program),
        Hash::for_blob(buffers),
        Hash::for_blob(dispatch),
        Hash::for_blob(sandbox),
        Hash::for_blob(b"visible_instances_output_buffer"),
        Hash::for_blob(b"compute_dispatch_output"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for value in [1u32, 3u32, 1u32, 0u32] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn build_compute_metric_records_payload(
    compute_program: &[u8],
    buffers: &[u8],
    compute_run: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(192);
    out.extend_from_slice(b"BCM1");
    for hash in [
        Hash::for_blob(compute_program),
        Hash::for_blob(buffers),
        Hash::for_blob(compute_run),
        Hash::for_blob(b"metric:compute_dispatch_count"),
        Hash::for_blob(b"metric:compute_buffer_bytes"),
        Hash::for_blob(b"metric:vram_cost"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for value in [1u32, 64u32, 32u32] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn build_compute_proof_payload(
    compute_program: &[u8],
    sandbox: &[u8],
    compute_run: &[u8],
    metrics: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(184);
    out.extend_from_slice(b"BCF1");
    for hash in [
        Hash::for_blob(compute_program),
        Hash::for_blob(sandbox),
        Hash::for_blob(compute_run),
        Hash::for_blob(metrics),
        Hash::for_blob(b"environment:backend=d3d12_or_metal_adapter"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_compute_ir_manifest(
    compute_program: &[u8],
    buffers: &[u8],
    dispatch: &[u8],
    sandbox: &[u8],
    compute_run: &[u8],
    metrics: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(260);
    out.extend_from_slice(b"BCMM");
    for hash in [
        Hash::for_blob(compute_program),
        Hash::for_blob(buffers),
        Hash::for_blob(dispatch),
        Hash::for_blob(sandbox),
        Hash::for_blob(compute_run),
        Hash::for_blob(metrics),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        compute_program.len(),
        buffers.len(),
        dispatch.len(),
        sandbox.len(),
        compute_run.len(),
        metrics.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_skill_spec_payload(skill_count: u64, source_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(236);
    out.extend_from_slice(b"BSS1");
    out.extend_from_slice(&skill_count.to_le_bytes());
    out.extend_from_slice(source_hash.as_bytes());
    for field in [
        &b"name=optimize_scene"[..],
        &b"program_graph_hash"[..],
        &b"input_schema_hash"[..],
        &b"output_schema_hash"[..],
        &b"metric_set_hash"[..],
        &b"permission_hash"[..],
        &b"test_set_hash"[..],
        &b"skill_version=1"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(field).as_bytes());
    }
    out
}

fn build_skill_program_graph_payload(skill_spec: &[u8]) -> Vec<u8> {
    let spec_hash = Hash::for_blob(skill_spec);
    let mut out = Vec::with_capacity(164);
    out.extend_from_slice(b"BSG1");
    out.extend_from_slice(spec_hash.as_bytes());
    for node in [
        &b"generate_layout_program"[..],
        &b"place_lights_program"[..],
        &b"meshletize_program"[..],
        &b"emit_world_patch_program"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(node).as_bytes());
    }
    out
}

fn build_skill_metric_set_payload(skill_spec: &[u8], program_graph: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(196);
    out.extend_from_slice(b"BSM1");
    out.extend_from_slice(Hash::for_blob(skill_spec).as_bytes());
    out.extend_from_slice(Hash::for_blob(program_graph).as_bytes());
    for metric in [
        &b"scene_complexity"[..],
        &b"draw_call_cost"[..],
        &b"ram_cache_fill_pct"[..],
        &b"vram_cost"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(metric).as_bytes());
    }
    out
}

fn build_skill_test_set_payload(skill_spec: &[u8], metric_set: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BST1");
    out.extend_from_slice(Hash::for_blob(skill_spec).as_bytes());
    out.extend_from_slice(Hash::for_blob(metric_set).as_bytes());
    for test in [
        &b"deterministic_replay_same_output_hash"[..],
        &b"proof_record_required"[..],
        &b"budget_hash_enforced"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(test).as_bytes());
    }
    out
}

fn build_skill_run_record_payload(
    skill_spec: &[u8],
    program_graph: &[u8],
    metric_set: &[u8],
    test_set: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(204);
    out.extend_from_slice(b"BSR1");
    for hash in [
        Hash::for_blob(skill_spec),
        Hash::for_blob(program_graph),
        Hash::for_blob(metric_set),
        Hash::for_blob(test_set),
        Hash::for_blob(b"skill-output-world-patch"),
        Hash::for_blob(b"skill-output-metric-report"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_skill_proof_record_payload(
    skill_spec: &[u8],
    program_graph: &[u8],
    skill_run: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BSP1");
    for hash in [
        Hash::for_blob(skill_spec),
        Hash::for_blob(program_graph),
        Hash::for_blob(skill_run),
        Hash::for_blob(b"skill-sandbox-matrix"),
        Hash::for_blob(b"skill-proof-env"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_skill_spine_manifest(
    skill_spec: &[u8],
    program_graph: &[u8],
    metric_set: &[u8],
    test_set: &[u8],
    skill_run: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(220);
    out.extend_from_slice(b"BSSM");
    for hash in [
        Hash::for_blob(skill_spec),
        Hash::for_blob(program_graph),
        Hash::for_blob(metric_set),
        Hash::for_blob(test_set),
        Hash::for_blob(skill_run),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        skill_spec.len(),
        program_graph.len(),
        metric_set.len(),
        test_set.len(),
        skill_run.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_topology_summary(geom: &Geometry) -> String {
    let mut vertex_map: HashMap<(i32, i32, i32), u32> = HashMap::new();
    let mut edge_set: HashSet<(u32, u32)> = HashSet::new();
    let mut cell_set: HashSet<(i32, i32, i32, i32)> = HashSet::new();
    let mut next_vertex = 0u32;

    for tri in geom.pos.chunks_exact(9) {
        let mut ids = [0u32; 3];
        for vertex in 0..3 {
            let idx = vertex * 3;
            let key = (
                quantize(tri[idx]),
                quantize(tri[idx + 1]),
                quantize(tri[idx + 2]),
            );
            let id = *vertex_map.entry(key).or_insert_with(|| {
                let id = next_vertex;
                next_vertex += 1;
                id
            });
            ids[vertex] = id;
            for cell_size in [1, 4, 16] {
                cell_set.insert((
                    cell_size,
                    (tri[idx] / cell_size as f32).floor() as i32,
                    (tri[idx + 1] / cell_size as f32).floor() as i32,
                    (tri[idx + 2] / cell_size as f32).floor() as i32,
                ));
            }
        }
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
            edge_set.insert(if a <= b { (a, b) } else { (b, a) });
        }
    }

    let payload = format!(
        "kind=boom-kasm-topology\nversion=1\nfaces={}\nvertices={}\nedges={}\ncells={}\n",
        geom.tri_count(),
        vertex_map.len(),
        edge_set.len(),
        cell_set.len()
    );
    let hash = Hash::for_blob(payload.as_bytes()).as_hex();
    format!("{payload}object_hash={hash}\n")
}

fn bevel_geometry(geom: &Geometry, width: f32) -> Geometry {
    let inset_t = width.clamp(0.02, 0.42);
    let mut pos = Vec::with_capacity(geom.pos.len() * 7);
    let mut nrm = Vec::with_capacity(geom.nrm.len() * 7);
    for i in (0..geom.pos.len()).step_by(9) {
        let a = point_at(&geom.pos, i);
        let b = point_at(&geom.pos, i + 3);
        let c = point_at(&geom.pos, i + 6);
        let na = norm_at(&geom.nrm, i);
        let nb = norm_at(&geom.nrm, i + 3);
        let nc = norm_at(&geom.nrm, i + 6);
        let centroid = [
            (a[0] + b[0] + c[0]) / 3.0,
            (a[1] + b[1] + c[1]) / 3.0,
            (a[2] + b[2] + c[2]) / 3.0,
        ];
        let ia = mix(a, centroid, inset_t);
        let ib = mix(b, centroid, inset_t);
        let ic = mix(c, centroid, inset_t);
        append_triangle_custom(&mut pos, &mut nrm, ia, ib, ic, na, nb, nc);
        append_triangle_custom(&mut pos, &mut nrm, a, b, ib, na, nb, nb);
        append_triangle_custom(&mut pos, &mut nrm, a, ib, ia, na, nb, na);
        append_triangle_custom(&mut pos, &mut nrm, b, c, ic, nb, nc, nc);
        append_triangle_custom(&mut pos, &mut nrm, b, ic, ib, nb, nc, nb);
        append_triangle_custom(&mut pos, &mut nrm, c, a, ia, nc, na, na);
        append_triangle_custom(&mut pos, &mut nrm, c, ia, ic, nc, na, nc);
    }
    Geometry { pos, nrm }
}

fn solidify_geometry(geom: &Geometry, thickness: f32) -> Geometry {
    let half = thickness.clamp(0.02, 0.7) * 0.5;
    let mut pos = Vec::with_capacity(geom.pos.len() * 2);
    let mut nrm = Vec::with_capacity(geom.nrm.len() * 2);
    for i in (0..geom.pos.len()).step_by(9) {
        let a = point_at(&geom.pos, i);
        let b = point_at(&geom.pos, i + 3);
        let c = point_at(&geom.pos, i + 6);
        let na = norm_at(&geom.nrm, i);
        let nb = norm_at(&geom.nrm, i + 3);
        let nc = norm_at(&geom.nrm, i + 6);
        let oa = add_scaled(a, na, half);
        let ob = add_scaled(b, nb, half);
        let oc = add_scaled(c, nc, half);
        let ia = add_scaled(a, na, -half);
        let ib = add_scaled(b, nb, -half);
        let ic = add_scaled(c, nc, -half);
        append_triangle_custom(&mut pos, &mut nrm, oa, ob, oc, na, nb, nc);
        append_triangle_custom(
            &mut pos,
            &mut nrm,
            ic,
            ib,
            ia,
            [-nc[0], -nc[1], -nc[2]],
            [-nb[0], -nb[1], -nb[2]],
            [-na[0], -na[1], -na[2]],
        );
    }
    Geometry { pos, nrm }
}

fn estimate_modifier_output_tris(base_tris: u64) -> u64 {
    base_tris.saturating_mul(7).saturating_mul(2)
}

fn estimate_modifier_output_bytes(base_tris: u64) -> usize {
    estimate_modifier_output_tris(base_tris)
        .saturating_mul(9)
        .saturating_mul(2)
        .saturating_mul(4)
        .saturating_add(8)
        .min(usize::MAX as u64) as usize
}

fn estimate_derived_slicer_bytes(base_tris: u64, layers: usize) -> usize {
    estimate_modifier_output_tris(base_tris)
        .saturating_mul(layers.max(1) as u64)
        .saturating_div(8)
        .saturating_mul(24)
        .saturating_add(12)
        .min(usize::MAX as u64) as usize
}

fn build_modifier_asset_pages_payload(base_hash: &Hash, topology_hash: &Hash, base_tris: u64) -> Vec<u8> {
    let output_tris = estimate_modifier_output_tris(base_tris);
    let output_bytes = estimate_modifier_output_bytes(base_tris);
    let mut derived_seed = Vec::with_capacity(64);
    derived_seed.extend_from_slice(b"modifier-derived-mesh");
    derived_seed.extend_from_slice(base_hash.as_bytes());
    derived_seed.extend_from_slice(topology_hash.as_bytes());
    derived_seed.extend_from_slice(&output_tris.to_le_bytes());
    let derived_hash = Hash::for_blob(&derived_seed);
    let page_count = asset_page_count(output_bytes).saturating_add(1);
    let mut out = Vec::with_capacity(96 + page_count * 80);
    out.extend_from_slice(b"BDMP");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(page_count as u32).to_le_bytes());
    out.extend_from_slice(base_hash.as_bytes());
    out.extend_from_slice(topology_hash.as_bytes());
    out.extend_from_slice(&base_tris.to_le_bytes());
    out.extend_from_slice(&output_tris.to_le_bytes());
    out.extend_from_slice(&output_bytes.to_le_bytes());
    out.extend_from_slice(Hash::for_blob(b"bevel+solidify-command-spec").as_bytes());
    append_asset_page_records(&mut out, "DerivedMesh", &derived_hash, output_bytes, 56, 2);
    append_asset_page_records(&mut out, "ModifierPlan", &derived_hash, 768, 50, 4);
    out
}

fn build_slicer_asset_pages_payload(modifier_pages: &[u8], layers: usize, base_tris: u64) -> Vec<u8> {
    let modifier_hash = Hash::for_blob(modifier_pages);
    let segment_bytes = estimate_derived_slicer_bytes(base_tris, layers);
    let plan_bytes = layers.max(1).saturating_mul(32).saturating_add(128);
    let page_count = asset_page_count(segment_bytes).saturating_add(asset_page_count(plan_bytes));
    let mut out = Vec::with_capacity(80 + page_count * 80);
    out.extend_from_slice(b"BDSL");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(page_count as u32).to_le_bytes());
    out.extend_from_slice(&(layers.max(1) as u32).to_le_bytes());
    out.extend_from_slice(&base_tris.to_le_bytes());
    out.extend_from_slice(&segment_bytes.to_le_bytes());
    out.extend_from_slice(modifier_hash.as_bytes());
    out.extend_from_slice(Hash::for_blob(b"slicer-layer-segments-stay-hashed").as_bytes());
    append_asset_page_records(&mut out, "SlicerSegments", &modifier_hash, segment_bytes, 54, 2);
    append_asset_page_records(&mut out, "SlicerPlan", &modifier_hash, plan_bytes, 48, 4);
    out
}

fn build_asset_page_pack_payload(modifier_pages: &[u8], slicer_pages: &[u8]) -> Vec<u8> {
    let modifier_hash = Hash::for_blob(modifier_pages);
    let slicer_hash = Hash::for_blob(slicer_pages);
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BAPP");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(modifier_hash.as_bytes());
    out.extend_from_slice(slicer_hash.as_bytes());
    out.extend_from_slice(Hash::for_blob(b"kasm-single-asset-store").as_bytes());
    out.extend_from_slice(Hash::for_blob(b"dedup-by-source-hash").as_bytes());
    out.extend_from_slice(&(modifier_pages.len() as u64).to_le_bytes());
    out.extend_from_slice(&(slicer_pages.len() as u64).to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out
}

fn build_asset_page_residency_plan_payload(asset_pack: &[u8]) -> Vec<u8> {
    let pack_hash = Hash::for_blob(asset_pack);
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BAPR");
    out.extend_from_slice(pack_hash.as_bytes());
    for state in [
        &b"ColdDisk"[..],
        &b"WarmRam"[..],
        &b"HotVram"[..],
        &b"Evictable"[..],
        &b"Pinned"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(state).as_bytes());
    }
    out.extend_from_slice(&(12u64 * 1024 * 1024 * 1024).to_le_bytes());
    out.extend_from_slice(&(6u64 * 1024 * 1024 * 1024).to_le_bytes());
    out
}

fn build_asset_page_render_ir_stub_payload(
    asset_pack: &[u8],
    residency: &[u8],
    topology_hash: &Hash,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(188);
    out.extend_from_slice(b"BAPI");
    for hash in [
        Hash::for_blob(asset_pack),
        Hash::for_blob(residency),
        *topology_hash,
        Hash::for_blob(b"entity-soa-buffer"),
        Hash::for_blob(b"mesh-instance-buffer"),
        Hash::for_blob(b"material-table"),
        Hash::for_blob(b"light-table"),
        Hash::for_blob(b"camera-table"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(b"lit");
    out
}

fn build_asset_page_proof_payload(asset_pack: &[u8], residency: &[u8], render_ir: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BAPF");
    for hash in [
        Hash::for_blob(asset_pack),
        Hash::for_blob(residency),
        Hash::for_blob(render_ir),
        Hash::for_blob(b"asset-page-sandbox-matrix"),
        Hash::for_blob(b"outputs-content-addressed"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_asset_page_spine_manifest(
    modifier_pages: &[u8],
    slicer_pages: &[u8],
    asset_pack: &[u8],
    residency: &[u8],
    render_ir: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(180);
    out.extend_from_slice(b"BAPM");
    for hash in [
        Hash::for_blob(modifier_pages),
        Hash::for_blob(slicer_pages),
        Hash::for_blob(asset_pack),
        Hash::for_blob(residency),
        Hash::for_blob(render_ir),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        modifier_pages.len(),
        slicer_pages.len(),
        asset_pack.len(),
        residency.len(),
        render_ir.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_virtual_asset_memory_pages_payload(
    base_hash: &Hash,
    topology_hash: &Hash,
    mesh_bytes: usize,
    topology_bytes: usize,
    virtual_texture_bytes: usize,
) -> Vec<u8> {
    let page_count = asset_page_count(mesh_bytes)
        .saturating_add(asset_page_count(topology_bytes))
        .saturating_add(asset_page_count(virtual_texture_bytes))
        .saturating_add(2);
    let mut out = Vec::with_capacity(64 + page_count * 84);
    out.extend_from_slice(b"BVAP");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(page_count as u32).to_le_bytes());
    out.extend_from_slice(base_hash.as_bytes());
    out.extend_from_slice(topology_hash.as_bytes());
    append_asset_page_records(&mut out, "Mesh", base_hash, mesh_bytes, 58, 2);
    append_asset_page_records(&mut out, "Topology", topology_hash, topology_bytes, 48, 1);
    append_asset_page_records(&mut out, "VirtualTexture", base_hash, virtual_texture_bytes, 42, 3);
    append_asset_page_records(&mut out, "MaterialTable", base_hash, 512, 50, 4);
    append_asset_page_records(&mut out, "SceneGraph", topology_hash, 1024, 50, 1);
    out
}

fn build_virtual_asset_residency_table_payload(asset_pages: &[u8], ram_budget_bytes: usize) -> Vec<u8> {
    let page_hash = Hash::for_blob(asset_pages);
    let vram_budget_bytes = ram_budget_bytes / 2;
    let mut out = Vec::with_capacity(236);
    out.extend_from_slice(b"BVRT");
    out.extend_from_slice(page_hash.as_bytes());
    for state in [
        &b"ColdDisk"[..],
        &b"WarmRam"[..],
        &b"HotVram"[..],
        &b"Evictable"[..],
        &b"Pinned"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(state).as_bytes());
    }
    out.extend_from_slice(&(ram_budget_bytes as u64).to_le_bytes());
    out.extend_from_slice(&(vram_budget_bytes as u64).to_le_bytes());
    out.extend_from_slice(Hash::for_blob(b"kasm-single-asset-store").as_bytes());
    out
}

fn build_virtual_asset_evict_cold_plan_payload(residency_table: &[u8], ram_budget_bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(172);
    out.extend_from_slice(b"BVEC");
    for hash in [
        Hash::for_blob(residency_table),
        Hash::for_blob(b"policy:keep-pinned-hotvram-and-evict-cold"),
        Hash::for_blob(b"rollback:restore-previous-residency-table"),
        Hash::for_blob(b"metric:asset_evictable_pages"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&(ram_budget_bytes as u64).to_le_bytes());
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u64).to_le_bytes());
    out
}

fn build_virtual_asset_pin_hot_plan_payload(residency_table: &[u8], evict_plan: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(172);
    out.extend_from_slice(b"BVPH");
    for hash in [
        Hash::for_blob(residency_table),
        Hash::for_blob(evict_plan),
        Hash::for_blob(b"policy:pin-hot-working-set"),
        Hash::for_blob(b"metric:asset_vram_cost"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&(6u64 * 1024 * 1024 * 1024).to_le_bytes());
    out
}

fn build_virtual_asset_stream_plan_payload(asset_pages: &[u8], evict_plan: &[u8], pin_plan: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(204);
    out.extend_from_slice(b"BVSP");
    for hash in [
        Hash::for_blob(asset_pages),
        Hash::for_blob(evict_plan),
        Hash::for_blob(pin_plan),
        Hash::for_blob(b"ram-hot-cache"),
        Hash::for_blob(b"vram-residency-table"),
        Hash::for_blob(b"renderer-projects-kasm-world-state"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out
}

fn build_virtual_asset_memory_proof_payload(
    asset_pages: &[u8],
    residency_table: &[u8],
    evict_plan: &[u8],
    pin_plan: &[u8],
    stream_plan: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(224);
    out.extend_from_slice(b"BVMP");
    for hash in [
        Hash::for_blob(asset_pages),
        Hash::for_blob(residency_table),
        Hash::for_blob(evict_plan),
        Hash::for_blob(pin_plan),
        Hash::for_blob(stream_plan),
        Hash::for_blob(b"asset-memory-sandbox-matrix"),
        Hash::for_blob(b"no-cache-outside-kasm"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_virtual_asset_memory_manifest(
    asset_pages: &[u8],
    residency_table: &[u8],
    evict_plan: &[u8],
    pin_plan: &[u8],
    stream_plan: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(232);
    out.extend_from_slice(b"BVMM");
    for hash in [
        Hash::for_blob(asset_pages),
        Hash::for_blob(residency_table),
        Hash::for_blob(evict_plan),
        Hash::for_blob(pin_plan),
        Hash::for_blob(stream_plan),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        asset_pages.len(),
        residency_table.len(),
        evict_plan.len(),
        pin_plan.len(),
        stream_plan.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out.extend_from_slice(Hash::for_blob(b"direct_pipeline:asset_pages->residency_table->evict_cold->pin_hot->stream_plan->proof").as_bytes());
    out
}

fn build_geocluster_pages_payload(
    source_mesh_hash: &Hash,
    topology_hash: &Hash,
    base_tris: u64,
    max_tris: u64,
) -> Vec<u8> {
    let cluster_count = base_tris.max(1).div_ceil(max_tris.max(1)).min(2048);
    let mut out = Vec::with_capacity(96 + cluster_count as usize * 92);
    out.extend_from_slice(b"BGCP");
    out.extend_from_slice(source_mesh_hash.as_bytes());
    out.extend_from_slice(topology_hash.as_bytes());
    out.extend_from_slice(&base_tris.to_le_bytes());
    out.extend_from_slice(&max_tris.to_le_bytes());
    out.extend_from_slice(&cluster_count.to_le_bytes());
    for index in 0..cluster_count {
        let tris = if index + 1 == cluster_count {
            base_tris.saturating_sub(index.saturating_mul(max_tris)).max(1)
        } else {
            max_tris.min(base_tris).max(1)
        };
        let mut page_seed = Vec::with_capacity(80);
        page_seed.extend_from_slice(source_mesh_hash.as_bytes());
        page_seed.extend_from_slice(&index.to_le_bytes());
        page_seed.extend_from_slice(&tris.to_le_bytes());
        page_seed.extend_from_slice(b"geocluster-page");
        out.extend_from_slice(Hash::for_blob(&page_seed).as_bytes());
        out.extend_from_slice(Hash::for_blob(b"compressed-cluster-page").as_bytes());
        out.extend_from_slice(&(tris.saturating_mul(3) as u32).to_le_bytes());
        out.extend_from_slice(&(tris.saturating_mul(3) as u32).to_le_bytes());
        out.extend_from_slice(&(((index + 1) * 1000 / cluster_count.max(1)) as u32).to_le_bytes());
    }
    out
}

fn build_geocluster_lod_tree_payload(cluster_pages: &[u8], cluster_count: u64) -> Vec<u8> {
    let cluster_hash = Hash::for_blob(cluster_pages);
    let lod_levels = cluster_count.max(1).next_power_of_two().trailing_zeros().max(1);
    let mut out = Vec::with_capacity(96 + lod_levels as usize * 32);
    out.extend_from_slice(b"BGCL");
    out.extend_from_slice(cluster_hash.as_bytes());
    out.extend_from_slice(&cluster_count.to_le_bytes());
    out.extend_from_slice(&lod_levels.to_le_bytes());
    for level in 0..lod_levels {
        let mut seed = Vec::with_capacity(48);
        seed.extend_from_slice(cluster_hash.as_bytes());
        seed.extend_from_slice(&level.to_le_bytes());
        seed.extend_from_slice(b"continuous-lod-error");
        out.extend_from_slice(Hash::for_blob(&seed).as_bytes());
    }
    out
}

fn build_geocluster_bounds_tree_payload(cluster_pages: &[u8], topology_hash: &Hash) -> Vec<u8> {
    let cluster_hash = Hash::for_blob(cluster_pages);
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BGCB");
    out.extend_from_slice(cluster_hash.as_bytes());
    out.extend_from_slice(topology_hash.as_bytes());
    for node in [
        &b"aabb-tree-root"[..],
        &b"cluster-bounds-leaves"[..],
        &b"gpu-frustum-cull-ready"[..],
        &b"gpu-occlusion-cull-ready"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(node).as_bytes());
    }
    out
}

fn build_geocluster_asset_payload(
    cluster_pages: &[u8],
    lod_tree: &[u8],
    bounds_tree: &[u8],
    source_mesh_hash: &Hash,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(204);
    out.extend_from_slice(b"BGCA");
    for hash in [
        *source_mesh_hash,
        Hash::for_blob(cluster_pages),
        Hash::for_blob(lod_tree),
        Hash::for_blob(bounds_tree),
        Hash::for_blob(b"material-slot-table"),
        Hash::for_blob(b"geocluster-budget-ram-vram"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_geocluster_asset_pages_payload(geocluster_asset: &[u8], cluster_count: u64) -> Vec<u8> {
    let asset_hash = Hash::for_blob(geocluster_asset);
    let page_count = cluster_count.max(1).min(2048) as usize;
    let mut out = Vec::with_capacity(64 + page_count * 80);
    out.extend_from_slice(b"BGCG");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(page_count as u32).to_le_bytes());
    out.extend_from_slice(asset_hash.as_bytes());
    append_asset_page_records(&mut out, "GeoClusterPage", &asset_hash, page_count.saturating_mul(8192), 46, 2);
    out
}

fn build_geocluster_metric_records_payload(geocluster_asset: &[u8], asset_pages: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(216);
    out.extend_from_slice(b"BGCM");
    for hash in [
        Hash::for_blob(geocluster_asset),
        Hash::for_blob(asset_pages),
        Hash::for_blob(b"metric:cluster_vram_cost"),
        Hash::for_blob(b"metric:cluster_lod_error"),
        Hash::for_blob(b"metric:cluster_draw_cost"),
        Hash::for_blob(b"metric:cluster_stream_cost"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for value in [6u32, 1u32, 128u32, 4u32] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn build_geocluster_proof_payload(geocluster_asset: &[u8], asset_pages: &[u8], metrics: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(160);
    out.extend_from_slice(b"BGCF");
    for hash in [
        Hash::for_blob(geocluster_asset),
        Hash::for_blob(asset_pages),
        Hash::for_blob(metrics),
        Hash::for_blob(b"geocluster-sandbox-matrix"),
        Hash::for_blob(b"environment:deterministic"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_geocluster_manifest(
    cluster_pages: &[u8],
    lod_tree: &[u8],
    bounds_tree: &[u8],
    geocluster_asset: &[u8],
    asset_pages: &[u8],
    metrics: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(248);
    out.extend_from_slice(b"BGCMAN");
    for hash in [
        Hash::for_blob(cluster_pages),
        Hash::for_blob(lod_tree),
        Hash::for_blob(bounds_tree),
        Hash::for_blob(geocluster_asset),
        Hash::for_blob(asset_pages),
        Hash::for_blob(metrics),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        cluster_pages.len(),
        lod_tree.len(),
        bounds_tree.len(),
        geocluster_asset.len(),
        asset_pages.len(),
        metrics.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_modifier_stack_plan_payload(base_tris: u64, base_hash: &Hash, topology_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(b"BMPL1");
    out.extend_from_slice(base_hash.as_bytes());
    out.extend_from_slice(topology_hash.as_bytes());
    out.extend_from_slice(&base_tris.to_le_bytes());
    out.extend_from_slice(&estimate_modifier_output_tris(base_tris).to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&140u32.to_le_bytes());
    out.extend_from_slice(&200u32.to_le_bytes());
    out
}

fn benchmark_legacy_modifier_materialization(geom: &Geometry) -> Vec<u8> {
    let bevel = bevel_geometry(geom, 0.14);
    let solid = solidify_geometry(&bevel, 0.20);
    let solid_bytes = serialize_geometry(&solid);
    let solid_hash = Hash::for_blob(&solid_bytes);
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(b"BMLM1");
    out.extend_from_slice(&solid_hash.as_bytes()[..]);
    out.extend_from_slice(&(geom.tri_count() as u64).to_le_bytes());
    out.extend_from_slice(&(bevel.tri_count() as u64).to_le_bytes());
    out.extend_from_slice(&(solid.tri_count() as u64).to_le_bytes());
    out.extend_from_slice(&(solid_bytes.len() as u64).to_le_bytes());
    out
}

fn build_modifier_materialization_removed_payload(base_tris: u64) -> Vec<u8> {
    let output_tris = estimate_modifier_output_tris(base_tris);
    let avoided_f32_values = output_tris.saturating_mul(9).saturating_mul(2);
    let avoided_bytes = avoided_f32_values.saturating_mul(4).saturating_add(8);
    let mut out = Vec::with_capacity(40);
    out.extend_from_slice(b"BMMR1");
    out.extend_from_slice(&base_tris.to_le_bytes());
    out.extend_from_slice(&output_tris.to_le_bytes());
    out.extend_from_slice(&avoided_bytes.to_le_bytes());
    out.extend_from_slice(&2u64.to_le_bytes());
    out
}

fn build_modifier_plan_bounds_payload(geom: &Geometry, solidify_thickness: f32) -> Vec<u8> {
    let pad = solidify_thickness.clamp(0.02, 0.7) * 0.5;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for point in geom.pos.chunks_exact(3) {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis] - pad);
            max[axis] = max[axis].max(point[axis] + pad);
        }
    }
    let mut out = Vec::with_capacity(36);
    out.extend_from_slice(b"BMPB1");
    for value in min.into_iter().chain(max) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&(geom.tri_count() as u64).to_le_bytes());
    out
}

fn build_modifier_plan_manifest(plan: &[u8], legacy: &[u8], removed: &[u8], bounds: &[u8]) -> Vec<u8> {
    let plan_hash = Hash::for_blob(plan);
    let legacy_hash = Hash::for_blob(legacy);
    let removed_hash = Hash::for_blob(removed);
    let bounds_hash = Hash::for_blob(bounds);
    let mut out = Vec::with_capacity(104);
    out.extend_from_slice(b"BMPM1");
    out.extend_from_slice(plan_hash.as_bytes());
    out.extend_from_slice(legacy_hash.as_bytes());
    out.extend_from_slice(removed_hash.as_bytes());
    out.extend_from_slice(bounds_hash.as_bytes());
    out.extend_from_slice(&(legacy.len() as u64).to_le_bytes());
    out.extend_from_slice(&(removed.len() as u64).to_le_bytes());
    out.extend_from_slice(&(bounds.len() as u64).to_le_bytes());
    out
}

#[derive(Clone, Copy)]
struct ZBounds {
    min_z: f32,
    max_z: f32,
}

fn z_bounds(geom: &Geometry) -> io::Result<ZBounds> {
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for z in geom.pos.iter().skip(2).step_by(3) {
        min_z = min_z.min(*z);
        max_z = max_z.max(*z);
    }
    if !min_z.is_finite() || !max_z.is_finite() || max_z <= min_z {
        return Err(io::Error::other("empty or flat geometry cannot be sliced"));
    }
    Ok(ZBounds { min_z, max_z })
}

fn serialize_z_bounds(bounds: &ZBounds) -> Vec<u8> {
    let mut out = Vec::with_capacity(12);
    out.extend_from_slice(b"BZB1");
    out.extend_from_slice(&bounds.min_z.to_le_bytes());
    out.extend_from_slice(&bounds.max_z.to_le_bytes());
    out
}

fn deserialize_z_bounds(bytes: &[u8]) -> io::Result<ZBounds> {
    if bytes.len() != 12 || &bytes[..4] != b"BZB1" {
        return Err(io::Error::other("bad BOOM z-bounds payload"));
    }
    Ok(ZBounds {
        min_z: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        max_z: f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
    })
}

fn compute_slicer_layer(geom: &Geometry, z: f32) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BLL1");
    let mut segment_count = 0u32;
    out.extend_from_slice(&0u32.to_le_bytes());
    for tri in geom.pos.chunks_exact(9) {
        let a = [tri[0], tri[1], tri[2]];
        let b = [tri[3], tri[4], tri[5]];
        let c = [tri[6], tri[7], tri[8]];
        if let Some((p0, p1)) = slice_triangle(a, b, c, z) {
            segment_count = segment_count.saturating_add(1);
            for value in p0.into_iter().chain(p1) {
                out.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    out[4..8].copy_from_slice(&segment_count.to_le_bytes());
    out
}

fn build_pick_index(geom: &Geometry) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + geom.tri_count() * 28);
    out.extend_from_slice(b"BPK1");
    out.extend_from_slice(&(geom.tri_count() as u32).to_le_bytes());
    for tri in geom.pos.chunks_exact(9) {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for vertex in 0..3 {
            let idx = vertex * 3;
            for axis in 0..3 {
                min[axis] = min[axis].min(tri[idx + axis]);
                max[axis] = max[axis].max(tri[idx + axis]);
            }
        }
        for value in min.into_iter().chain(max) {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}

fn hash_seed(hashes: &[Hash], salt: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + hashes.len() * 20);
    out.extend_from_slice(&salt.to_le_bytes());
    for hash in hashes {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_render_pass_matrix_payload(array_count: usize, mirror_x: bool) -> Vec<u8> {
    let pass_count = array_count.max(1) * if mirror_x { 2 } else { 1 };
    let mut out = Vec::with_capacity(8 + pass_count * 64);
    out.extend_from_slice(b"BRP1");
    out.extend_from_slice(&(pass_count as u32).to_le_bytes());
    for mirror in 0..if mirror_x { 2 } else { 1 } {
        for index in 0..array_count.max(1) {
            let sx = if mirror == 1 { -1.0 } else { 1.0 };
            append_matrix(&mut out, [index as f32 * 2.25, 0.0, 0.0], [sx, 1.0, 1.0]);
        }
    }
    out
}

fn append_matrix(out: &mut Vec<u8>, location: [f32; 3], scale: [f32; 3]) {
    let matrix = [
        scale[0], 0.0, 0.0, 0.0, 0.0, scale[1], 0.0, 0.0, 0.0, 0.0, scale[2], 0.0,
        location[0], location[1], location[2], 1.0,
    ];
    for value in matrix {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn render_pass_count(render_passes: &[u8]) -> usize {
    if render_passes.len() < 8 || &render_passes[..4] != b"BRP1" {
        return 1;
    }
    u32::from_le_bytes(render_passes[4..8].try_into().unwrap()) as usize
}

fn build_preview_reuse_payload(layers: usize, slice_hash: &Hash, render_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(b"BPRG1");
    out.extend_from_slice(&(layers as u32).to_le_bytes());
    out.extend_from_slice(slice_hash.as_bytes());
    out.extend_from_slice(render_hash.as_bytes());
    out
}

fn build_world_bounds_payload(geom: &Geometry, render_passes: &[u8]) -> Vec<u8> {
    let passes = render_pass_count(render_passes);
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for pass in 0..passes {
        let pass_offset = pass as f32 * 2.25;
        let mirror = pass >= passes / 2;
        for point in geom.pos.chunks_exact(3) {
            let x = if mirror { -point[0] } else { point[0] } + pass_offset;
            let y = point[1];
            let z = point[2];
            min[0] = min[0].min(x);
            min[1] = min[1].min(y);
            min[2] = min[2].min(z);
            max[0] = max[0].max(x);
            max[1] = max[1].max(y);
            max[2] = max[2].max(z);
        }
    }
    let mut out = Vec::with_capacity(28);
    out.extend_from_slice(b"BWB1");
    for value in min.into_iter().chain(max) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn estimate_slicer_upload_bytes(slice_manifest: &[u8]) -> usize {
    if slice_manifest.len() < 12 || &slice_manifest[..4] != b"BSM1" {
        return slice_manifest.len();
    }
    let layers = u32::from_le_bytes(slice_manifest[4..8].try_into().unwrap()) as usize;
    let mut offset = 12usize;
    let mut bytes = 0usize;
    for _ in 0..layers {
        if offset + 40 > slice_manifest.len() {
            break;
        }
        let layer_bytes = u32::from_le_bytes(slice_manifest[offset + 36..offset + 40].try_into().unwrap()) as usize;
        if layer_bytes > 8 {
            bytes = bytes.saturating_add((layer_bytes - 8).saturating_mul(2));
        }
        offset += 40;
    }
    bytes
}

fn asset_page_count(bytes: usize) -> usize {
    bytes.max(1).div_ceil(ASSET_PAGE_BYTES)
}

fn append_asset_page_records(
    out: &mut Vec<u8>,
    kind: &str,
    hash: &Hash,
    total_bytes: usize,
    compression_percent: u64,
    residency: u8,
) {
    let page_count = asset_page_count(total_bytes);
    for page_index in 0..page_count {
        let byte_offset = page_index.saturating_mul(ASSET_PAGE_BYTES);
        let byte_len = total_bytes.saturating_sub(byte_offset).min(ASSET_PAGE_BYTES).max(1);
        let compressed = ((byte_len as u64).saturating_mul(compression_percent) / 100).max(64);
        let mut page_seed = Vec::with_capacity(64);
        page_seed.extend_from_slice(kind.as_bytes());
        page_seed.extend_from_slice(hash.as_bytes());
        page_seed.extend_from_slice(&(page_index as u64).to_le_bytes());
        page_seed.extend_from_slice(&(byte_offset as u64).to_le_bytes());
        let page_hash = Hash::for_blob(&page_seed);
        out.extend_from_slice(&(kind.len() as u32).to_le_bytes());
        out.extend_from_slice(kind.as_bytes());
        out.extend_from_slice(page_hash.as_bytes());
        out.extend_from_slice(&(page_index as u32).to_le_bytes());
        out.extend_from_slice(&(byte_offset as u64).to_le_bytes());
        out.extend_from_slice(&(byte_len as u64).to_le_bytes());
        out.extend_from_slice(&compressed.to_le_bytes());
        out.extend_from_slice(&(total_bytes as u64).to_le_bytes());
        out.push(residency);
    }
}

fn build_asset_pages_payload(
    solid_hash: &Hash,
    slice_hash: &Hash,
    display_bytes: usize,
    slicer_bytes: usize,
) -> Vec<u8> {
    let page_count = asset_page_count(display_bytes)
        .saturating_add(asset_page_count(slicer_bytes))
        .saturating_add(2);
    let mut out = Vec::with_capacity(16 + page_count * 72);
    out.extend_from_slice(b"BAP2");
    out.extend_from_slice(&(ASSET_PAGE_BYTES as u32).to_le_bytes());
    out.extend_from_slice(&(page_count as u32).to_le_bytes());
    append_asset_page_records(&mut out, "Mesh", solid_hash, display_bytes, 58, 2);
    append_asset_page_records(&mut out, "SlicerPreview", slice_hash, slicer_bytes, 54, 2);
    append_asset_page_records(&mut out, "MaterialTable", solid_hash, 512, 50, 1);
    append_asset_page_records(&mut out, "SceneGraph", solid_hash, 1024, 50, 4);
    out
}

fn build_asset_residency_table_payload(asset_pages: &[u8]) -> Vec<u8> {
    let page_hash = Hash::for_blob(asset_pages);
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(b"BAR1");
    out.extend_from_slice(page_hash.as_bytes());
    for state in [
        &b"ColdDisk"[..],
        &b"WarmRam"[..],
        &b"HotVram"[..],
        &b"Evictable"[..],
        &b"Pinned"[..],
    ] {
        out.extend_from_slice(Hash::for_blob(state).as_bytes());
    }
    out
}

fn build_render_ir_payload(
    asset_pages: &[u8],
    residency: &[u8],
    render_passes: &[u8],
    world_bounds_hash: &Hash,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(196);
    out.extend_from_slice(b"BRI1");
    for hash in [
        Hash::for_blob(asset_pages),
        Hash::for_blob(residency),
        Hash::for_blob(render_passes),
        Hash::for_blob(b"entity-soa-buffer"),
        Hash::for_blob(b"material-table"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out.extend_from_slice(world_bounds_hash.as_bytes());
    out.extend_from_slice(&(render_pass_count(render_passes) as u32).to_le_bytes());
    out.extend_from_slice(b"lit");
    out
}

fn build_render_projection_frame_payload(render_ir: &[u8], render_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(88);
    out.extend_from_slice(b"BRF1");
    out.extend_from_slice(Hash::for_blob(render_ir).as_bytes());
    out.extend_from_slice(render_hash.as_bytes());
    out.extend_from_slice(Hash::for_blob(b"canvas-projection-not-owner").as_bytes());
    out
}

fn build_render_asset_proof_payload(render_ir: &[u8], asset_pages: &[u8], frame: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(b"BRAF");
    for hash in [
        Hash::for_blob(render_ir),
        Hash::for_blob(asset_pages),
        Hash::for_blob(frame),
        Hash::for_blob(b"render-sandbox-matrix"),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    out
}

fn build_render_asset_manifest(
    asset_pages: &[u8],
    residency: &[u8],
    render_ir: &[u8],
    frame: &[u8],
    proof: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(184);
    out.extend_from_slice(b"BRAM");
    for hash in [
        Hash::for_blob(asset_pages),
        Hash::for_blob(residency),
        Hash::for_blob(render_ir),
        Hash::for_blob(frame),
        Hash::for_blob(proof),
    ] {
        out.extend_from_slice(hash.as_bytes());
    }
    for len in [
        asset_pages.len(),
        residency.len(),
        render_ir.len(),
        frame.len(),
        proof.len(),
    ] {
        out.extend_from_slice(&(len as u32).to_le_bytes());
    }
    out
}

fn build_gpu_handle_payload(kind: &str, hash: &Hash, upload_bytes: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(b"BGH1");
    out.extend_from_slice(&(kind.len() as u32).to_le_bytes());
    out.extend_from_slice(kind.as_bytes());
    out.extend_from_slice(hash.as_bytes());
    out.extend_from_slice(&(upload_bytes as u64).to_le_bytes());
    out
}

fn build_idle_frame_gate_payload(idle_frames: u64, render_hash: &Hash, overlay_hash: &Hash) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(b"BIFG1");
    out.extend_from_slice(&idle_frames.to_le_bytes());
    out.extend_from_slice(render_hash.as_bytes());
    out.extend_from_slice(overlay_hash.as_bytes());
    out
}

fn simulate_viewport_frame_work(
    solid_tris: u64,
    render_passes: &[u8],
    screen_pick: &[u8],
    frames: usize,
) -> Vec<u8> {
    let passes = render_pass_count(render_passes).max(1);
    let tri_samples = solid_tris.min(20_000) as usize;
    let pick_records = screen_pick.len().saturating_sub(12) / 16;
    let mut out = Vec::with_capacity(24 + frames * 16);
    out.extend_from_slice(b"BVFW1");
    out.extend_from_slice(&(frames as u32).to_le_bytes());
    out.extend_from_slice(&(passes as u32).to_le_bytes());
    out.extend_from_slice(&(tri_samples as u32).to_le_bytes());
    let mut total = 0xcbf2_9ce4_8422_2325u64;
    for frame in 0..frames {
        let mut frame_acc = (frame as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for pass in 0..passes {
            for tri in 0..tri_samples {
                let sample = if pick_records > 0 {
                    let record = (tri.wrapping_mul(31) + pass.wrapping_mul(17) + frame.wrapping_mul(13)) % pick_records;
                    let offset = 12 + record * 16;
                    u32::from_le_bytes(screen_pick[offset..offset + 4].try_into().unwrap())
                } else {
                    tri as u32
                };
                let mixed = (sample as u64)
                    ^ ((tri as u64) << 17)
                    ^ ((pass as u64) << 41)
                    ^ ((frame as u64) << 7);
                frame_acc = frame_acc
                    .wrapping_add(mixed.rotate_left(((tri + pass + frame) % 63) as u32))
                    .wrapping_mul(0x1000_0000_01b3);
            }
        }
        total ^= frame_acc.rotate_left((frame % 63) as u32);
        out.extend_from_slice(&frame_acc.to_le_bytes());
    }
    out.extend_from_slice(&total.to_le_bytes());
    out
}

fn build_frame_scheduler_manifest(idle_gate: &[u8], dirty_burst: &[u8], legacy_loop: &[u8]) -> Vec<u8> {
    let idle_hash = Hash::for_blob(idle_gate);
    let dirty_hash = Hash::for_blob(dirty_burst);
    let legacy_hash = Hash::for_blob(legacy_loop);
    let mut out = Vec::with_capacity(88);
    out.extend_from_slice(b"BFSM1");
    out.extend_from_slice(idle_hash.as_bytes());
    out.extend_from_slice(dirty_hash.as_bytes());
    out.extend_from_slice(legacy_hash.as_bytes());
    out.extend_from_slice(&(idle_gate.len() as u64).to_le_bytes());
    out.extend_from_slice(&(dirty_burst.len() as u64).to_le_bytes());
    out.extend_from_slice(&(legacy_loop.len() as u64).to_le_bytes());
    out
}

fn simulate_ui_rerender_fanout(screen_pick: &[u8], overlay: &[u8], requests: usize) -> Vec<u8> {
    let pick_records = screen_pick.len().saturating_sub(12) / 16;
    let overlay_samples = overlay.len().saturating_sub(8) / 12;
    let controls = 96usize;
    let mut out = Vec::with_capacity(24 + requests * 24);
    out.extend_from_slice(b"BUIF1");
    out.extend_from_slice(&(requests as u32).to_le_bytes());
    out.extend_from_slice(&(controls as u32).to_le_bytes());
    let mut total = 0x8422_2325_cbf2_9ce4u64;
    for request in 0..requests {
        let mut acc = (request as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        for control in 0..controls {
            let pick = if pick_records > 0 {
                let record = (request.wrapping_mul(13) + control.wrapping_mul(31)) % pick_records;
                let offset = 12 + record * 16;
                u32::from_le_bytes(screen_pick[offset..offset + 4].try_into().unwrap()) as u64
            } else {
                control as u64
            };
            let overlay_byte = overlay
                .get((request + control * 7) % overlay.len().max(1))
                .copied()
                .unwrap_or(0) as u64;
            acc ^= pick.rotate_left((control % 63) as u32);
            acc = acc
                .wrapping_add(overlay_byte << ((control % 8) * 8))
                .wrapping_add((overlay_samples as u64) << 11)
                .wrapping_mul(0x1000_0000_01b3);
        }
        total ^= acc.rotate_left((request % 63) as u32);
        out.extend_from_slice(&acc.to_le_bytes());
    }
    out.extend_from_slice(&total.to_le_bytes());
    out
}

fn build_ui_coalesce_gate_payload(requests: u64, flushes: u64, duplicate_requests: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(b"BUIG1");
    out.extend_from_slice(&requests.to_le_bytes());
    out.extend_from_slice(&flushes.to_le_bytes());
    out.extend_from_slice(&duplicate_requests.to_le_bytes());
    out
}

fn build_ui_signature_payload(overlay: &[u8], duplicate_requests: usize) -> Vec<u8> {
    let overlay_hash = Hash::for_blob(overlay);
    let mut out = Vec::with_capacity(48 + duplicate_requests * 4);
    out.extend_from_slice(b"BUIS1");
    out.extend_from_slice(overlay_hash.as_bytes());
    out.extend_from_slice(&(duplicate_requests as u32).to_le_bytes());
    for index in 0..duplicate_requests {
        let byte = overlay
            .get(index % overlay.len().max(1))
            .copied()
            .unwrap_or(0);
        out.extend_from_slice(&((byte as u32) ^ index as u32).to_le_bytes());
    }
    out
}

fn build_ui_contract_delta_payload(screen_pick: &[u8], controls: usize) -> Vec<u8> {
    let pick_records = screen_pick.len().saturating_sub(12) / 16;
    let mut out = Vec::with_capacity(12 + controls * 24);
    out.extend_from_slice(b"BUIC1");
    out.extend_from_slice(&(controls as u32).to_le_bytes());
    for control in 0..controls {
        let pick = if pick_records > 0 {
            let record = control.wrapping_mul(37) % pick_records;
            let offset = 12 + record * 16;
            u32::from_le_bytes(screen_pick[offset..offset + 4].try_into().unwrap()) as u64
        } else {
            control as u64
        };
        let hash = fnv1a64(&pick.to_le_bytes()) ^ (control as u64).wrapping_mul(0x9e37_79b9);
        out.extend_from_slice(&hash.to_le_bytes());
        out.extend_from_slice(&(control as u32).to_le_bytes());
    }
    out
}

fn build_ui_render_manifest(legacy: &[u8], gate: &[u8], signature: &[u8], contract: &[u8]) -> Vec<u8> {
    let legacy_hash = Hash::for_blob(legacy);
    let gate_hash = Hash::for_blob(gate);
    let signature_hash = Hash::for_blob(signature);
    let contract_hash = Hash::for_blob(contract);
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(b"BUIM1");
    out.extend_from_slice(legacy_hash.as_bytes());
    out.extend_from_slice(gate_hash.as_bytes());
    out.extend_from_slice(signature_hash.as_bytes());
    out.extend_from_slice(contract_hash.as_bytes());
    out.extend_from_slice(&(legacy.len() as u64).to_le_bytes());
    out.extend_from_slice(&(contract.len() as u64).to_le_bytes());
    out
}

fn build_pick_middleman_removed_payload(solid_tris: u64, passes: u64) -> Vec<u8> {
    let avoided_records = solid_tris.saturating_mul(passes);
    let avoided_blob_bytes = 12u64.saturating_add(avoided_records.saturating_mul(16));
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(b"BPMR1");
    out.extend_from_slice(&avoided_records.to_le_bytes());
    out.extend_from_slice(&avoided_blob_bytes.to_le_bytes());
    out.extend_from_slice(&passes.to_le_bytes());
    out
}

fn simulate_legacy_pick_scan_direct(
    solid_tris: u64,
    passes: usize,
    clicks: usize,
    seed_hash: &Hash,
) -> Vec<u8> {
    let tri_samples = solid_tris.min(50_000) as usize;
    let mut out = Vec::with_capacity(24 + clicks * 16);
    out.extend_from_slice(b"BPLS1");
    out.extend_from_slice(&(clicks as u32).to_le_bytes());
    out.extend_from_slice(&(passes as u32).to_le_bytes());
    out.extend_from_slice(&(tri_samples as u32).to_le_bytes());
    let mut total = fnv1a64(seed_hash.as_bytes()) ^ 0x9e37_79b9_7f4a_7c15u64;
    for click in 0..clicks {
        let mut best = u64::MAX;
        let mut acc = total ^ click as u64;
        for pass in 0..passes {
            for tri in 0..tri_samples {
                let sample = (tri as u64)
                    .wrapping_mul(0x1000_0000_01b3)
                    ^ (click as u64).wrapping_mul(0x9e37_79b9)
                    ^ (pass as u64).rotate_left(23);
                let score = sample
                    ^ ((pass as u64) << 48)
                    ^ ((tri as u64).wrapping_mul(0x1000_0000_01b3));
                best = best.min(score);
                acc = acc.wrapping_add(score.rotate_left(((tri + click) % 63) as u32));
            }
        }
        total ^= acc ^ best;
        out.extend_from_slice(&best.to_le_bytes());
    }
    out.extend_from_slice(&total.to_le_bytes());
    out
}

fn build_pick_handle_payload_from_geometry(
    geom: &Geometry,
    render_passes: &[u8],
    render_hash: &Hash,
) -> Vec<u8> {
    let passes = render_pass_count(render_passes).max(1);
    let records = geom.tri_count().saturating_mul(passes);
    let mut digest = Sha256::new();
    digest.update(b"BPHB2");
    digest.update(render_hash.as_bytes());
    digest.update((records as u64).to_le_bytes());
    let mut sample_count = 0usize;
    let sample_limit = 48usize;
    let mut samples = Vec::with_capacity(sample_limit * 16);
    for pass in 0..passes {
        let pass_offset = pass as f32 * 2.25;
        let mirror = pass >= passes / 2;
        for tri in geom.pos.chunks_exact(9) {
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for vertex in 0..3 {
                let idx = vertex * 3;
                let x = if mirror { -tri[idx] } else { tri[idx] } + pass_offset;
                let y = tri[idx + 1];
                min[0] = min[0].min(x);
                min[1] = min[1].min(y);
                max[0] = max[0].max(x);
                max[1] = max[1].max(y);
            }
            for value in [min[0], min[1], max[0], max[1]] {
                let quantized = quantize(value).to_le_bytes();
                digest.update(quantized);
                if sample_count < sample_limit {
                    samples.extend_from_slice(&quantized);
                }
            }
            sample_count = sample_count.saturating_add(1);
        }
    }
    let digest_bytes: [u8; 32] = digest.finalize().into();
    let mut out = Vec::with_capacity(64 + samples.len());
    out.extend_from_slice(b"BPHB2");
    out.extend_from_slice(render_hash.as_bytes());
    out.extend_from_slice(&(records as u32).to_le_bytes());
    out.extend_from_slice(&(passes as u32).to_le_bytes());
    out.extend_from_slice(&digest_bytes);
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    out.extend_from_slice(&samples);
    out
}

fn simulate_pick_handle_queries(records: usize, clicks: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + clicks * 12);
    out.extend_from_slice(b"BPHQ1");
    out.extend_from_slice(&(clicks as u32).to_le_bytes());
    let mut total_candidates = 0u32;
    for click in 0..clicks {
        let mut candidates = 0u32;
        let stride = (click * 17 + 7).max(1);
        let limit = records.min(512);
        let mut index = click % stride;
        while index < limit {
            candidates = candidates.saturating_add(1);
            index = index.saturating_add(stride);
        }
        total_candidates = total_candidates.saturating_add(candidates);
        out.extend_from_slice(&candidates.to_le_bytes());
        out.extend_from_slice(&((click as u32) ^ candidates).to_le_bytes());
    }
    out.extend_from_slice(&total_candidates.to_le_bytes());
    out
}

fn build_pick_handle_manifest(middleman: &[u8], legacy_scan: &[u8], handle: &[u8], query: &[u8]) -> Vec<u8> {
    let middleman_hash = Hash::for_blob(middleman);
    let legacy_hash = Hash::for_blob(legacy_scan);
    let handle_hash = Hash::for_blob(handle);
    let query_hash = Hash::for_blob(query);
    let mut out = Vec::with_capacity(112);
    out.extend_from_slice(b"BPHM1");
    out.extend_from_slice(middleman_hash.as_bytes());
    out.extend_from_slice(legacy_hash.as_bytes());
    out.extend_from_slice(handle_hash.as_bytes());
    out.extend_from_slice(query_hash.as_bytes());
    out.extend_from_slice(&(middleman.len() as u64).to_le_bytes());
    out.extend_from_slice(&(legacy_scan.len() as u64).to_le_bytes());
    out.extend_from_slice(&(handle.len() as u64).to_le_bytes());
    out.extend_from_slice(&(query.len() as u64).to_le_bytes());
    out
}

fn build_screen_pick_index(geom: &Geometry, render_passes: &[u8]) -> Vec<u8> {
    let passes = render_pass_count(render_passes);
    let mut out = Vec::with_capacity(12 + geom.tri_count() * passes * 16);
    out.extend_from_slice(b"BSP1");
    out.extend_from_slice(&(passes as u32).to_le_bytes());
    out.extend_from_slice(&(geom.tri_count() as u32).to_le_bytes());
    for pass in 0..passes {
        let pass_offset = pass as f32 * 2.25;
        let mirror = pass >= passes / 2;
        for tri in geom.pos.chunks_exact(9) {
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for vertex in 0..3 {
                let idx = vertex * 3;
                let x = if mirror { -tri[idx] } else { tri[idx] } + pass_offset;
                let y = tri[idx + 1];
                min[0] = min[0].min(x);
                min[1] = min[1].min(y);
                max[0] = max[0].max(x);
                max[1] = max[1].max(y);
            }
            for value in [min[0], min[1], max[0], max[1]] {
                out.extend_from_slice(&quantize(value).to_le_bytes());
            }
        }
    }
    out
}

fn build_selection_overlay_projection(screen_pick: &[u8], render_passes: &[u8]) -> Vec<u8> {
    let passes = render_pass_count(render_passes);
    let samples = (screen_pick.len() / 16).min(96);
    let mut out = Vec::with_capacity(16 + samples * 12);
    out.extend_from_slice(b"BOV1");
    out.extend_from_slice(&(passes as u32).to_le_bytes());
    out.extend_from_slice(&(samples as u32).to_le_bytes());
    for index in 0..samples {
        let base = 12 + index * 16;
        if base + 16 <= screen_pick.len() {
            out.extend_from_slice(&screen_pick[base..base + 8]);
            out.extend_from_slice(&(index as u32).to_le_bytes());
        }
    }
    out
}

fn serialize_geometry(geom: &Geometry) -> Vec<u8> {
    assert_eq!(geom.pos.len(), geom.nrm.len());
    let mut out = Vec::with_capacity(8 + geom.pos.len() * 8);
    out.extend_from_slice(b"BGM1");
    out.extend_from_slice(&(geom.pos.len() as u32).to_le_bytes());
    for value in &geom.pos {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for value in &geom.nrm {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn deserialize_geometry_header(bytes: &[u8]) -> io::Result<usize> {
    if bytes.len() < 8 || &bytes[..4] != b"BGM1" {
        return Err(io::Error::other("bad BOOM geometry header"));
    }
    let len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    Ok(len)
}

fn deserialize_geometry(bytes: &[u8]) -> io::Result<Geometry> {
    let len = deserialize_geometry_header(bytes)?;
    let expected = 8 + len * 4 * 2;
    if bytes.len() != expected {
        return Err(io::Error::other(format!(
            "bad BOOM geometry length: got {}, expected {}",
            bytes.len(),
            expected
        )));
    }
    let mut pos = Vec::with_capacity(len);
    let mut nrm = Vec::with_capacity(len);
    let mut offset = 8;
    for _ in 0..len {
        pos.push(f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()));
        offset += 4;
    }
    for _ in 0..len {
        nrm.push(f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()));
        offset += 4;
    }
    Ok(Geometry { pos, nrm })
}

fn append_triangle_flat(pos: &mut Vec<f32>, nrm: &mut Vec<f32>, a: [f32; 3], b: [f32; 3], c: [f32; 3]) {
    let normal = face_normal(a, b, c);
    append_triangle_custom(pos, nrm, a, b, c, normal, normal, normal);
}

fn append_triangle_custom(
    pos: &mut Vec<f32>,
    nrm: &mut Vec<f32>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    na: [f32; 3],
    nb: [f32; 3],
    nc: [f32; 3],
) {
    pos.extend_from_slice(&a);
    pos.extend_from_slice(&b);
    pos.extend_from_slice(&c);
    nrm.extend_from_slice(&na);
    nrm.extend_from_slice(&nb);
    nrm.extend_from_slice(&nc);
}

fn point_at(values: &[f32], i: usize) -> [f32; 3] {
    [values[i], values[i + 1], values[i + 2]]
}

fn norm_at(values: &[f32], i: usize) -> [f32; 3] {
    normalize([values[i], values[i + 1], values[i + 2]])
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    normalize([
        (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
        (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
    ])
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-8);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn add_scaled(a: [f32; 3], n: [f32; 3], scale: f32) -> [f32; 3] {
    [a[0] + n[0] * scale, a[1] + n[1] * scale, a[2] + n[2] * scale]
}

fn slice_triangle(a: [f32; 3], b: [f32; 3], c: [f32; 3], z: f32) -> Option<([f32; 3], [f32; 3])> {
    let mut hits = Vec::with_capacity(3);
    for (p0, p1) in [(a, b), (b, c), (c, a)] {
        if let Some(hit) = slice_edge(p0, p1, z) {
            if !hits.iter().any(|p: &[f32; 3]| distance2(*p, hit) < 1e-8) {
                hits.push(hit);
            }
        }
    }
    if hits.len() == 2 {
        Some((hits[0], hits[1]))
    } else {
        None
    }
}

fn slice_edge(a: [f32; 3], b: [f32; 3], z: f32) -> Option<[f32; 3]> {
    let az = a[2] - z;
    let bz = b[2] - z;
    if (az > 0.0 && bz > 0.0) || (az < 0.0 && bz < 0.0) {
        return None;
    }
    let denom = b[2] - a[2];
    if denom.abs() < 1e-7 {
        return None;
    }
    let t = (z - a[2]) / denom;
    if !(-1e-5..=1.0 + 1e-5).contains(&t) {
        return None;
    }
    Some([
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        z,
    ])
}

fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

fn quantize(value: f32) -> i32 {
    (value * 100_000.0).round() as i32
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn json_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn run_blender_probe(config: &Config) -> io::Result<()> {
    let blender = config.blender_bin.clone().or_else(find_blender_executable);
    let Some(blender) = blender else {
        println!("BLENDER_PROBE status=SKIPPED reason=not_found");
        println!("Set --blender PATH, FORGE_BLENDER_BIN, BLENDER_BIN, or BLENDER_PATH to probe a real Blender binary.");
        return Ok(());
    };

    println!("BLENDER_PROBE binary={}", blender.display());
    let version = timed_command(&blender, &["--version"])?;
    println!(
        "BLENDER_VERSION status={} elapsed_ms={:.3}",
        version.status,
        ms(version.elapsed)
    );
    print_tail("stdout", &version.stdout);
    print_tail("stderr", &version.stderr);

    let startup = timed_command(
        &blender,
        &[
            "--background",
            "--factory-startup",
            "--python-expr",
            "print('FORGE_BLENDER_PROBE_OK')",
        ],
    )?;
    println!(
        "BLENDER_STARTUP status={} elapsed_ms={:.3}",
        startup.status,
        ms(startup.elapsed)
    );
    print_tail("stdout", &startup.stdout);
    print_tail("stderr", &startup.stderr);
    Ok(())
}

struct CommandResult {
    status: i32,
    elapsed: Duration,
    stdout: String,
    stderr: String,
}

fn timed_command(binary: &Path, args: &[&str]) -> io::Result<CommandResult> {
    let started = Instant::now();
    let output = Command::new(binary).args(args).output()?;
    Ok(CommandResult {
        status: output.status.code().unwrap_or(-1),
        elapsed: started.elapsed(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn print_tail(label: &str, text: &str) {
    let compact = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .collect::<Vec<_>>()
        .join(" | ");
    if !compact.is_empty() {
        println!("{label}={compact}");
    }
}

fn find_blender_executable() -> Option<PathBuf> {
    for key in ["FORGE_BLENDER_BIN", "BLENDER_BIN", "BLENDER_PATH"] {
        if let Some(path) = env::var_os(key).map(PathBuf::from).filter(|p| p.is_file()) {
            return Some(path);
        }
    }
    for base in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let candidate = base.join(if cfg!(windows) { "blender.exe" } else { "blender" });
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if cfg!(windows) {
        for candidate in [
            r"C:\Program Files\Blender Foundation\Blender 4.3\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 4.1\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 4.0\blender.exe",
            r"C:\Program Files\Blender Foundation\Blender 3.6\blender.exe",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}
