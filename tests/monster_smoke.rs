//! End-to-end smoke test: verify + run a KASM program through Monster.

use std::path::PathBuf;
use std::time::SystemTime;

use scan::kasm::{compose, Node, Program, Target, Ty};
use scan::{MemoryGovernor, MonsterNode, Store};

fn fresh_path(tag: &str) -> PathBuf {
    let mut p = std::env::current_dir().unwrap();
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(".codex-tmp");
    p.push(format!("scan-kasm-{tag}-{nanos}"));
    p
}

fn affine_program() -> Program {
    // out = (input0 * 3) + 1
    Program::new(
        Target::Cpu,
        1,
        1,
        6,
        vec![
            Node::input(0),
            Node::const_i64(3),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ],
    )
    .unwrap()
}

fn composed_program() -> Program {
    let double = Program::new(
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
    let plus_one = Program::new(
        Target::Cpu,
        1,
        1,
        4,
        vec![
            Node::input(0),
            Node::const_i64(1),
            Node::add(0, 1),
            Node::output(2, Ty::I64),
        ],
    )
    .unwrap();
    compose(&double, &plus_one, Target::Cpu).unwrap()
}

#[test]
fn kasm_affine_smoke() {
    let path = fresh_path("smoke");
    let store = Store::open(&path).unwrap();
    let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));

    let program = affine_program();
    let func = node.store().store(program.bytes()).unwrap();
    let args = node.store().store(&14i64.to_le_bytes()).unwrap();
    let result_hash = node.call(&func, &args).expect("call").result;

    let bytes = node.store().load(&result_hash).unwrap();
    let value = i64::from_le_bytes(bytes.try_into().unwrap());
    assert_eq!(value, 43);

    // Re-call hits the same memoized result hash.
    let r2 = node.call(&func, &args).unwrap().result;
    assert_eq!(r2, result_hash);

    let _ = std::fs::remove_dir_all(&path);
}

#[test]
fn composed_kasm_program_is_memoized_like_any_other_program() {
    let path = fresh_path("composed");
    let store = Store::open(&path).unwrap();
    let node = MonsterNode::new(store, MemoryGovernor::new(1024 * 1024));

    let program = composed_program();
    let func = node.store().store(program.bytes()).unwrap();
    let args = node.store().store(&21i64.to_le_bytes()).unwrap();
    let result_hash = node.call(&func, &args).expect("call").result;

    let bytes = node.store().load(&result_hash).unwrap();
    let value = i64::from_le_bytes(bytes.try_into().unwrap());
    assert_eq!(value, 43);
    assert_eq!(node.call(&func, &args).unwrap().result, result_hash);

    let _ = std::fs::remove_dir_all(&path);
}
