use std::path::PathBuf;

use crate::kasm::{Node, Program, Target, Ty};
use crate::{MemoryGovernor, Store, SwarmKnowledgeFrame};

use super::{MonsterNode, MonsterSource};

fn fresh_path(tag: &str) -> PathBuf {
    crate::fresh_tmp_path("scan-monster", tag)
}

fn program() -> Program {
    Program::new(
        Target::Cpu,
        1,
        1,
        6,
        vec![
            Node::input(0),
            Node::const_i64(9),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap()
}

fn qpu_program() -> Program {
    Program::new(
        Target::Qpu,
        1,
        1,
        6,
        vec![
            Node::input(0),
            Node::const_i64(9),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap()
}

fn opaque_affine_program(rounds: usize) -> Program {
    let mut nodes = vec![Node::input(0)];
    let mut current = 0u16;
    for _ in 0..rounds {
        let eq = nodes.len() as u16;
        nodes.push(Node::eq(current, current));
        let zero = nodes.len() as u16;
        nodes.push(Node::const_i64(0));
        let selected = nodes.len() as u16;
        nodes.push(Node::select_i64(eq, current, zero));
        current = selected;
    }
    let two = nodes.len() as u16;
    nodes.push(Node::const_i64(2));
    let doubled = nodes.len() as u16;
    nodes.push(Node::mul(current, two));
    let five = nodes.len() as u16;
    nodes.push(Node::const_i64(5));
    let shifted = nodes.len() as u16;
    nodes.push(Node::add(doubled, five));
    nodes.push(Node::output(shifted, Ty::I64));
    Program::new(Target::Cpu, 1, 1, nodes.len() as u32, nodes).unwrap()
}

#[test]
fn hot_node_executes_once_then_hits_ram_memo() {
    let path = fresh_path("hot");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func = monster.store().store(program.bytes()).unwrap();

    let first = monster.call_bytes(&func, &3i64.to_le_bytes()).unwrap();
    let second = monster.call_bytes(&func, &3i64.to_le_bytes()).unwrap();
    assert_eq!(first.result, second.result);
    assert_eq!(first.source, MonsterSource::StructuralRule);
    assert_eq!(second.source, MonsterSource::RamMemo);

    let stats = monster.stats();
    assert_eq!(stats.executions, 0);
    assert_eq!(stats.rule_hits, 1);
    assert_eq!(stats.ram_memo_hits, 1);
    assert_eq!(stats.program_cache_misses, 1);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn peek_call_returns_true_when_brain_resolves_without_interpreter() {
    let path = fresh_path("peek");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    // Affine program — caught by the StructuralRule layer, never needs
    // the interpreter. peek_call should report `true` immediately.
    let func = monster.store().store(program().bytes()).unwrap();
    let args = 7i64.to_le_bytes();

    assert!(monster.peek_call(&func, &args).unwrap());
    assert_eq!(monster.stats().executions, 0);

    // After dispatch the same call hits RamMemo — peek still reports true.
    let _ = monster.call_bytes(&func, &args).unwrap();
    assert!(monster.peek_call(&func, &args).unwrap());

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn batch_mode_collapses_duplicates_before_node_work() {
    let path = fresh_path("batch");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func = monster.store().store(program.bytes()).unwrap();
    let args = vec![
        1i64.to_le_bytes().to_vec(),
        2i64.to_le_bytes().to_vec(),
        1i64.to_le_bytes().to_vec(),
        2i64.to_le_bytes().to_vec(),
        1i64.to_le_bytes().to_vec(),
    ];

    let calls = monster.call_many_bytes(&func, &args).unwrap();
    assert_eq!(calls.len(), 5);
    assert_eq!(calls[0].result, calls[2].result);
    assert_eq!(calls[1].result, calls[3].result);

    let stats = monster.stats();
    assert_eq!(stats.executions, 0);
    assert_eq!(stats.rule_hits, 2);
    assert_eq!(stats.batch_dedupe_hits, 3);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn parallel_batch_preserves_results() {
    let path = fresh_path("parallel-batch");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func = monster.store().store(program.bytes()).unwrap();
    let args = (0..32)
        .map(|i| ((i % 4) as i64).to_le_bytes().to_vec())
        .collect::<Vec<_>>();

    let calls = monster.call_many_bytes_parallel(&func, &args).unwrap();
    assert_eq!(calls.len(), 32);
    assert_eq!(calls[0].result, calls[4].result);
    assert_eq!(calls[1].result, calls[5].result);
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, 4);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn value_path_returns_bytes_without_reloading_result_blob() {
    let path = fresh_path("value");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func = monster.store().store(program.bytes()).unwrap();

    let values = monster.call_many_values_i64(&func, &[4, 4, 5]).unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], values[1]);
    assert_ne!(values[1], values[2]);
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, 2);
    assert_eq!(monster.stats().batch_dedupe_hits, 1);
    // Φ.ν.7g — Le RAM cache n'est plus peuplé par call_many_values_i64.
    // Refactor session 2026-05-03 : call_many_values_i64 boucle
    // call_one_i64 dont le AffineI64 fast path SKIP intentionnellement
    // le cache (cf. exec.rs:1116 "intentionally NOT consulted nor
    // populated"). Plus rapide ET cohérent entre call_one_i64 et
    // call_many_values_i64.
    assert_eq!(monster.result_cache_len(), 0);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn large_value_batch_uses_internal_affine_fast_lane() {
    let path = fresh_path("batch-affine-lane");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func = monster.store().store(program.bytes()).unwrap();
    let values = (-1024..1024).collect::<Vec<i64>>();

    let out = monster.call_many_values_i64(&func, &values).unwrap();

    assert_eq!(out.len(), values.len());
    for (got, input) in out.iter().zip(values.iter()) {
        assert_eq!(*got, input.wrapping_mul(9).wrapping_add(1));
    }
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, values.len() as u64);
    assert_eq!(monster.result_cache_len(), 0);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn large_value_batch_uses_internal_jit_for_non_affine_graphs() {
    let path = fresh_path("batch-jit-lane");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        5,
        vec![
            Node::input(0),
            Node::mul(0, 0),
            Node::const_i64(3),
            Node::add(1, 2),
            Node::output(3, Ty::I64),
        ],
    )
    .unwrap();
    let func = monster.store().store(program.bytes()).unwrap();
    let values = (-1024..1024).collect::<Vec<i64>>();

    let out = monster.call_many_values_i64(&func, &values).unwrap();

    assert_eq!(out.len(), values.len());
    for (got, input) in out.iter().zip(values.iter()) {
        assert_eq!(*got, input.wrapping_mul(*input).wrapping_add(3));
    }
    assert_eq!(monster.stats().executions, values.len() as u64);
    assert_eq!(monster.stats().rule_hits, 0);
    assert_eq!(monster.result_cache_len(), 0);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn qpu_target_refuses_classical_fallback() {
    let path = fresh_path("qpu-miss");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let func = monster.store().store(qpu_program().bytes()).unwrap();
    let err = monster.call_bytes(&func, &3i64.to_le_bytes()).unwrap_err();

    assert!(err.to_string().contains("QPU target"));

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn qpu_target_can_be_replayed_from_external_result() {
    let path = fresh_path("qpu-hit");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let func = monster.store().store(qpu_program().bytes()).unwrap();
    let args = 3i64.to_le_bytes();
    let result = 28i64.to_le_bytes();

    let result_hash = monster.ingest_external_result(&func, &args, &result).unwrap();
    let call = monster.call_bytes(&func, &args).unwrap();

    assert_eq!(call.result, result_hash);
    assert_eq!(call.source, MonsterSource::RamMemo);
    assert_eq!(monster.stats().executions, 0);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn semantic_fingerprint_shares_memos_across_different_structures() {
    let path = fresh_path("semantic-memo");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let mul_two = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(2),
            Node::mul(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    let add_self = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![Node::input(0), Node::add(0, 0), Node::output(1, Ty::I64)],
    )
    .unwrap();
    let f_mul = monster.store().store(mul_two.bytes()).unwrap();
    let f_add = monster.store().store(add_self.bytes()).unwrap();
    let args = 21i64.to_le_bytes();

    let first = monster.call_bytes(&f_mul, &args).unwrap();
    let second = monster.call_bytes(&f_add, &args).unwrap();

    assert_eq!(first.result, second.result);
    assert_eq!(first.source, MonsterSource::StructuralRule);
    assert_eq!(second.source, MonsterSource::RamMemo);
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, 1);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn static_rule_answers_without_execution() {
    let path = fresh_path("static-rule");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        7,
        vec![
            Node::input(0),
            Node::const_i64(0),
            Node::mul(0, 1),
            Node::const_i64(99),
            Node::add(2, 3),
            Node::sub(4, 3),
            Node::output(5, Ty::I64),
        ],
    )
    .unwrap();
    let func = monster.store().store(program.bytes()).unwrap();
    let value = monster.call_many_values_i64(&func, &[123]).unwrap();

    assert_eq!(value, vec![0]);
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, 1);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn affine_rule_answers_without_interpreter_for_new_graph_shapes() {
    let path = fresh_path("affine-rule");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = Program::new(
        Target::Cpu,
        1,
        1,
        10,
        vec![
            Node::input(0),
            Node::add(0, 0),
            Node::const_i64(3),
            Node::mul(1, 2),
            Node::const_i64(7),
            Node::sub(3, 4),
            Node::const_i64(-2),
            Node::mul(5, 6),
            Node::const_i64(5),
            Node::output(7, Ty::I64),
        ],
    )
    .unwrap();
    let func = monster.store().store(program.bytes()).unwrap();
    let call = monster.call_bytes(&func, &4i64.to_le_bytes()).unwrap();
    let result_bytes = monster.store().load(&call.result).unwrap();

    assert_eq!(call.source, MonsterSource::StructuralRule);
    assert_eq!(i64::from_le_bytes(result_bytes.try_into().unwrap()), -34);
    assert_eq!(monster.stats().executions, 0);
    assert_eq!(monster.stats().rule_hits, 1);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn sovereign_nodes_can_gossip_memos_without_a_central_runtime() {
    let path_a = fresh_path("swarm-a");
    let path_b = fresh_path("swarm-b");
    let node_a = MonsterNode::new(
        Store::open(&path_a).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let node_b = MonsterNode::new(
        Store::open(&path_b).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func_a = node_a.store().store(program.bytes()).unwrap();
    let func_b = node_b.store().store(program.bytes()).unwrap();
    let args = 7i64.to_le_bytes();

    let first = node_a.call_bytes(&func_a, &args).unwrap();
    assert_eq!(first.source, MonsterSource::StructuralRule);

    let frame = node_a.export_swarm_frame(32).unwrap();
    let wire = frame.encode();
    let decoded = SwarmKnowledgeFrame::decode(&wire).unwrap();
    let imported = node_b.import_swarm_frame(&decoded).unwrap();
    let replay = node_b.call_bytes(&func_b, &args).unwrap();

    assert_eq!(imported, 1);
    assert_eq!(replay.result, first.result);
    assert_eq!(replay.source, MonsterSource::RamMemo);
    assert_eq!(node_b.stats().executions, 0);

    let _ = std::fs::remove_dir_all(&path_a);
    let _ = std::fs::remove_dir_all(&path_b);
}

#[test]
fn sovereign_nodes_can_sync_directly_without_a_wire_intermediary() {
    let path_a = fresh_path("direct-a");
    let path_b = fresh_path("direct-b");
    let node_a = MonsterNode::new(
        Store::open(&path_a).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let node_b = MonsterNode::new(
        Store::open(&path_b).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = program();
    let func_a = node_a.store().store(program.bytes()).unwrap();
    let func_b = node_b.store().store(program.bytes()).unwrap();
    let args = 11i64.to_le_bytes();

    let first = node_a.call_bytes(&func_a, &args).unwrap();
    assert_eq!(first.source, MonsterSource::StructuralRule);

    let imported = node_b.sync_direct_from(&node_a, 32).unwrap();
    let replay = node_b.call_bytes(&func_b, &args).unwrap();

    assert_eq!(imported, 1);
    assert_eq!(replay.result, first.result);
    assert_eq!(replay.source, MonsterSource::RamMemo);
    assert_eq!(node_b.stats().executions, 0);

    let _ = std::fs::remove_dir_all(&path_a);
    let _ = std::fs::remove_dir_all(&path_b);
}

#[test]
fn reverse_index_finds_convergent_calls_and_pairs() {
    let path = fresh_path("reverse-convergent");
    // Explicit opt-in: this test exercises the analytic reverse-index
    // API (`calls_for_result`, `convergent_pairs`, `results_for_program`),
    // which V7 made off-by-default for hot-path performance.
    let monster = MonsterNode::new_with_reverse_index(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    // f(x) = x * x and g(x) = x + x have DIFFERENT semantic
    // fingerprints (one is multiplicative, the other affine). They
    // converge on the same result for x = 2: 2*2 == 2+2 == 4.
    let square = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![Node::input(0), Node::mul(0, 0), Node::output(1, Ty::I64)],
    )
    .unwrap();
    let double = Program::new(
        Target::Cpu,
        1,
        1,
        3,
        vec![Node::input(0), Node::add(0, 0), Node::output(1, Ty::I64)],
    )
    .unwrap();
    let f_sq = monster.store().store(square.bytes()).unwrap();
    let f_db = monster.store().store(double.bytes()).unwrap();
    let args = 2i64.to_le_bytes();

    let call_sq = monster.call_bytes(&f_sq, &args).unwrap();
    let call_db = monster.call_bytes(&f_db, &args).unwrap();
    assert_eq!(call_sq.result, call_db.result);

    // Both call keys should appear under the shared result.
    let callers = monster.calls_for_result(&call_sq.result);
    assert_eq!(callers.len(), 2, "expected two distinct call keys");

    // convergent_pairs should discover the pair.
    let pairs = monster.convergent_pairs();
    assert_eq!(pairs.len(), 1);
    let (a, b) = &pairs[0];
    assert_ne!(a, b);
    let mut found = std::collections::HashSet::new();
    found.insert(a.as_bytes());
    found.insert(b.as_bytes());
    let expected_keys: std::collections::HashSet<[u8; 32]> =
        callers.iter().map(|k| k.as_bytes()).collect();
    assert_eq!(found, expected_keys);

    // results_for_program should report at least the shared result.
    let res_sq = monster.results_for_program(&f_sq);
    assert!(res_sq.contains(&call_sq.result));

    // reverse index has exactly one entry (the shared result).
    assert_eq!(monster.reverse_index_len(), 1);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn reverse_index_disabled_when_constructed_no_reverse() {
    let path = fresh_path("reverse-disabled");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let func = monster.store().store(program().bytes()).unwrap();
    let call = monster.call_bytes(&func, &3i64.to_le_bytes()).unwrap();
    assert!(monster.calls_for_result(&call.result).is_empty());
    assert_eq!(monster.reverse_index_len(), 0);
    assert!(monster.convergent_pairs().is_empty());

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn distill_tick_synthesises_oracle_for_opaque_affine_program_without_user_calls() {
    // The self-distillation invariant: a program can sit in the
    // store with ZERO user calls and the daemon (here exercised
    // synchronously via `distill_tick`) probes it, learns the rule,
    // and persists the oracle. After one tick a brand-new arg the
    // user has never sent must be served from the oracle path —
    // not the interpreter.
    use crate::DistillConfig;

    let path = fresh_path("distill-tick");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(2 * 1024 * 1024),
    );
    // 256+ KASM nodes computing 2x + 5; the structural affine_rule
    // detector cannot see through the obfuscation, so this falls
    // into HotPlan::Interpret (oracle territory).
    let prog = opaque_affine_program(120);
    let func = monster.store().store(prog.bytes()).unwrap();

    // Force `hot_program` to load the program (so the daemon can
    // see it). We do NOT do any meaningful call yet.
    let _ = monster.call_bytes(&func, &0i64.to_le_bytes()).unwrap();

    let stats_before = monster.stats();
    assert_eq!(stats_before.distillations_succeeded, 0, "no distillation yet");

    // One synchronous tick. With DEFAULT_PROBES (13 args) and
    // ORACLE_WINDOW = 10, the affine detector fires on the 10th
    // sample and adopts Affine{2,5}.
    let newly = monster.distill_tick(&DistillConfig::default());
    assert!(newly >= 1, "tick must distill at least one program");

    let stats_after = monster.stats();
    assert!(
        stats_after.distillations_succeeded >= 1,
        "succeeded counter must rise: {} -> {}",
        stats_before.distillations_succeeded,
        stats_after.distillations_succeeded,
    );
    assert!(stats_after.distillations_attempted > stats_before.distillations_attempted);

    // Now an unseen-by-anyone arg must be served via the oracle.
    let oracle_hits_before = monster.stats().oracle_hits;
    let surprise_arg = 999_999_999i64;
    let call = monster
        .call_bytes(&func, &surprise_arg.to_le_bytes())
        .unwrap();
    let stats_post_call = monster.stats();
    assert!(
        stats_post_call.oracle_hits > oracle_hits_before,
        "oracle_hits must rise on a fresh arg after distillation"
    );
    let result_bytes = monster.store().load(&call.result).unwrap();
    let result_value = i64::from_le_bytes(result_bytes.try_into().unwrap());
    assert_eq!(
        result_value,
        2 * surprise_arg + 5,
        "distilled oracle must reproduce 2x+5 bit-exact"
    );

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn distill_daemon_does_not_loop_on_already_learned_program() {
    // After a program has a learned oracle, the daemon must not
    // keep firing probes at it (no infinite work). The candidate
    // collection step filters them out.
    use crate::DistillConfig;

    let path = fresh_path("distill-skip-learned");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let prog = opaque_affine_program(120);
    let func = monster.store().store(prog.bytes()).unwrap();
    let _ = monster.call_bytes(&func, &0i64.to_le_bytes()).unwrap();

    // First tick: distillation succeeds.
    let n1 = monster.distill_tick(&DistillConfig::default());
    assert!(n1 >= 1);

    let attempts_after_first = monster.stats().distillations_attempted;

    // Second tick: candidate must be filtered out — no new probes.
    let n2 = monster.distill_tick(&DistillConfig::default());
    assert_eq!(n2, 0, "no new distillations on already-learned program");
    let attempts_after_second = monster.stats().distillations_attempted;
    assert_eq!(
        attempts_after_first, attempts_after_second,
        "daemon must NOT re-probe an already-learned program"
    );

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn shadow_execution_invalidates_a_lying_oracle_under_load() {
    // Inject a deliberately wrong oracle into the in-RAM map for an
    // opaque-affine program (true f(x) = 2x + 5; lying oracle says
    // 99·x + 99). Drive enough calls to trigger shadow sampling and
    // verify the lie is caught and the oracle cleared.
    use crate::monster::oracle::LearnedOracle;

    let path = fresh_path("shadow-invalidate");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(2 * 1024 * 1024),
    );
    let prog = opaque_affine_program(120);
    let func = monster.store().store(prog.bytes()).unwrap();

    // Warm hot_program once so we have a fingerprint to inject under.
    let _ = monster.call_bytes(&func, &0i64.to_le_bytes()).unwrap();
    let fp = {
        let progs = monster.programs.read().unwrap();
        progs
            .get(&func)
            .expect("hot program loaded")
            .semantic_fingerprint
    };

    // Inject the lying oracle — bypassing the normal observation path
    // so we know exactly what's installed.
    {
        let mut oracles = monster.oracles.write().unwrap();
        let state = oracles.entry(fp).or_default();
        state.learned = Some(LearnedOracle::Affine { mul: 99, add: 99 });
    }
    // Also publish it so we can verify the ref gets purged on
    // invalidation (mirrors what happens when the persistence path
    // stored a bad rule).
    let key_hex = crate::monster::oracle::oracle_ref_key(&fp);
    let oracle_blob = LearnedOracle::Affine { mul: 99, add: 99 }.serialize();
    let oracle_hash = monster.store().store(&oracle_blob).unwrap();
    monster
        .store()
        .write_ref(&format!("refs/oracle/{key_hex}"), &oracle_hash, "test")
        .unwrap();
    assert!(monster
        .store()
        .lookup_ref(&format!("refs/oracle/{key_hex}"))
        .is_some());

    // Enough calls (with distinct fresh args so each goes through the
    // fast lane apply_oracle path) to guarantee shadow sampling fires.
    // SHADOW_PERIOD is 1024, so 2050 distinct calls give us ≥ 2 samples.
    let invalidations_before = monster.stats().shadow_invalidations;
    for x in 1..=2050i64 {
        // Use args far outside any cache to force the oracle path
        let arg = 1_000_000 + x;
        let _ = monster.call_bytes(&func, &arg.to_le_bytes()).unwrap();
    }
    let stats_after = monster.stats();

    assert!(
        stats_after.shadow_invalidations > invalidations_before,
        "shadow execution must catch the lie ({} -> {})",
        invalidations_before,
        stats_after.shadow_invalidations,
    );
    // The lying Affine{99,99} must be gone. The system is self-healing:
    // post-invalidation the slow lane re-observes and may re-learn the
    // *correct* rule (Affine{2,5} for opaque_affine_program(120)). So
    // we assert "no longer the lie", not "no oracle at all".
    let oracles = monster.oracles.read().unwrap();
    let current = oracles.get(&fp).and_then(|s| s.learned);
    assert!(
        current != Some(LearnedOracle::Affine { mul: 99, add: 99 }),
        "lying oracle must be replaced; got {:?}",
        current
    );
    drop(oracles);
    // The lying ref was deleted at invalidation time. The slow-lane
    // re-learn may have published a NEW (correct) oracle under the
    // same key, so we just check the current ref (if any) doesn't
    // decode to the lie.
    if let Some(hash) = monster
        .store()
        .lookup_ref(&format!("refs/oracle/{key_hex}"))
    {
        let bytes = monster.store().load(&hash).unwrap();
        let rule = LearnedOracle::deserialize(&bytes).unwrap();
        assert!(
            rule != LearnedOracle::Affine { mul: 99, add: 99 },
            "republished ref must not encode the lie"
        );
    }

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn fresh_node_inherits_oracle_via_persisted_ref() {
    // The "knowledge sharing" invariant: once node A has learned an
    // oracle for a program, a freshly-constructed node B sharing the
    // same store path must NOT need to execute the interpreter even
    // once before serving correct results — the oracle is materialised
    // from `refs/oracle/<fingerprint>` at hot_program load time.
    let path = fresh_path("oracle-inherit");
    let prog = opaque_affine_program(120); // f(x) = 2x + 5

    // Phase 1: node A learns the oracle.
    let learned_hits;
    {
        let node_a = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let func = node_a.store().store(prog.bytes()).unwrap();
        for x in 0..16i64 {
            let _ = node_a.call_bytes(&func, &x.to_le_bytes()).unwrap();
        }
        let stats = node_a.stats();
        assert!(stats.executions >= 10, "node A must have executed enough samples to learn");
        assert!(stats.oracle_hits > 0, "node A must have engaged the oracle");
        learned_hits = stats.oracle_hits;
    }

    // Phase 2: a brand-new MonsterNode opens the same store.
    {
        let node_b = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let func = node_b.store().store(prog.bytes()).unwrap();

        // Take a snapshot before the very first call.
        let before = node_b.stats();
        // Pick an arg node A NEVER saw — guarantees the cached memo
        // can't be the source of the answer.
        let novel_arg = 9_999_999i64;
        let call = node_b.call_bytes(&func, &novel_arg.to_le_bytes()).unwrap();
        let after = node_b.stats();

        // The result must be bit-exact correct via the oracle.
        let bytes = node_b.store().load(&call.result).unwrap();
        let value = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(value, 2 * novel_arg + 5, "inherited oracle must match 2x+5");

        // No interpreter run on node B.
        assert_eq!(
            after.executions, before.executions,
            "fresh node must not re-execute opaque programs an oracle covers"
        );
        // The oracle was used at least once.
        assert!(after.oracle_hits > before.oracle_hits, "oracle hit on fresh node");
    }

    let _ = learned_hits; // silence unused if the asserts move
    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn learned_oracle_takes_over_after_observing_opaque_affine_graph() {
    let path = fresh_path("learned-oracle");
    let monster = MonsterNode::new(
        Store::open(&path).unwrap(),
        MemoryGovernor::new(1024 * 1024),
    );
    let program = opaque_affine_program(90);
    assert!(program.nodes().len() > 256);
    let func = monster.store().store(program.bytes()).unwrap();

    let mut sources = Vec::new();
    for value in 0..16i64 {
        let call = monster.call_bytes(&func, &value.to_le_bytes()).unwrap();
        sources.push(call.source.clone());
        let result = monster.store().load(&call.result).unwrap();
        assert_eq!(i64::from_le_bytes(result.try_into().unwrap()), value * 2 + 5);
    }

    assert_eq!(monster.stats().executions, 10);
    assert_eq!(monster.stats().oracle_hits, 6);
    assert!(sources.iter().take(10).all(|source| *source == MonsterSource::ExecutedHot));
    assert!(sources.iter().skip(10).all(|source| *source == MonsterSource::LearnedOracle));

    let _ = std::fs::remove_dir_all(&path);
}
