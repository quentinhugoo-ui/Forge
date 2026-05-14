//! Thin CLI shim for MonsterNode lab operations.
//!
//! All lab behavior lives in `src/monster/lab.rs` as `MonsterNode` capabilities.
//! This example is kept only for the historical Cargo command surface.

use scan::{MonsterNode, LAB_DEFAULT_ITERATIONS as DEFAULT_ITERATIONS, LAB_LOG_PATH};

fn prune_lab_log(keep: usize) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, BufWriter, Write};
    let path = LAB_LOG_PATH;
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            println!("lab log not found: {path}");
            return Ok(());
        }
    };
    let bytes_before = metadata.len();
    println!("pruning {path} ({:.1} MB)...", bytes_before as f64 / 1_048_576.0);

    // Tail-N en streaming (pas tout charger en RAM) :
    // 1. Compter les lignes
    let f = std::fs::File::open(path)?;
    let total_lines = BufReader::new(f).lines().count();
    if total_lines <= keep {
        println!("already ≤ {keep} lines ({total_lines}), nothing to prune");
        return Ok(());
    }
    let skip = total_lines - keep;

    // 2. Re-lire en gardant seulement les `keep` dernières lignes
    let f = std::fs::File::open(path)?;
    let tmp_path = format!("{path}.prune-tmp");
    let out = std::fs::File::create(&tmp_path)?;
    let mut writer = BufWriter::new(out);
    for (i, line) in BufReader::new(f).lines().enumerate() {
        if i < skip {
            continue;
        }
        let line = line?;
        writeln!(writer, "{}", line)?;
    }
    writer.flush()?;
    drop(writer);

    // 3. Atomic rename
    std::fs::rename(&tmp_path, path)?;
    let bytes_after = std::fs::metadata(path)?.len();
    println!(
        "pruned {} → {} lines, {:.1} MB → {:.1} MB ({:.1}% saved)",
        total_lines,
        keep,
        bytes_before as f64 / 1_048_576.0,
        bytes_after as f64 / 1_048_576.0,
        100.0 * (1.0 - bytes_after as f64 / bytes_before as f64)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scan::find_parasites;
    use scan::kasm::{Node, Program, Target, Ty};

    #[test]
    fn parasites_finds_dead_node() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(42),
            Node::add(0, 0),
            Node::output(2, Ty::I64),
        ];
        let p = Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap();
        let r = find_parasites(&p);
        assert_eq!(r.dead, vec![1]);
    }

    #[test]
    fn parasites_finds_duplicate() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 1),
            Node::mul(0, 1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        let p = Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap();
        let r = find_parasites(&p);
        assert!(r.duplicate_clusters.iter().any(|c| c.contains(&2) && c.contains(&3)));
    }

    #[test]
    fn parasites_clean_program() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(9),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        let p = Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap();
        let r = find_parasites(&p);
        assert_eq!(r.parasite_count(), 0);
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("run");
    match mode {
        "analyze" => {
            let limit = args.get(2).and_then(|s| s.parse::<usize>().ok());
            MonsterNode::analyze_lab_log(limit)
        }
        "parasites" => {
            let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
            MonsterNode::parasite_lab(n)
        }
        "self_improve" | "self-improve" => {
            let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(DEFAULT_ITERATIONS);
            MonsterNode::self_improve_lab(n)
        }
        "ephemeral" => {
            let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(64);
            MonsterNode::ephemeral_lab(n)
        }
        "ephemeral_ram" | "ephemeral-ram" => {
            let n = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(48);
            MonsterNode::ephemeral_ram_lab(n)
        }
        "dna_motif" | "dna-motif" => {
            let path = args.get(2).map(|s| s.as_str()).unwrap_or(".\\data\\dna_cohort.csv");
            let motif = args.get(3).map(|s| s.as_str()).unwrap_or("GATACCA");
            let mm = args.get(4).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
            MonsterNode::dna_motif_lab(path, motif, mm)
        }
        "audit_tier1" | "audit-tier1" => MonsterNode::audit_tier1_lab(),
        "audit" if args.get(2).map(|s| s.as_str()) == Some("tier1") => {
            MonsterNode::audit_tier1_lab()
        }
        "dendritic_probe" | "dendritic" => MonsterNode::dendritic_probe(),
        "validate_features" | "validate-features" | "validate" => {
            MonsterNode::validate_features()
        }
        "prune" => {
            // Garde les N dernières entrées (default 100k). Le log
            // append-only croît à plusieurs centaines de MB en R&D
            // intensive ; cette commande le truncate proprement.
            let keep = args.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(100_000);
            prune_lab_log(keep)
        }
        "run" | _ => {
            let n = args.get(1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(DEFAULT_ITERATIONS);
            MonsterNode::run_lab_batch(n)
        }
    }
}
