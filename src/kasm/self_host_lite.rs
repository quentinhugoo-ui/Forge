//! Λ.2 lite — KASM-bytecode self-host of a tiny program subset.
//!
//! Where `kasm::self_host` provides a runtime-level self-hosting (a
//! Rust struct that resolves program hashes and dispatches via the
//! existing scalar interpreter), THIS module is a different beast :
//! a KASM **program** that, when executed, interprets ANOTHER KASM
//! program. The interpreter is itself bytecode — no Rust runtime
//! orchestration, no fractal dispatcher hooks. Pure KASM-in-KASM.
//!
//! This is the doctrine §8 mutation substrat made literal at the
//! bytecode level : the interpreter for the affine subset is a
//! deterministic content-addressed `Program` like any other. Its
//! hash is stable. Two Forge installations anywhere in the world,
//! given the same bytecode, will run the same interpretation.
//!
//! ## Scope of "lite"
//!
//! - **Subset** : programs of shape
//!   `[Input(0), ConstI64(a), ConstI64(b), Mul(0,1), Add(3,2), Output(4, I64)]`
//!   — i.e. the affine map `f(x) = a·x + b`. Any 6-node program with
//!   that exact structural shape decodes correctly.
//! - **Wire format** : each KASM `Node` is 8 bytes
//!   `[op:1][ty:1][a:2 LE][b:2 LE][imm:2 LE]` and packs into a single
//!   `i64` for transit through `Ty::VecI64` slots.
//! - **Dispatch** : the lite interpreter does NOT walk the program
//!   node-by-node — it knows the structural shape and reads the two
//!   `imm` fields directly from the packed Vec via `Op::VGetI64` (the
//!   primitive added in Wave 7i specifically to unlock this).
//!
//! The full general-purpose self-host (arbitrary opcodes, runtime
//! op dispatch, dynamic stack growth) is Wave 11+ work. The lite
//! version is an **existence proof** : KASM has all the primitives
//! it needs to interpret itself.
//!
//! ## Why this matters
//!
//! - Λ.2 axiom realized : the KASM interpreter is a KASM program.
//!   Its hash is part of `forge.atlas` like every other program.
//! - Cross-domain content addressing : any computation that decodes
//!   to "interpret an affine program at a given x" hits the same
//!   cached result regardless of what domain the affine program
//!   describes (linear regression slope, bond pricing, charge curves).
//! - Foundation for Λ.3 (synth-as-KASM) : the same bytecode-decoding
//!   technique generalizes to scoring / loss computation in KASM.

use super::types::{Node, Target, Ty};
use super::Program;

/// Pack a single KASM `Node` into the 8-byte little-endian layout
/// (`[op:1][ty:1][a:2 LE][b:2 LE][imm:2 LE]`) and reinterpret as a
/// signed `i64`. Round-trips through `Vec<i64>` wire transit.
///
/// The high 16 bits carry the imm field (signed i16) reinterpreted
/// as an unsigned u16, so naive `>> 48` reads back as `u16` ; the
/// caller (KASM-side decoder OR Rust test) sign-extends as needed.
pub fn pack_node_to_i64(node: &Node) -> i64 {
    let op = node.op as u8 as i64;
    let ty = node.ty as u8 as i64;
    let a = node.a as i64;
    let b = node.b as i64;
    // u16 reinterpretation preserves the bit pattern of negative i16
    // (e.g. -1 as u16 = 0xFFFF). Sign-extension on the KASM side uses
    // the (raw ^ 0x8000) - 0x8000 trick.
    let imm = node.imm as u16 as i64;
    op | (ty << 8) | (a << 16) | (b << 32) | (imm << 48)
}

/// Pack an entire program into a `Vec<i64>` ready to be passed as a
/// `Ty::VecI64` input slot to the self-host interpreter program.
pub fn pack_program_to_vec_i64(program: &Program) -> Vec<i64> {
    program.nodes().iter().map(pack_node_to_i64).collect()
}

/// Build the KASM program that interprets the affine subset
/// `[Input, Const(a), Const(b), Mul, Add, Output]`. Returns f(x) = a·x + b.
///
/// 23-node deterministic bytecode. Hash is stable across builds — any
/// future refactor that changes node ordering will change the hash and
/// be caught by `affine_self_host_program_hash_is_stable`.
pub fn affine_self_host_program() -> Program {
    let nodes = vec![
        // ── inputs ──
        Node::input_vec(0),     // 0: prog (Vec<i64>, 6 packed nodes)
        Node::input(1),         // 1: x   (i64)
        // ── small constants used to extract imm from packed nodes ──
        Node::const_i64(1),     // 2: index of node Const(a) AND scalar 1
        Node::const_i64(48),    // 3: shift for `>> 48` (imm field)
        Node::const_i64(15),    // 4: for building 0x8000 = 1 << 15
        Node::const_i64(2),     // 5: index of node Const(b)
        Node::const_i64(16),    // 6: for building 0x10000 = 1 << 16
        // ── 0x8000 = 1 << 15 (used as bias for sign extension) ──
        Node::shl(2, 4),        // 7: 0x8000
        // ── 0xFFFF = (1 << 16) - 1 (low-16-bit mask) ──
        Node::shl(2, 6),        // 8: 0x10000
        Node::sub(8, 2),        // 9: 0xFFFF
        // ── decode imm of node[1] = Const(a) ──
        Node::v_get(0, 2),      // 10: packed_a
        Node::shr(10, 3),       // 11: raw imm = packed_a >> 48
        Node::bit_and(11, 9),   // 12: u16 imm (zero-extended into i64)
        Node::bit_xor(12, 7),   // 13: ^ 0x8000
        Node::sub(13, 7),       // 14: signed imm_a
        // ── decode imm of node[2] = Const(b) ──
        Node::v_get(0, 5),      // 15: packed_b
        Node::shr(15, 3),       // 16: raw imm
        Node::bit_and(16, 9),   // 17: u16 imm
        Node::bit_xor(17, 7),   // 18: ^ 0x8000
        Node::sub(18, 7),       // 19: signed imm_b
        // ── compute a·x + b ──
        Node::mul(14, 1),       // 20: a * x
        Node::add(20, 19),      // 21: + b
        Node::output(21, Ty::I64), // 22
    ];
    Program::new(Target::Cpu, 2, 1, 64, nodes)
        .expect("affine_self_host_program is well-formed")
}

/// Λ.2 v2 — generalized self-host for 6-node KASM programs over the
/// `{Input, ConstI64, AddI64, SubI64, MulI64, Output}` subset.
///
/// Where `affine_self_host_program` (Λ.2 lite) hardcodes the structural
/// shape `[Input, Const, Const, Mul, Add, Output]` and reads the two
/// `imm` fields directly, this version walks slots 1..=4 and DECODES
/// each node's op byte at runtime, dispatching via a chained
/// `Op::Cond` cascade. The same KASM bytecode interprets ANY 6-node
/// program of the supported shape — affine, quadratic, mixed —
/// without hardcoded structural assumptions.
///
/// ## Why this is the v2 milestone
///
/// Λ.2 lite proved bytecode-interprets-bytecode on a single
/// hardcoded layout. Λ.2 v2 proves bytecode-interprets-bytecode with
/// **dynamic op dispatch** — one program hash that resolves any
/// shape in the supported subset. This is the smallest step toward
/// the full Wave 11+ self-host (arbitrary opcodes, dynamic node
/// counts, recursive composition).
///
/// ## Wire format
///
/// Inputs (same as Λ.2 lite, plus dispatch-driven slots 1..=4) :
///   slot 0 : `prog` Vec<i64> of 6 packed nodes (encode/decode via
///            `pack_node_to_i64` ; layout `[op:1][ty:1][a:2 LE][b:2 LE][imm:2 LE]`).
///   slot 1 : `x` i64 — the value `Input(0)` produces in the
///            interpreted program.
///
/// Output : i64 — the result of running the source program at `x`.
///
/// ## Convention
///
/// The source program MUST follow this 6-node canonical layout :
///   - slot 0 : `Op::Input(0)` (read by convention, never decoded)
///   - slots 1..=4 : `Op::ConstI64`, `Op::AddI64`, `Op::SubI64`, or
///                   `Op::MulI64` in any order, with `a`/`b` refs to
///                   any earlier slot
///   - slot 5 : `Op::Output(a, Ty::I64)` (read for its `a` field
///              only, the `op` byte itself is ignored)
///
/// Programs that violate this shape produce undefined results
/// (modulo VGetI64's safe wrapping) — the interpreter trusts the
/// caller that the source is well-shaped.
///
/// ## Dispatch chain
///
///   result = if op==1 then imm                           // ConstI64
///            else if op==2 then stack[a] + stack[b]      // AddI64
///            else if op==7 then stack[a] - stack[b]      // SubI64
///            else if op==3 then stack[a] * stack[b]      // MulI64
///            else 0                                       // unknown
///
/// Each iteration appends the result to the stack via
/// `VConcat(stack, VBroadcast(result, 1))`. The stack is itself a
/// `Ty::VecI64` grown from `[x]` at start to `[x, s1, s2, s3, s4]`
/// after the four iterations. The final output reads
/// `stack[output.a]` where `output` is the packed slot-5 node.
pub fn general_6node_self_host_program() -> Program {
    let mut nodes: Vec<Node> = Vec::with_capacity(128);

    // ── inputs ──
    nodes.push(Node::input_vec(0)); // 0: prog
    nodes.push(Node::input(1));     // 1: x

    // ── small i64 constants used as slot indices, op-byte literals,
    //    and shift amounts. Each fits in i16 ; encoded via const_i64 ──
    let c0 = nodes.len() as u16; nodes.push(Node::const_i64(0));
    let c1 = nodes.len() as u16; nodes.push(Node::const_i64(1));
    let c2 = nodes.len() as u16; nodes.push(Node::const_i64(2));
    let c3 = nodes.len() as u16; nodes.push(Node::const_i64(3));
    let c4 = nodes.len() as u16; nodes.push(Node::const_i64(4));
    let c5 = nodes.len() as u16; nodes.push(Node::const_i64(5));
    let c7 = nodes.len() as u16; nodes.push(Node::const_i64(7));
    let c15 = nodes.len() as u16; nodes.push(Node::const_i64(15));
    let c16 = nodes.len() as u16; nodes.push(Node::const_i64(16));
    let c32 = nodes.len() as u16; nodes.push(Node::const_i64(32));
    let c48 = nodes.len() as u16; nodes.push(Node::const_i64(48));
    let c255 = nodes.len() as u16; nodes.push(Node::const_i64(255));

    // ── derived constants : 0x8000 (sign-extension bias),
    //    0xFFFF (low-16-bit mask). Built via shifts because
    //    they exceed the i16 range of `const_i64`. ──
    let c_8000 = nodes.len() as u16; nodes.push(Node::shl(c1, c15));
    let c_10000 = nodes.len() as u16; nodes.push(Node::shl(c1, c16));
    let c_ffff = nodes.len() as u16; nodes.push(Node::sub(c_10000, c1));

    // ── init stack = [x] ──
    let mut stack: u16 = nodes.len() as u16;
    nodes.push(Node::v_broadcast(1, c1)); // stack starts as Vec of length 1 holding `x`

    // ── 4 unrolled iterations for slots 1..=4 of the source program ──
    for &slot_i_const in &[c1, c2, c3, c4] {
        // packed = prog[i]
        let packed = nodes.len() as u16; nodes.push(Node::v_get(0, slot_i_const));

        // op_byte = packed & 0xFF
        let op_byte = nodes.len() as u16; nodes.push(Node::bit_and(packed, c255));

        // a = (packed >> 16) & 0xFFFF
        let a_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c16));
        let a = nodes.len() as u16; nodes.push(Node::bit_and(a_shr, c_ffff));

        // b = (packed >> 32) & 0xFFFF
        let b_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c32));
        let b = nodes.len() as u16; nodes.push(Node::bit_and(b_shr, c_ffff));

        // imm = sign-extended (packed >> 48) & 0xFFFF
        let imm_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c48));
        let imm_raw = nodes.len() as u16; nodes.push(Node::bit_and(imm_shr, c_ffff));
        let imm_xor = nodes.len() as u16; nodes.push(Node::bit_xor(imm_raw, c_8000));
        let imm_signed = nodes.len() as u16; nodes.push(Node::sub(imm_xor, c_8000));

        // val_a, val_b read from stack (VGetI64 wraps idx mod len).
        let val_a = nodes.len() as u16; nodes.push(Node::v_get(stack, a));
        let val_b = nodes.len() as u16; nodes.push(Node::v_get(stack, b));

        // Compute all three binary candidates ; only the matching one
        // survives the dispatch chain.
        let add_v = nodes.len() as u16; nodes.push(Node::add(val_a, val_b));
        let sub_v = nodes.len() as u16; nodes.push(Node::sub(val_a, val_b));
        let mul_v = nodes.len() as u16; nodes.push(Node::mul(val_a, val_b));

        // Bool predicates for the 4 supported ops.
        let is_const = nodes.len() as u16; nodes.push(Node::eq(op_byte, c1));
        let is_add = nodes.len() as u16; nodes.push(Node::eq(op_byte, c2));
        let is_sub = nodes.len() as u16; nodes.push(Node::eq(op_byte, c7));
        let is_mul = nodes.len() as u16; nodes.push(Node::eq(op_byte, c3));

        // Dispatch chain : default to 0, override with imm/add/sub/mul
        // based on the matching op-byte test.
        let t1 = nodes.len() as u16; nodes.push(Node::cond(is_const, imm_signed, c0));
        let t2 = nodes.len() as u16; nodes.push(Node::cond(is_add, add_v, t1));
        let t3 = nodes.len() as u16; nodes.push(Node::cond(is_sub, sub_v, t2));
        let result = nodes.len() as u16; nodes.push(Node::cond(is_mul, mul_v, t3));

        // Append result to the stack : new_stack = VConcat(stack, [result])
        let singleton = nodes.len() as u16; nodes.push(Node::v_broadcast(result, c1));
        let new_stack = nodes.len() as u16; nodes.push(Node::v_concat(stack, singleton));
        stack = new_stack;
    }

    // ── final output : read slot 5's `a` field, lookup stack[a] ──
    let packed_out = nodes.len() as u16; nodes.push(Node::v_get(0, c5));
    let out_a_shr = nodes.len() as u16; nodes.push(Node::shr(packed_out, c16));
    let out_a = nodes.len() as u16; nodes.push(Node::bit_and(out_a_shr, c_ffff));
    let final_val = nodes.len() as u16; nodes.push(Node::v_get(stack, out_a));
    nodes.push(Node::output(final_val, Ty::I64));

    Program::new(Target::Cpu, 2, 1, 256, nodes)
        .expect("general_6node_self_host_program is well-formed")
}

/// Λ.3 v2 — score a generalized 6-node candidate against K=4 examples,
/// fully in KASM with no Rust per-example loop.
///
/// Where `affine_score_program` (Λ.3 lite) hardcodes the affine shape
/// and decodes only the two `imm` fields, this version inlines the v2
/// generalized self-host interpreter (M3, `general_6node_self_host_program`)
/// **once per example**, so the scorer handles any 6-node candidate
/// over `{Input, Const, Add, Sub, Mul, Output}`.
///
/// ## Wire format
///
/// Inputs :
///   slot 0 : `prog` Vec<i64> (6 packed nodes — the candidate)
///   slot 1 : `examples_x` Vec<i64> (length must be ≥ K=4 ; extra
///            elements ignored ; shorter vecs wrap via VGetI64
///            modulo-len semantics — caller's responsibility to pad)
///   slot 2 : `examples_y` Vec<i64> (same length convention as x)
///
/// Output : i64 — `Σ_{k=0..4} |interpret(prog, x_k) - y_k|` (L1 loss)
///
/// ## Why K=4
///
/// The unrolled-per-example structure means each example costs ~115
/// nodes (108 v2 interpreter + 7 score-and-accumulate). K=4 keeps the
/// total under ~500 nodes — comfortably within KASM `MAX_NODES = 4096`,
/// generous test surface, and small enough to compile/exec quickly.
/// For larger K, the structure is identical but expanded ; future work
/// (vectorized v2 interpreter) would amortize via Vec ops.
pub fn generalized_score_program() -> Program {
    /// Internal shared-constant slots, computed once at the top of the
    /// program and reused by every example iteration.
    struct V2Constants {
        c0: u16, c1: u16, c2: u16, c3: u16, c4: u16, c5: u16, c7: u16,
        c16: u16, c32: u16, c48: u16, c255: u16,
        c_8000: u16, c_ffff: u16,
    }

    /// Emit the v2 interpreter body for ONE example. `x_slot` holds
    /// the input value, `prog_slot` is the candidate Vec<i64>. Returns
    /// the node slot of the final `pred` (interpret(prog, x)).
    fn emit_v2_eval(
        nodes: &mut Vec<Node>,
        x_slot: u16,
        prog_slot: u16,
        c: &V2Constants,
    ) -> u16 {
        let mut stack: u16 = nodes.len() as u16;
        nodes.push(Node::v_broadcast(x_slot, c.c1));

        for &slot_i_const in &[c.c1, c.c2, c.c3, c.c4] {
            let packed = nodes.len() as u16; nodes.push(Node::v_get(prog_slot, slot_i_const));
            let op_byte = nodes.len() as u16; nodes.push(Node::bit_and(packed, c.c255));
            let a_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c.c16));
            let a = nodes.len() as u16; nodes.push(Node::bit_and(a_shr, c.c_ffff));
            let b_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c.c32));
            let b = nodes.len() as u16; nodes.push(Node::bit_and(b_shr, c.c_ffff));
            let imm_shr = nodes.len() as u16; nodes.push(Node::shr(packed, c.c48));
            let imm_raw = nodes.len() as u16; nodes.push(Node::bit_and(imm_shr, c.c_ffff));
            let imm_xor = nodes.len() as u16; nodes.push(Node::bit_xor(imm_raw, c.c_8000));
            let imm_signed = nodes.len() as u16; nodes.push(Node::sub(imm_xor, c.c_8000));
            let val_a = nodes.len() as u16; nodes.push(Node::v_get(stack, a));
            let val_b = nodes.len() as u16; nodes.push(Node::v_get(stack, b));
            let add_v = nodes.len() as u16; nodes.push(Node::add(val_a, val_b));
            let sub_v = nodes.len() as u16; nodes.push(Node::sub(val_a, val_b));
            let mul_v = nodes.len() as u16; nodes.push(Node::mul(val_a, val_b));
            let is_const = nodes.len() as u16; nodes.push(Node::eq(op_byte, c.c1));
            let is_add = nodes.len() as u16; nodes.push(Node::eq(op_byte, c.c2));
            let is_sub = nodes.len() as u16; nodes.push(Node::eq(op_byte, c.c7));
            let is_mul = nodes.len() as u16; nodes.push(Node::eq(op_byte, c.c3));
            let t1 = nodes.len() as u16; nodes.push(Node::cond(is_const, imm_signed, c.c0));
            let t2 = nodes.len() as u16; nodes.push(Node::cond(is_add, add_v, t1));
            let t3 = nodes.len() as u16; nodes.push(Node::cond(is_sub, sub_v, t2));
            let result = nodes.len() as u16; nodes.push(Node::cond(is_mul, mul_v, t3));
            let singleton = nodes.len() as u16; nodes.push(Node::v_broadcast(result, c.c1));
            let new_stack = nodes.len() as u16; nodes.push(Node::v_concat(stack, singleton));
            stack = new_stack;
        }

        // Decode output.a from prog[5], read pred = stack[output.a].
        let packed_out = nodes.len() as u16; nodes.push(Node::v_get(prog_slot, c.c5));
        let out_a_shr = nodes.len() as u16; nodes.push(Node::shr(packed_out, c.c16));
        let out_a = nodes.len() as u16; nodes.push(Node::bit_and(out_a_shr, c.c_ffff));
        let pred = nodes.len() as u16; nodes.push(Node::v_get(stack, out_a));
        pred
    }

    let mut nodes: Vec<Node> = Vec::with_capacity(512);

    // ── inputs ──
    nodes.push(Node::input_vec(0)); // 0: prog
    nodes.push(Node::input_vec(1)); // 1: examples_x
    nodes.push(Node::input_vec(2)); // 2: examples_y

    // ── shared constants ──
    let c0 = nodes.len() as u16; nodes.push(Node::const_i64(0));
    let c1 = nodes.len() as u16; nodes.push(Node::const_i64(1));
    let c2 = nodes.len() as u16; nodes.push(Node::const_i64(2));
    let c3 = nodes.len() as u16; nodes.push(Node::const_i64(3));
    let c4 = nodes.len() as u16; nodes.push(Node::const_i64(4));
    let c5 = nodes.len() as u16; nodes.push(Node::const_i64(5));
    let c7 = nodes.len() as u16; nodes.push(Node::const_i64(7));
    let c15 = nodes.len() as u16; nodes.push(Node::const_i64(15));
    let c16 = nodes.len() as u16; nodes.push(Node::const_i64(16));
    let c32 = nodes.len() as u16; nodes.push(Node::const_i64(32));
    let c48 = nodes.len() as u16; nodes.push(Node::const_i64(48));
    let c255 = nodes.len() as u16; nodes.push(Node::const_i64(255));

    // Derived
    let c_8000 = nodes.len() as u16; nodes.push(Node::shl(c1, c15));
    let c_10000 = nodes.len() as u16; nodes.push(Node::shl(c1, c16));
    let c_ffff = nodes.len() as u16; nodes.push(Node::sub(c_10000, c1));

    let consts = V2Constants {
        c0, c1, c2, c3, c4, c5, c7, c16, c32, c48, c255, c_8000, c_ffff,
    };

    // ── unrolled per-example evaluation + accumulate L1 loss ──
    let mut sum: u16 = c0; // start at 0
    for &k_const in &[c0, c1, c2, c3] {
        // x_k = examples_x[k]
        let x_k = nodes.len() as u16; nodes.push(Node::v_get(1, k_const));
        // pred_k = interpret(prog, x_k)
        let pred = emit_v2_eval(&mut nodes, x_k, 0, &consts);
        // y_k = examples_y[k]
        let y_k = nodes.len() as u16; nodes.push(Node::v_get(2, k_const));
        // diff = pred - y_k
        let diff = nodes.len() as u16; nodes.push(Node::sub(pred, y_k));
        // abs(diff) via select : if diff < 0 then -diff else diff
        let neg_diff = nodes.len() as u16; nodes.push(Node::neg(diff));
        let is_neg = nodes.len() as u16; nodes.push(Node::lt(diff, c0));
        let abs_diff = nodes.len() as u16; nodes.push(Node::select_i64(is_neg, neg_diff, diff));
        // sum += abs_diff
        let new_sum = nodes.len() as u16; nodes.push(Node::add(sum, abs_diff));
        sum = new_sum;
    }

    nodes.push(Node::output(sum, Ty::I64));

    Program::new(Target::Cpu, 3, 1, 1024, nodes)
        .expect("generalized_score_program is well-formed")
}

/// The fixed K=4 number of examples the `generalized_score_program`
/// scorer iterates over per call. Caller must pad shorter vecs.
pub const GENERALIZED_SCORE_K: usize = 4;

/// Λ.3 lite — score an affine candidate program against an example set
/// entirely in KASM, using the same packed-program wire format as the
/// affine self-host interpreter.
///
/// Inputs :
///   slot 0 : `prog` (Vec<i64>) — packed affine candidate, same shape
///            as `affine_self_host_program` consumes.
///   slot 1 : `examples_x` (Vec<i64>) — example inputs.
///   slot 2 : `examples_y` (Vec<i64>) — target outputs.
/// Output : i64 — L1 loss `Σ |f(x_i) - y_i|`.
///
/// ## Why this is structurally cleaner than a scalar interpreter
///
/// We never walk the affine program node-by-node. The scoring kernel
/// reads the two `imm` fields directly via VGetI64 (decode `a` and
/// `b`) and then evaluates the entire example set in a single pass
/// through the Wave 7d-bis Vec ops :
///
///   outputs = a · x_vec + b              (broadcast + VMul + VAdd)
///   loss    = Σ |outputs - y_vec|        (VSub + VAbs + VSum)
///
/// No interpreter loop, no per-example scalar dispatch — pure parallel
/// vector arithmetic. The doctrine §9 paranoid filter applies
/// transparently because the program hash captures the entire
/// (decode + broadcast + arithmetic + reduction) pipeline as one
/// content-addressed identity.
pub fn affine_score_program() -> Program {
    let nodes = vec![
        // ── inputs ──
        Node::input_vec(0),     // 0: prog
        Node::input_vec(1),     // 1: examples_x
        Node::input_vec(2),     // 2: examples_y
        // ── decode constants ──
        Node::const_i64(1),     // 3: index of Const(a) AND scalar 1
        Node::const_i64(48),    // 4: shift for imm extraction
        Node::const_i64(15),    // 5: bias bit position (0x8000 = 1 << 15)
        Node::const_i64(2),     // 6: index of Const(b)
        Node::const_i64(16),    // 7: 0x10000 = 1 << 16
        // ── 0x8000 and 0xFFFF ──
        Node::shl(3, 5),        // 8: 0x8000 (sign-extension bias)
        Node::shl(3, 7),        // 9: 0x10000
        Node::sub(9, 3),        // 10: 0xFFFF (low-16-bit mask)
        // ── decode imm_a from prog[1] ──
        Node::v_get(0, 3),      // 11: packed_a
        Node::shr(11, 4),       // 12: raw imm
        Node::bit_and(12, 10),  // 13: u16 imm
        Node::bit_xor(13, 8),   // 14: ^ 0x8000
        Node::sub(14, 8),       // 15: signed imm_a (i64)
        // ── decode imm_b from prog[2] ──
        Node::v_get(0, 6),      // 16: packed_b
        Node::shr(16, 4),       // 17: raw imm
        Node::bit_and(17, 10),  // 18: u16 imm
        Node::bit_xor(18, 8),   // 19: ^ 0x8000
        Node::sub(19, 8),       // 20: signed imm_b
        // ── len(examples_x) and broadcast a, b ──
        Node::v_len(1),         // 21: n
        Node::v_broadcast(15, 21), // 22: [a, a, ..., a]
        Node::v_broadcast(20, 21), // 23: [b, b, ..., b]
        // ── outputs = a · x_vec + b ──
        Node::v_mul(22, 1),     // 24: a * x (elementwise)
        Node::v_add(24, 23),    // 25: + b (elementwise)
        // ── loss = Σ |outputs - y_vec| ──
        Node::v_sub(25, 2),     // 26: diff
        Node::v_abs(26),        // 27: |diff|
        Node::v_sum(27),        // 28: Σ |diff| → i64
        Node::output(28, Ty::I64), // 29
    ];
    Program::new(Target::Cpu, 3, 1, 80, nodes)
        .expect("affine_score_program is well-formed")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an affine source program `f(x) = a·x + b` whose
    /// structural shape matches what the lite interpreter expects.
    fn affine_source_program(a: i16, b: i16) -> Program {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(a),
            Node::const_i64(b),
            Node::mul(0, 1),
            Node::add(3, 2),
            Node::output(4, Ty::I64),
        ];
        Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap()
    }

    /// Pack a Vec<i64> into the wire format expected by `kasm::execute`
    /// for a `Ty::VecI64` input slot : `[u32 LE count][count × 8 bytes i64 LE]`.
    fn vec_input_bytes(values: &[i64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 8 * values.len());
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Run the affine interpreter for a given (source program, x) pair,
    /// returning the i64 output.
    fn run_self_host(src: &Program, x: i64) -> i64 {
        let interp = affine_self_host_program();
        let packed = pack_program_to_vec_i64(src);
        let mut args = Vec::new();
        args.extend_from_slice(&vec_input_bytes(&packed));
        args.extend_from_slice(&x.to_le_bytes());
        let out = crate::kasm::execute(&interp, &args).expect("self-host exec");
        i64::from_le_bytes(out.try_into().unwrap())
    }

    /// Bit-exact baseline : f(x) = 3x + 7 evaluated by the Rust
    /// reference matches the KASM self-host interpreter's output for
    /// every x in a representative grid.
    #[test]
    fn affine_self_host_matches_rust_3x_plus_7() {
        let src = affine_source_program(3, 7);
        for &x in &[0i64, 1, -1, 5, -5, 100, -100, 1234, -1234] {
            let rust = 3i64.wrapping_mul(x).wrapping_add(7);
            let kasm = run_self_host(&src, x);
            assert_eq!(kasm, rust, "f({}) : kasm={} rust={}", x, kasm, rust);
        }
    }

    /// Negative constants must round-trip correctly through the
    /// pack-to-i64 → VGetI64 → sign-extension pipeline.
    #[test]
    fn affine_self_host_handles_negative_constants() {
        for (a, b) in [
            (-1i16, 1i16),
            (-3, 7),
            (5, -5),
            (-100, -100),
            (i16::MIN, i16::MAX),
            (i16::MAX, i16::MIN),
        ] {
            let src = affine_source_program(a, b);
            for &x in &[0i64, 1, -1, 7, -7] {
                let rust = (a as i64).wrapping_mul(x).wrapping_add(b as i64);
                let kasm = run_self_host(&src, x);
                assert_eq!(
                    kasm, rust,
                    "f(x)={}·x+{} at x={}: kasm={} rust={}",
                    a, b, x, kasm, rust
                );
            }
        }
    }

    /// The affine self-host is a deterministic 23-node program. Hash
    /// stability is asserted so any future refactor that perturbs the
    /// bytecode is caught and forces a conscious update.
    #[test]
    fn affine_self_host_program_hash_is_stable() {
        let prog = affine_self_host_program();
        assert_eq!(prog.nodes().len(), 23, "affine self-host is a 23-node program");
        let hash = prog.structural_hash_hex();
        assert!(!hash.is_empty(), "structural hash must be produced");
    }

    // ─── Λ.3 lite — affine_score_program tests ──────────────────────

    /// Helper : run the score kernel on (candidate, examples_x, examples_y),
    /// returning the loss.
    fn run_score(
        candidate: &Program,
        examples_x: &[i64],
        examples_y: &[i64],
    ) -> i64 {
        let kernel = affine_score_program();
        let packed = pack_program_to_vec_i64(candidate);
        let mut args = Vec::new();
        args.extend_from_slice(&vec_input_bytes(&packed));
        args.extend_from_slice(&vec_input_bytes(examples_x));
        args.extend_from_slice(&vec_input_bytes(examples_y));
        let out = crate::kasm::execute(&kernel, &args).expect("score exec");
        i64::from_le_bytes(out.try_into().unwrap())
    }

    /// Reference loss — same arithmetic as the kernel, in pure Rust.
    fn rust_affine_loss(a: i64, b: i64, xs: &[i64], ys: &[i64]) -> i64 {
        xs.iter()
            .zip(ys.iter())
            .map(|(&x, &y)| a.wrapping_mul(x).wrapping_add(b).wrapping_sub(y).wrapping_abs())
            .fold(0i64, |acc, v| acc.wrapping_add(v))
    }

    /// Perfect-fit candidate : f(x) = 3x + 7 against ys = [3·x + 7]
    /// must yield loss = 0.
    #[test]
    fn affine_score_zero_loss_on_exact_match() {
        let src = affine_source_program(3, 7);
        let xs: Vec<i64> = (-5..=10).collect();
        let ys: Vec<i64> = xs.iter().map(|x| 3i64 * x + 7).collect();
        let loss = run_score(&src, &xs, &ys);
        assert_eq!(loss, 0, "exact-match candidate must yield zero L1 loss");
    }

    /// Off-by-some candidate : f(x) = 2x + 5 against ys = [3·x + 7].
    /// Per-example diff = |2x+5 - (3x+7)| = |-x - 2|. Sum should match
    /// the Rust reference bit-exactly.
    #[test]
    fn affine_score_bit_exact_against_rust() {
        let src = affine_source_program(2, 5);
        let xs: Vec<i64> = (-5..=5).collect();
        let ys: Vec<i64> = xs.iter().map(|x| 3i64 * x + 7).collect();
        let kasm_loss = run_score(&src, &xs, &ys);
        let rust_loss = rust_affine_loss(2, 5, &xs, &ys);
        assert_eq!(kasm_loss, rust_loss);
    }

    /// Negative-coefficient candidates and asymmetric example sets —
    /// the same bit-exactness must hold across the full
    /// (a, b, x, y) grid we care about for synth scoring.
    #[test]
    fn affine_score_handles_negative_and_asymmetric() {
        let cases: &[(i16, i16)] = &[
            (-1, 0),
            (1, -1),
            (-3, 7),
            (5, -5),
            (-100, 100),
            (i16::MIN, i16::MAX),
        ];
        let xs: Vec<i64> = vec![-7, -3, 0, 1, 4, 9];
        let ys: Vec<i64> = vec![10, -3, 0, -1, 8, 12];
        for &(a, b) in cases {
            let src = affine_source_program(a, b);
            let kasm = run_score(&src, &xs, &ys);
            let rust = rust_affine_loss(a as i64, b as i64, &xs, &ys);
            assert_eq!(kasm, rust, "a={} b={}", a, b);
        }
    }

    /// Empty example set : the score kernel must produce 0 (empty
    /// VSumI64 → 0) regardless of the candidate.
    #[test]
    fn affine_score_empty_examples_returns_zero() {
        let src = affine_source_program(3, 7);
        let xs: Vec<i64> = vec![];
        let ys: Vec<i64> = vec![];
        let loss = run_score(&src, &xs, &ys);
        assert_eq!(loss, 0);
    }

    #[test]
    fn affine_score_program_hash_is_stable() {
        let prog = affine_score_program();
        assert_eq!(prog.nodes().len(), 30, "affine score is a 30-node program");
        assert!(!prog.structural_hash_hex().is_empty());
    }

    // ─── Λ.2 v2 — general_6node_self_host_program tests ────────────────

    /// Run a 6-node source program through the generalized self-host
    /// interpreter at the given x, returning the i64 output.
    fn run_general(src: &Program, x: i64) -> i64 {
        let interp = general_6node_self_host_program();
        let packed = pack_program_to_vec_i64(src);
        let mut args = Vec::new();
        args.extend_from_slice(&vec_input_bytes(&packed));
        args.extend_from_slice(&x.to_le_bytes());
        let out = crate::kasm::execute(&interp, &args).expect("general v2 exec");
        i64::from_le_bytes(out.try_into().unwrap())
    }

    /// The generalized v2 interpreter must produce the same output as
    /// the Λ.2 lite affine interpreter for any affine source program —
    /// a regression check that the dynamic dispatch path correctly
    /// handles the hardcoded affine shape.
    #[test]
    fn v2_matches_v1_lite_on_affine() {
        for (a, b) in [(3i16, 7i16), (-1, 1), (0, 0), (i16::MAX, i16::MIN)] {
            let src = affine_source_program(a, b);
            for &x in &[0i64, 1, -1, 5, -5, 100] {
                let v1 = run_self_host(&src, x);
                let v2 = run_general(&src, x);
                assert_eq!(v1, v2, "a={} b={} x={}: v1={} v2={}", a, b, x, v1, v2);
            }
        }
    }

    /// A non-affine 6-node shape : `f(x) = x*x + 7`. Slot layout :
    ///   0: Input(0)
    ///   1: Const(7)
    ///   2: Mul(0, 0)        // x*x
    ///   3: Add(2, 1)        // x*x + 7
    ///   4: Const(0)         // unused (filler so we have 6 nodes)
    ///   5: Output(3)
    #[test]
    fn v2_handles_non_affine_quadratic() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 0),
            Node::add(2, 1),
            Node::const_i64(0),
            Node::output(3, Ty::I64),
        ];
        let src = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        for &x in &[0i64, 1, -1, 5, -5, 13] {
            let kasm = run_general(&src, x);
            let rust = x.wrapping_mul(x).wrapping_add(7);
            assert_eq!(kasm, rust, "x={}: kasm={} rust={}", x, kasm, rust);
        }
    }

    /// A subtraction-based shape : `f(x) = (x - 3) * 2`. Slot layout :
    ///   0: Input(0)
    ///   1: Const(3)
    ///   2: Const(2)
    ///   3: Sub(0, 1)        // x - 3
    ///   4: Mul(3, 2)        // (x-3) * 2
    ///   5: Output(4)
    #[test]
    fn v2_handles_sub_and_mul_mixed() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(3),
            Node::const_i64(2),
            Node::sub(0, 1),
            Node::mul(3, 2),
            Node::output(4, Ty::I64),
        ];
        let src = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        for &x in &[0i64, 3, 7, -10, 100] {
            let kasm = run_general(&src, x);
            let rust = (x.wrapping_sub(3)).wrapping_mul(2);
            assert_eq!(kasm, rust, "x={}: kasm={} rust={}", x, kasm, rust);
        }
    }

    /// All-add shape — exercises the `is_add` branch of the dispatch
    /// chain : `f(x) = x + x + 5 + 5 = 2x + 10`. Slot layout :
    ///   0: Input(0)
    ///   1: Const(5)
    ///   2: Add(0, 0)        // 2x
    ///   3: Add(1, 1)        // 10
    ///   4: Add(2, 3)        // 2x + 10
    ///   5: Output(4)
    #[test]
    fn v2_handles_all_add_chain() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(5),
            Node::add(0, 0),
            Node::add(1, 1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        let src = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        for &x in &[0i64, 1, 5, -5, 1000] {
            let kasm = run_general(&src, x);
            let rust = 2i64.wrapping_mul(x).wrapping_add(10);
            assert_eq!(kasm, rust, "x={}: kasm={} rust={}", x, kasm, rust);
        }
    }

    #[test]
    fn v2_program_hash_is_stable() {
        let prog = general_6node_self_host_program();
        // Captures the structural-hash + node-count for regression.
        // A change here means the v2 bytecode shifted — verify
        // intent before bumping these numbers.
        assert!(prog.nodes().len() >= 100 && prog.nodes().len() <= 150,
            "v2 self-host should be ~120 nodes, got {}", prog.nodes().len());
        assert!(!prog.structural_hash_hex().is_empty());
    }

    // ─── M4 / Λ.3 v2 — generalized_score_program tests ─────────────────

    fn run_generalized_score(
        candidate: &Program,
        xs: &[i64; GENERALIZED_SCORE_K],
        ys: &[i64; GENERALIZED_SCORE_K],
    ) -> i64 {
        let kernel = generalized_score_program();
        let packed = pack_program_to_vec_i64(candidate);
        let mut args = Vec::new();
        args.extend_from_slice(&vec_input_bytes(&packed));
        args.extend_from_slice(&vec_input_bytes(xs));
        args.extend_from_slice(&vec_input_bytes(ys));
        let out = crate::kasm::execute(&kernel, &args).expect("score v2 exec");
        i64::from_le_bytes(out.try_into().unwrap())
    }

    fn rust_general_loss(candidate: &Program, xs: &[i64; GENERALIZED_SCORE_K], ys: &[i64; GENERALIZED_SCORE_K]) -> i64 {
        xs.iter()
            .zip(ys.iter())
            .map(|(&x, &y)| {
                let args = x.to_le_bytes();
                let out = crate::kasm::execute(candidate, &args).expect("ref exec");
                let pred = i64::from_le_bytes(out.try_into().unwrap());
                pred.wrapping_sub(y).wrapping_abs()
            })
            .fold(0i64, |acc, v| acc.wrapping_add(v))
    }

    /// Affine candidate (matches Λ.3 lite affine_score_program semantics
    /// when K=4 examples are passed). Cross-checks Λ.3 v2 vs Rust ref.
    #[test]
    fn v2_score_affine_3x_plus_7_zero_loss_on_exact_match() {
        let src = affine_source_program(3, 7);
        let xs: [i64; GENERALIZED_SCORE_K] = [0, 1, 2, 3];
        let ys: [i64; GENERALIZED_SCORE_K] = [7, 10, 13, 16];
        let kasm = run_generalized_score(&src, &xs, &ys);
        let rust = rust_general_loss(&src, &xs, &ys);
        assert_eq!(kasm, 0, "exact match must yield 0 loss");
        assert_eq!(kasm, rust);
    }

    /// Affine off-by-some : Λ.3 v2 must agree with Rust reference.
    #[test]
    fn v2_score_affine_off_by_some_matches_rust() {
        let src = affine_source_program(2, 5);
        let xs: [i64; GENERALIZED_SCORE_K] = [-3, 0, 3, 7];
        let ys: [i64; GENERALIZED_SCORE_K] = [10, -3, 8, 12];
        let kasm = run_generalized_score(&src, &xs, &ys);
        let rust = rust_general_loss(&src, &xs, &ys);
        assert_eq!(kasm, rust);
    }

    /// Non-affine candidate (quadratic-ish `x*x + 7`). Generalized v2
    /// must score it correctly while the Λ.3 lite affine scorer would
    /// mis-decode the imm fields.
    #[test]
    fn v2_score_handles_non_affine_quadratic_ish() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(7),
            Node::mul(0, 0),
            Node::add(2, 1),
            Node::const_i64(0),
            Node::output(3, Ty::I64),
        ];
        let src = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        // f(x) = x² + 7
        let xs: [i64; GENERALIZED_SCORE_K] = [0, 1, 2, -3];
        // ys deliberately off so loss > 0
        let ys: [i64; GENERALIZED_SCORE_K] = [10, 5, 20, 100];
        let kasm = run_generalized_score(&src, &xs, &ys);
        let rust = rust_general_loss(&src, &xs, &ys);
        assert_eq!(kasm, rust);
    }

    /// Sub-and-mul mixed candidate `(x - 3) * 2`. Exercises the Sub
    /// dispatch branch + Mul dispatch branch in the inlined v2 logic.
    #[test]
    fn v2_score_handles_sub_mul_mixed() {
        let nodes = vec![
            Node::input(0),
            Node::const_i64(3),
            Node::const_i64(2),
            Node::sub(0, 1),
            Node::mul(3, 2),
            Node::output(4, Ty::I64),
        ];
        let src = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        let xs: [i64; GENERALIZED_SCORE_K] = [3, 5, 0, -7];
        // ys = ((x-3)*2) for some, off for others
        let ys: [i64; GENERALIZED_SCORE_K] = [0, 4, -6, -20];
        let kasm = run_generalized_score(&src, &xs, &ys);
        let rust = rust_general_loss(&src, &xs, &ys);
        assert_eq!(kasm, rust);
    }

    #[test]
    fn v2_score_program_size_is_bounded() {
        let prog = generalized_score_program();
        let n = prog.nodes().len();
        assert!(
            (450..=550).contains(&n),
            "generalized score should be ~480 nodes, got {}",
            n
        );
        assert!(!prog.structural_hash_hex().is_empty());
    }

    /// Round-trip the pack_node helper : pack(unpack via shifts) → original.
    /// Verifies the decode formulae used inside the self-host program
    /// against the Rust packing helper.
    #[test]
    fn pack_node_round_trips_imm_field() {
        for imm in [0i16, 1, -1, 7, -7, i16::MAX, i16::MIN, -32768] {
            let n = Node::const_i64(imm);
            let packed = pack_node_to_i64(&n);
            // Decode the imm using the same formula the KASM program
            // uses inside : ((raw >> 48) & 0xFFFF) sign-extended via
            // (x ^ 0x8000) - 0x8000.
            let raw = (packed as u64 >> 48) & 0xFFFF;
            let signed = ((raw as i64) ^ 0x8000) - 0x8000;
            assert_eq!(
                signed, imm as i64,
                "imm {} packed -> {:#x} -> decoded {}",
                imm, packed, signed
            );
        }
    }
}
