//! Canonicalisation, partial evaluation, semantic fingerprint and
//! static-output detection — every pure transformation that turns one
//! valid Program into a smaller (or equivalent) Program.

use std::collections::HashMap;

use sha2::{Digest, Sha256};

use super::interpreter::execute;
use super::program::{checked_imm_ref, digest, hash_i64, Program};
use super::types::{F64SubOp, KasmError, Node, Op, Ty};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Known {
    I64(i64),
    Bool(bool),
    /// Φ.0 — Folded F64 constant. Stored as the `i64` bit pattern
    /// (`f64::to_bits`) to keep `Known` `Eq`-friendly: `f64` itself
    /// breaks `Eq` because of `NaN != NaN`. Bit-pattern equality is
    /// the right notion here anyway — two F64 constants are KASM-
    /// equivalent iff their bit patterns match, regardless of `NaN`
    /// quirks.
    F64(i64),
    Ref(u16, Ty),
}

impl Eq for Known {}

pub fn canonicalize(program: &Program) -> Result<Program, KasmError> {
    // `ReduceAddI64`/`ReduceMulI64` reference a *contiguous* range of
    // source nodes by `[a, a + imm)`. Re-indexing through fingerprint
    // ordering would break that range. Until a dedicated re-emit path
    // exists, programs that contain reduce ops are returned untouched.
    if has_range_op(program) {
        return Program::new(
            program.target(),
            program.inputs(),
            program.outputs(),
            program.fuel(),
            program.nodes().to_vec(),
        );
    }

    let mut nodes = Vec::new();
    let mut seen = HashMap::<Node, u16>::new();
    let mut old_to_new = vec![None; program.nodes().len()];
    let mut fingerprints = HashMap::new();

    for (source, ty) in program.output_sources() {
        let source = canonical_node(
            program,
            source as usize,
            &mut nodes,
            &mut seen,
            &mut old_to_new,
            &mut fingerprints,
        )?;
        nodes.push(Node::output(source, ty));
    }

    Program::new(program.target(), program.inputs(), program.outputs(), nodes.len() as u32, nodes)
}

fn has_range_op(program: &Program) -> bool {
    program
        .nodes()
        .iter()
        .any(|n| matches!(n.op, Op::ReduceAddI64 | Op::ReduceMulI64))
}

pub fn simplify(program: &Program) -> Result<Program, KasmError> {
    // Reduce nodes can't be re-indexed safely yet, so simplifier just
    // returns the canonical-equivalent (which is the program itself in
    // that case).
    if has_range_op(program) {
        return program.canonical();
    }
    let canonical = program.canonical()?;
    let mut nodes = Vec::new();
    let mut values = Vec::with_capacity(canonical.nodes().len());
    let mut seen = HashMap::<Node, u16>::new();

    for (i, node) in canonical.nodes().iter().copied().enumerate() {
        let value = match node.op {
            Op::Input => match node.ty {
                // Wave 7b — Ty::VecI64 inputs are now first-class.
                // The optimizer treats them as opaque Refs (no
                // constant folding yet — there's no `Known::Vec`
                // variant since Vec arithmetic ops don't exist
                // until Wave 7c).
                Ty::I64 | Ty::F64 | Ty::VecI64 => {
                    Known::Ref(emit_node(&mut nodes, &mut seen, node)?, node.ty)
                }
                Ty::Bool => return Err(KasmError::TypeMismatch { node: i }),
            },
            Op::ConstI64 => Known::I64(node.imm as i64),
            Op::AddI64 => simplify_add(values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::MulI64 => simplify_mul(values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::SubI64 => simplify_sub(values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::DivI64Checked => simplify_div(values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::MinI64 => simplify_minmax(true, values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::MaxI64 => simplify_minmax(false, values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::EqI64 => simplify_eq(values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::LtI64 => simplify_cmp(
                |a, b| a < b,
                Node::lt,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::LeI64 => simplify_cmp(
                |a, b| a <= b,
                Node::le,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::BitAndI64 => simplify_bitwise(
                |a, b| a & b,
                Node::bit_and,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::BitOrI64 => simplify_bitwise(
                |a, b| a | b,
                Node::bit_or,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::BitXorI64 => simplify_bitwise(
                |a, b| a ^ b,
                Node::bit_xor,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ShlI64 => simplify_bitwise(
                |a, b| ((a as u64).wrapping_shl((b as u64 & 63) as u32)) as i64,
                Node::shl,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ShrI64 => simplify_bitwise(
                |a, b| ((a as u64).wrapping_shr((b as u64 & 63) as u32)) as i64,
                Node::shr,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::SatAddI64 => simplify_bitwise(
                |a, b| a.saturating_add(b),
                Node::sat_add,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::SatSubI64 => simplify_bitwise(
                |a, b| a.saturating_sub(b),
                Node::sat_sub,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ModI64Checked => simplify_bitwise(
                |a, b| a.checked_rem(b).unwrap_or(0),
                Node::mod_checked,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ClampI64 => {
                let v = values[node.a as usize];
                let lo = values[node.b as usize];
                let hi = values[checked_imm_ref(node.imm, i)? as usize];
                if let (Known::I64(v), Known::I64(lo), Known::I64(hi)) = (v, lo, hi) {
                    Known::I64(v.max(lo).min(hi))
                } else {
                    let v = materialize_i64(v, &mut nodes, &mut seen)?;
                    let lo = materialize_i64(lo, &mut nodes, &mut seen)?;
                    let hi = materialize_i64(hi, &mut nodes, &mut seen)?;
                    Known::Ref(emit_node(&mut nodes, &mut seen, Node::clamp(v, lo, hi))?, Ty::I64)
                }
            }
            Op::ReduceAddI64 | Op::ReduceMulI64 => {
                // Excluded by `has_range_op` above. Keeping a defensive
                // arm so an accidental call still produces a clear error
                // rather than a panic.
                return Err(KasmError::BadReduceCount {
                    node: i,
                    count: node.imm,
                });
            }
            Op::Hash64 => {
                let a = values[node.a as usize];
                if let Known::I64(v) = a {
                    Known::I64(hash_i64(v))
                } else {
                    let a = materialize_i64(a, &mut nodes, &mut seen)?;
                    Known::Ref(emit_node(&mut nodes, &mut seen, Node::hash64(a))?, Ty::I64)
                }
            }
            Op::BitFlipI64 => simplify_unary_i64(
                |v| !v,
                Node::bit_flip,
                Op::BitFlipI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::NegI64 => simplify_unary_i64(
                |v| v.wrapping_neg(),
                Node::neg,
                Op::NegI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ReverseBitsI64 => simplify_unary_i64(
                |v| v.reverse_bits(),
                Node::reverse_bits,
                Op::ReverseBitsI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::ByteswapI64 => simplify_unary_i64(
                |v| v.swap_bytes(),
                Node::byteswap,
                Op::ByteswapI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::PopcntI64 => simplify_unary_i64(
                |v| crate::cpu_bits::popcount_u64(v as u64) as i64,
                Node::popcnt,
                Op::PopcntI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::LzcntI64 => simplify_unary_i64(
                |v| crate::cpu_bits::leading_zeros_u64(v as u64) as i64,
                Node::lzcnt,
                Op::LzcntI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::TzcntI64 => simplify_unary_i64(
                |v| crate::cpu_bits::trailing_zeros_u64(v as u64) as i64,
                Node::tzcnt,
                Op::TzcntI64,
                values[node.a as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::PextI64 => simplify_bitwise(
                |a, b| crate::cpu_bits::pext_u64(a as u64, b as u64) as i64,
                Node::pext,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::PdepI64 => simplify_bitwise(
                |a, b| crate::cpu_bits::pdep_u64(a as u64, b as u64) as i64,
                Node::pdep,
                values[node.a as usize],
                values[node.b as usize],
                &mut nodes,
                &mut seen,
            )?,
            Op::Lazy => {
                let child = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                if let Some(Node { op: Op::Lazy, .. }) = nodes.get(child as usize).copied() {
                    Known::Ref(child, Ty::I64)
                } else {
                    let idx = emit_node(&mut nodes, &mut seen, Node::lazy(child))?;
                    Known::Ref(idx, Ty::I64)
                }
            }
            Op::Force => {
                let future = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                if let Some(Node { op: Op::Lazy, a: child, .. }) = nodes.get(future as usize).copied() {
                    Known::Ref(child, Ty::I64)
                } else {
                    let idx = emit_node(&mut nodes, &mut seen, Node::force(future))?;
                    Known::Ref(idx, Ty::I64)
                }
            }
            Op::SelectI64 => {
                let cond = values[node.a as usize];
                let yes = values[node.b as usize];
                let no = values[checked_imm_ref(node.imm, i)? as usize];
                match cond {
                    Known::Bool(true) => yes,
                    Known::Bool(false) => no,
                    _ if yes == no => yes,
                    _ => {
                        let cond = materialize_bool(cond, &mut nodes, &mut seen)?;
                        let yes = materialize_i64(yes, &mut nodes, &mut seen)?;
                        let no = materialize_i64(no, &mut nodes, &mut seen)?;
                        Known::Ref(emit_node(&mut nodes, &mut seen, Node::select_i64(cond, yes, no))?, Ty::I64)
                    }
                }
            }
            Op::AndBool => simplify_bool_bin(true, values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::OrBool => simplify_bool_bin(false, values[node.a as usize], values[node.b as usize], &mut nodes, &mut seen)?,
            Op::NotBool => simplify_not(values[node.a as usize], &mut nodes, &mut seen)?,
            Op::ConstF64 => Known::F64((node.imm as f64).to_bits() as i64),
            Op::F64Op => simplify_f64_op(node, &values, &mut nodes, &mut seen, i)?,
            Op::Output => {
                let source = match node.ty {
                    Ty::I64 => materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?,
                    Ty::Bool => materialize_bool(values[node.a as usize], &mut nodes, &mut seen)?,
                    Ty::F64 => materialize_f64(values[node.a as usize], &mut nodes, &mut seen)?,
                    // Wave 7b — Vec outputs : the source must be a
                    // Vec slot (Known::Ref(_, VecI64)). No constant
                    // folding for Vec values — there's no
                    // `Known::Vec` variant. If the upstream optimizer
                    // path produced a non-Ref Vec value, that's a
                    // bug — fail-loud with TypeMismatch.
                    Ty::VecI64 => match values[node.a as usize] {
                        Known::Ref(idx, Ty::VecI64) => idx,
                        _ => return Err(KasmError::TypeMismatch { node: i }),
                    },
                };
                nodes.push(Node::output(source, node.ty));
                Known::Ref(source, node.ty)
            }
            // KASM v1.0 — Op::Comptime: load-time constant fold. If the
            // wrapped slot is already a known constant (Known::I64), we
            // bypass the wrapper and propagate the constant — that's
            // exactly what `comptime` means semantically (Mojo @comptime,
            // Zig comptime). The materialize step further down picks the
            // shortest encoding (ConstI64 if fits in i16, multi-op chain
            // otherwise). For Op::Memoize / Op::Adaptive, same propagation
            // because they're pass-through wrappers — folding the
            // constant out makes the program shorter and the hash
            // captures the specialization.
            Op::Comptime | Op::Memoize | Op::Adaptive => {
                // Inline the wrapped value. The optimizer's normal const-
                // folding paths take it from here.
                values[node.a as usize]
            }
            // Op::Grad, Op::Vmap, Op::Pmap : opaque meta-ops. Materialize
            // the dep and re-emit the meta-op unchanged — only the brain
            // dispatch can resolve them.
            Op::Grad | Op::Vmap | Op::Pmap => {
                let a = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                let new_node = Node { op: node.op, ty: node.ty, a, b: 0, imm: node.imm };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, node.ty)
            }
            Op::Cond => {
                let pred = materialize_bool(values[node.a as usize], &mut nodes, &mut seen)?;
                let then_v = materialize_i64(values[node.b as usize], &mut nodes, &mut seen)?;
                let else_v = materialize_i64(
                    values[node.imm as usize], &mut nodes, &mut seen,
                )?;
                let new_node = Node {
                    op: Op::Cond, ty: Ty::I64, a: pred, b: then_v, imm: else_v as i16,
                };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::I64)
            }
            Op::Pipeline | Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan
            | Op::Fractal | Op::Eval => {  // Wave 8 — opaque to optimizer
                let a = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                let b = materialize_i64(values[node.b as usize], &mut nodes, &mut seen)?;
                let new_node = Node { op: node.op, ty: node.ty, a, b, imm: node.imm };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, node.ty)
            }
            Op::VLenI64 => {
                // Wave 7d — Vec length query. Source must be a Vec
                // slot (Known::Ref(_, VecI64)) ; emit the new length
                // node referencing it. Output type is I64.
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let new_node = Node { op: Op::VLenI64, ty: Ty::I64, a, b: 0, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, node.ty)
            }
            // Wave 7d-bis — VSumI64 unary, VAddI64/VMulI64 binary,
            // all opaque Refs (no constant folding for Vec values).
            Op::VSumI64 => {
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let new_node = Node { op: Op::VSumI64, ty: Ty::I64, a, b: 0, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, node.ty)
            }
            Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
            | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64 => {
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let b = match values[node.b as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let new_node = Node { op: node.op, ty: Ty::VecI64, a, b, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::VecI64)
            }
            Op::VRangeI64 => {
                // Wave 7e — Vec range from i64 length slot.
                let a = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                let new_node = Node { op: Op::VRangeI64, ty: Ty::VecI64, a, b: 0, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::VecI64)
            }
            Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
                // Wave 7f + 7h — Vec unary transformations.
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let new_node = Node { op: node.op, ty: Ty::VecI64, a, b: 0, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::VecI64)
            }
            Op::VConcatI64 => {
                // Wave 7f — concatenate two Vec slots.
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let b = match values[node.b as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let new_node = Node { op: Op::VConcatI64, ty: Ty::VecI64, a, b, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::VecI64)
            }
            Op::VBroadcastI64 => {
                // Wave 7f — broadcast scalar i64 over length.
                let a = materialize_i64(values[node.a as usize], &mut nodes, &mut seen)?;
                let b = materialize_i64(values[node.b as usize], &mut nodes, &mut seen)?;
                let new_node = Node { op: Op::VBroadcastI64, ty: Ty::VecI64, a, b, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::VecI64)
            }
            Op::VGetI64 => {
                // Wave 7i — Vec random-access read : (Vec, i64) → i64.
                // Source must be a Vec slot ; index materializes as i64.
                let a = match values[node.a as usize] {
                    Known::Ref(idx, Ty::VecI64) => idx,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let b = materialize_i64(values[node.b as usize], &mut nodes, &mut seen)?;
                let new_node = Node { op: Op::VGetI64, ty: Ty::I64, a, b, imm: 0 };
                let idx = emit_node(&mut nodes, &mut seen, new_node)?;
                Known::Ref(idx, Ty::I64)
            }
        };
        values.push(value);
    }

    Program::new(canonical.target(), canonical.inputs(), canonical.outputs(), nodes.len() as u32, nodes)?.canonical()
}

fn simplify_add(a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (Known::I64(0), x) | (x, Known::I64(0)) => Ok(x),
        (Known::I64(a), Known::I64(b)) => Ok(Known::I64(a.wrapping_add(b))),
        _ => emit_i64_bin(Node::add, a, b, nodes, seen),
    }
}

fn simplify_mul(a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (Known::I64(0), _) | (_, Known::I64(0)) => Ok(Known::I64(0)),
        (Known::I64(1), x) | (x, Known::I64(1)) => Ok(x),
        (Known::I64(a), Known::I64(b)) => Ok(Known::I64(a.wrapping_mul(b))),
        _ => emit_i64_bin(Node::mul, a, b, nodes, seen),
    }
}

fn simplify_sub(a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (x, Known::I64(0)) => Ok(x),
        (a, b) if a == b => Ok(Known::I64(0)),
        (Known::I64(a), Known::I64(b)) => Ok(Known::I64(a.wrapping_sub(b))),
        _ => emit_i64_bin(Node::sub, a, b, nodes, seen),
    }
}

fn simplify_div(a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (_, Known::I64(0)) => Ok(Known::I64(0)),
        (x, Known::I64(1)) => Ok(x),
        (Known::I64(a), Known::I64(b)) => Ok(Known::I64(a.checked_div(b).unwrap_or(0))),
        _ => emit_i64_bin(Node::div_checked, a, b, nodes, seen),
    }
}

fn simplify_minmax(is_min: bool, a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (a, b) if a == b => Ok(a),
        (Known::I64(a), Known::I64(b)) => Ok(Known::I64(if is_min { a.min(b) } else { a.max(b) })),
        _ => emit_i64_bin(if is_min { Node::min } else { Node::max }, a, b, nodes, seen),
    }
}

fn simplify_cmp(
    fold: fn(i64, i64) -> bool,
    make: fn(u16, u16) -> Node,
    a: Known,
    b: Known,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
) -> Result<Known, KasmError> {
    if let (Known::I64(av), Known::I64(bv)) = (a, b) {
        return Ok(Known::Bool(fold(av, bv)));
    }
    let a = materialize_i64(a, nodes, seen)?;
    let b = materialize_i64(b, nodes, seen)?;
    Ok(Known::Ref(emit_node(nodes, seen, make(a, b))?, Ty::Bool))
}

fn simplify_bitwise(
    fold: fn(i64, i64) -> i64,
    make: fn(u16, u16) -> Node,
    a: Known,
    b: Known,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
) -> Result<Known, KasmError> {
    if let (Known::I64(av), Known::I64(bv)) = (a, b) {
        return Ok(Known::I64(fold(av, bv)));
    }
    let a = materialize_i64(a, nodes, seen)?;
    let b = materialize_i64(b, nodes, seen)?;
    Ok(Known::Ref(emit_node(nodes, seen, make(a, b))?, Ty::I64))
}

fn simplify_eq(a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (a, b) {
        (a, b) if a == b => Ok(Known::Bool(true)),
        (Known::I64(a), Known::I64(b)) => Ok(Known::Bool(a == b)),
        _ => {
            let a = materialize_i64(a, nodes, seen)?;
            let b = materialize_i64(b, nodes, seen)?;
            Ok(Known::Ref(emit_node(nodes, seen, Node::eq(a, b))?, Ty::Bool))
        }
    }
}

fn simplify_bool_bin(is_and: bool, a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match (is_and, a, b) {
        (_, a, b) if a == b => Ok(a),
        (true, Known::Bool(false), _) | (true, _, Known::Bool(false)) => Ok(Known::Bool(false)),
        (true, Known::Bool(true), x) | (true, x, Known::Bool(true)) => Ok(x),
        (false, Known::Bool(true), _) | (false, _, Known::Bool(true)) => Ok(Known::Bool(true)),
        (false, Known::Bool(false), x) | (false, x, Known::Bool(false)) => Ok(x),
        _ => {
            let a = materialize_bool(a, nodes, seen)?;
            let b = materialize_bool(b, nodes, seen)?;
            Ok(Known::Ref(emit_node(nodes, seen, if is_and { Node::and(a, b) } else { Node::or(a, b) })?, Ty::Bool))
        }
    }
}

/// Helper pour les ops unaires bijectives I64 → I64 (Ω-6.1) avec
/// élimination du double-flip : `op(op(x)) = x` quand op est involutif.
/// `make` construit le Node, `op_kind` permet de matcher l'inner pour
/// l'élimination involutive.
fn simplify_unary_i64(
    fold: fn(i64) -> i64,
    make: fn(u16) -> Node,
    op_kind: Op,
    a: Known,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
) -> Result<Known, KasmError> {
    if let Known::I64(v) = a {
        return Ok(Known::I64(fold(v)));
    }
    let a = materialize_i64(a, nodes, seen)?;
    if let Some(node) = nodes.get(a as usize).copied() {
        if node.op == op_kind {
            return Ok(Known::Ref(node.a, Ty::I64));
        }
    }
    Ok(Known::Ref(emit_node(nodes, seen, make(a))?, Ty::I64))
}

fn simplify_not(a: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    match a {
        Known::Bool(v) => Ok(Known::Bool(!v)),
        _ => {
            let a = materialize_bool(a, nodes, seen)?;
            let node = nodes.get(a as usize).copied();
            if let Some(Node { op: Op::NotBool, a: inner, .. }) = node {
                Ok(Known::Ref(inner, Ty::Bool))
            } else {
                Ok(Known::Ref(emit_node(nodes, seen, Node::not(a))?, Ty::Bool))
            }
        }
    }
}

fn emit_i64_bin(make: fn(u16, u16) -> Node, a: Known, b: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<Known, KasmError> {
    let a = materialize_i64(a, nodes, seen)?;
    let b = materialize_i64(b, nodes, seen)?;
    Ok(Known::Ref(emit_node(nodes, seen, make(a, b))?, Ty::I64))
}

fn materialize_i64(value: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<u16, KasmError> {
    match value {
        Known::Ref(index, Ty::I64) => Ok(index),
        Known::I64(v) => {
            if let Ok(small) = i16::try_from(v) {
                return emit_node(nodes, seen, Node::const_i64(small));
            }
            // Tier 1 — compact `add(shl(const_h, const_k), const_l)` if
            // value happens to fit (high, low, k all in i16). 5 nodes,
            // covers ~30% of mid-range values.
            if let Some((high, low, k)) = fit_i64_via_shl(v) {
                let const_h = emit_node(nodes, seen, Node::const_i64(high))?;
                let const_k = emit_node(nodes, seen, Node::const_i64(k))?;
                let shifted = emit_node(nodes, seen, Node::shl(const_h, const_k))?;
                let const_l = emit_node(nodes, seen, Node::const_i64(low))?;
                return emit_node(nodes, seen, Node::add(shifted, const_l));
            }
            // Tier 2 — KASM v1.0 wave 3 : 16-bit-chunk OR-decomposition,
            // works for ANY i64. Splits the value into 4 chunks of 16 bits,
            // emits non-zero chunks shifted to position, OR-combines them.
            // Critical for Op::Comptime / load-time eval to work on hash
            // outputs and other arbitrary i64 values that don't fit the
            // compact (high, low, k) pattern. Cost : 5-22 nodes depending
            // on how many chunks are non-zero and need masking.
            materialize_i64_via_or_chain(v, nodes, seen)
        }
        _ => Err(KasmError::TypeMismatch { node: nodes.len() }),
    }
}

/// Build any i64 from 16-bit chunks combined via shl + bit_or.
///
/// Strategy : split v into 4 u16 chunks (low/mid_low/mid_high/high),
/// emit only non-zero ones. For chunks ≥ 0x8000 (i16 sign bit set),
/// `Node::const_i64(chunk as i16)` would sign-extend to a negative i64,
/// so we mask with `0xFFFF` before shifting into position.
///
/// The `0xFFFF` mask itself is built as `ShrI64(ConstI64(-1), ConstI64(48))`
/// — three nodes shared across all chunks that need masking.
///
/// This is the load-time materializer that makes Op::Comptime real for
/// arbitrary i64 outputs (Hash64(Const), large arithmetic results, etc.).
fn materialize_i64_via_or_chain(
    value: i64,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
) -> Result<u16, KasmError> {
    let v = value as u64;
    // Collect non-zero 16-bit chunks with their bit position.
    let mut chunks: Vec<(u32, u16)> = Vec::with_capacity(4);
    for shift in [0u32, 16, 32, 48] {
        let chunk = ((v >> shift) & 0xFFFF) as u16;
        if chunk != 0 {
            chunks.push((shift, chunk));
        }
    }
    debug_assert!(!chunks.is_empty(), "value 0 should have been handled by i16::try_from");

    // Mask 0xFFFF, lazy : built only if a chunk needs it (chunk ≥ 0x8000).
    let needs_mask = chunks.iter().any(|(_, c)| *c >= 0x8000);
    let mask_node = if needs_mask {
        let neg_one = emit_node(nodes, seen, Node::const_i64(-1))?;
        let const_48 = emit_node(nodes, seen, Node::const_i64(48))?;
        Some(emit_node(nodes, seen, Node::shr(neg_one, const_48))?)
    } else {
        None
    };

    // Build each chunk as a positioned i64 value.
    let mut accumulator: Option<u16> = None;
    for (shift, chunk) in chunks {
        // Emit the const for this chunk's value. If it sign-extends
        // (chunk has bit 15 set), mask with 0xFFFF to clear high bits.
        let chunk_const = emit_node(nodes, seen, Node::const_i64(chunk as i16))?;
        let chunk_clean = if chunk >= 0x8000 {
            emit_node(nodes, seen, Node::bit_and(chunk_const, mask_node.unwrap()))?
        } else {
            chunk_const
        };
        // Shift to its bit position. Skip the shift for the low chunk
        // (shift == 0). Shift amount fits in i16 (max 48).
        let chunk_positioned = if shift == 0 {
            chunk_clean
        } else {
            let const_shift = emit_node(nodes, seen, Node::const_i64(shift as i16))?;
            emit_node(nodes, seen, Node::shl(chunk_clean, const_shift))?
        };
        // OR into the running accumulator.
        accumulator = Some(match accumulator {
            None => chunk_positioned,
            Some(prev) => emit_node(nodes, seen, Node::bit_or(prev, chunk_positioned))?,
        });
    }
    Ok(accumulator.expect("non-zero value must have at least one chunk"))
}

/// Try to express `value` as `high * 2^k + low` with `high`, `low`,
/// `k` all in i16 range. Returns `Some((high, low, k))` on success
/// and `None` if no such decomposition exists.
///
/// Strategy: scan `k` from 1..=15 (15 fits in i16 and 2^15 = 32768
/// is the threshold past which `i16` overflow becomes total). For
/// each k, compute high = value / 2^k (truncating toward zero) and
/// low = value - high * 2^k; accept the first `k` for which both
/// fit in i16.
fn fit_i64_via_shl(value: i64) -> Option<(i16, i16, i16)> {
    for k in 1i16..=15 {
        let scale = 1i64 << k;
        let high = value / scale;
        let low = value - high.wrapping_mul(scale);
        if let (Ok(h), Ok(l)) = (i16::try_from(high), i16::try_from(low)) {
            return Some((h, l, k));
        }
    }
    None
}

fn materialize_bool(value: Known, nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>) -> Result<u16, KasmError> {
    match value {
        Known::Ref(index, Ty::Bool) => Ok(index),
        Known::Bool(value) => {
            let zero = emit_node(nodes, seen, Node::const_i64(0))?;
            let one = emit_node(nodes, seen, Node::const_i64(1))?;
            emit_node(nodes, seen, if value { Node::eq(zero, zero) } else { Node::eq(zero, one) })
        }
        _ => Err(KasmError::TypeMismatch { node: nodes.len() }),
    }
}

/// Φ.0 — Materialise an F64 `Known` into a node index whose result type
/// is `Ty::F64`. Constants that round-trip through `i16` are emitted as
/// a single `ConstF64`; values outside that range fall back to building
/// the constant through `ConstI64 + F64Op::FromI64` (still bounded by
/// the i16 range of `ConstI64`, which is acceptable for Φ.0 — fancier
/// constants are a synthesizer concern).
fn materialize_f64(
    value: Known,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
) -> Result<u16, KasmError> {
    match value {
        Known::Ref(index, Ty::F64) => Ok(index),
        Known::F64(bits) => {
            let v = f64::from_bits(bits as u64);
            // Fast path: integer-valued constants in `i16` range. Cover
            // the bulk of synthesised F64 constants (0.0, 1.0, 2.0, etc.)
            // without ever leaving the bit-cast domain.
            if v.is_finite() && v.fract() == 0.0 {
                if let Ok(small) = i16::try_from(v as i64) {
                    if (small as f64) == v {
                        return emit_node(nodes, seen, Node::const_f64(small));
                    }
                }
            }
            // Slow path (rare): build through I64→F64 conversion. Only
            // works if the value's i64 truncation fits in i16 and round-
            // trips cleanly. Otherwise we report a type mismatch — the
            // synthesizer will then construct a multi-node combinator.
            if v.is_finite() {
                let truncated = v as i64;
                if let Ok(small) = i16::try_from(truncated) {
                    if (small as f64) == v {
                        let const_i = emit_node(nodes, seen, Node::const_i64(small))?;
                        return emit_node(nodes, seen, Node::f64_from_i64(const_i));
                    }
                }
            }
            Err(KasmError::TypeMismatch { node: nodes.len() })
        }
        _ => Err(KasmError::TypeMismatch { node: nodes.len() }),
    }
}

/// Φ.0 — Simplify a single `F64Op` node. The optimiser folds when both
/// operands are statically `Known::F64` (or `Known::I64` for the
/// `FromI64` conversion); otherwise it materialises the operands and
/// emits the original op. Folding uses the **exact same bit-cast
/// arithmetic** as `interpreter::exec_f64_op` — both call sites must
/// agree byte-for-byte for `static_output` to remain a sound shortcut.
fn simplify_f64_op(
    node: Node,
    values: &[Known],
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
    _index: usize,
) -> Result<Known, KasmError> {
    let sub = F64SubOp::from_imm(node.imm)?;
    let a_val = values[node.a as usize];

    // Try constant folding first.
    let folded = fold_f64_op(sub, a_val, values, node.b);
    if let Some(known) = folded {
        return Ok(known);
    }

    // Materialise and emit.
    let a_ref = match sub.a_ty() {
        Ty::I64 => materialize_i64(a_val, nodes, seen)?,
        Ty::F64 => materialize_f64(a_val, nodes, seen)?,
        // F64SubOp::a_ty() never returns Bool or VecI64 — F64 ops
        // operate on i64/f64 inputs only. These arms exist solely
        // to satisfy match exhaustiveness ; reaching them is a
        // bug in F64SubOp::a_ty().
        Ty::Bool | Ty::VecI64 => unreachable!(
            "F64SubOp::a_ty() returned non-numeric type — invariant broken"
        ),
    };
    let new_node = if sub.is_binary() {
        let b_val = values[node.b as usize];
        let b_ref = materialize_f64(b_val, nodes, seen)?;
        Node {
            op: Op::F64Op,
            ty: sub.result_ty(),
            a: a_ref,
            b: b_ref,
            imm: sub.imm(),
        }
    } else {
        Node {
            op: Op::F64Op,
            ty: sub.result_ty(),
            a: a_ref,
            b: 0,
            imm: sub.imm(),
        }
    };
    let idx = emit_node(nodes, seen, new_node)?;
    Ok(Known::Ref(idx, sub.result_ty()))
}

fn fold_f64_op(sub: F64SubOp, a: Known, values: &[Known], b_idx: u16) -> Option<Known> {
    let to_bits = |v: f64| v.to_bits() as i64;
    let a_f = match (sub.a_ty(), a) {
        (Ty::F64, Known::F64(b)) => Some(f64::from_bits(b as u64)),
        (Ty::I64, Known::I64(v)) => Some(v as f64),
        _ => None,
    };
    if sub.is_binary() {
        let a = a_f?;
        let b = match values[b_idx as usize] {
            Known::F64(b) => f64::from_bits(b as u64),
            _ => return None,
        };
        let r = match sub {
            F64SubOp::Add => a + b,
            F64SubOp::Sub => a - b,
            F64SubOp::Mul => a * b,
            F64SubOp::DivChecked => {
                let r = a / b;
                if r.is_finite() {
                    r
                } else {
                    0.0
                }
            }
            F64SubOp::Min => {
                let r = a.min(b);
                if r.is_nan() {
                    0.0
                } else {
                    r
                }
            }
            F64SubOp::Max => {
                let r = a.max(b);
                if r.is_nan() {
                    0.0
                } else {
                    r
                }
            }
            _ => unreachable!("non-binary sub-op in binary fold path"),
        };
        return Some(Known::F64(to_bits(r)));
    }
    let a = a_f?;
    let folded = match sub {
        F64SubOp::Sqrt => {
            let r = a.sqrt();
            if r.is_finite() {
                r
            } else {
                0.0
            }
        }
        F64SubOp::Abs => a.abs(),
        F64SubOp::Neg => -a,
        F64SubOp::FromI64 => a, // a was already promoted I64→f64 above
        F64SubOp::ToI64 => {
            let r = if a.is_finite() {
                if a >= i64::MAX as f64 {
                    i64::MAX
                } else if a <= i64::MIN as f64 {
                    i64::MIN
                } else {
                    a as i64
                }
            } else {
                0
            };
            return Some(Known::I64(r));
        }
        F64SubOp::Exp => {
            let r = a.exp();
            if r.is_finite() {
                r
            } else {
                0.0
            }
        }
        F64SubOp::Ln => {
            let av = a.abs();
            let r = if av == 0.0 { 0.0 } else { av.ln() };
            if r.is_finite() {
                r
            } else {
                0.0
            }
        }
        _ => return None,
    };
    Some(Known::F64(to_bits(folded)))
}

fn emit_node(nodes: &mut Vec<Node>, seen: &mut HashMap<Node, u16>, node: Node) -> Result<u16, KasmError> {
    if let Some(index) = seen.get(&node).copied() {
        return Ok(index);
    }
    let index = u16::try_from(nodes.len()).map_err(|_| KasmError::BadNodeCount(nodes.len()))?;
    nodes.push(node);
    seen.insert(node, index);
    Ok(index)
}

pub fn semantic_fingerprint(program: &Program) -> Result<[u8; 32], KasmError> {
    if program.target().needs_external_backend() {
        return Err(KasmError::ExternalTarget(program.target()));
    }

    // Φ.ν.7e — α-normalisation des slots Input par ordre de première
    // occurrence dans le DAG canonical. Deux programmes qui ne diffèrent
    // que par la numérotation des slots collapsent au même fingerprint.
    let canonical = alpha_renumber_inputs(&program.canonical()?)?;
    let mut h = Sha256::new();
    h.update(b"kasm-semantic-fingerprint-v1\0");
    h.update([canonical.inputs(), canonical.outputs()]);
    for ty in canonical.output_types() {
        h.update([ty as u8]);
    }

    for sample in 0..16u8 {
        let args = semantic_sample_args(canonical.inputs(), sample);
        h.update([sample]);
        h.update(&(args.len() as u16).to_le_bytes());
        h.update(&args);
        let result = execute(&canonical, &args)?;
        h.update(&(result.len() as u16).to_le_bytes());
        h.update(&result);
    }

    Ok(h.finalize().into())
}

/// Φ.ν.7e — Renumérote les `Op::Input(imm = old_slot)` du programme par
/// ordre de **première occurrence** dans la séquence topologique. Le
/// nombre déclaré d'inputs devient le nombre de slots effectivement
/// utilisés. Idempotent sur un programme déjà α-normalisé. Préserve
/// strictement la sémantique sur les inputs renumérotés.
fn alpha_renumber_inputs(canonical: &Program) -> Result<Program, KasmError> {
    let mut slot_map: HashMap<u8, u8> = HashMap::new();
    let mut next_slot: u8 = 0;
    let mut new_nodes = Vec::with_capacity(canonical.nodes().len());
    for node in canonical.nodes() {
        if node.op == Op::Input {
            let old = node.imm as u8;
            let new = *slot_map.entry(old).or_insert_with(|| {
                let s = next_slot;
                next_slot = next_slot.saturating_add(1);
                s
            });
            new_nodes.push(Node { imm: new as i16, ..*node });
        } else {
            new_nodes.push(*node);
        }
    }
    // If no Input nodes (pure-const program), preserve original count
    // (Program::new may require ≥ 1). Otherwise compress to used count.
    let effective_inputs = if next_slot == 0 { canonical.inputs() } else { next_slot };
    Program::new(
        canonical.target(),
        effective_inputs,
        canonical.outputs(),
        canonical.fuel(),
        new_nodes,
    )
}

fn semantic_sample_args(inputs: u8, sample: u8) -> Vec<u8> {
    let base = [-8i64, -3, -1, 0, 1, 2, 3, 5, 8, 13, 21, 34, -13, -21, 55, -55];
    let mut out = Vec::with_capacity(inputs as usize * 8);
    for slot in 0..inputs {
        let value = base[sample as usize].wrapping_add((slot as i64).wrapping_mul(17));
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn canonical_node(
    program: &Program,
    old_index: usize,
    nodes: &mut Vec<Node>,
    seen: &mut HashMap<Node, u16>,
    old_to_new: &mut [Option<u16>],
    fingerprints: &mut HashMap<usize, Vec<u8>>,
) -> Result<u16, KasmError> {
    if let Some(index) = old_to_new[old_index] {
        return Ok(index);
    }

    let old = program.nodes()[old_index];
    let node = match old.op {
        Op::Input | Op::ConstI64 | Op::ConstF64 => old,
        Op::F64Op => {
            // Φ.0 — F64Op canonical form: recurse into `a`, recurse
            // into `b` only when the sub-op is binary (verified
            // upstream), keep `imm` selector unchanged. F64 operations
            // do **not** participate in commutative reordering even
            // though Add/Mul/Min/Max are mathematically commutative —
            // IEEE 754 floating-point addition is non-associative, so
            // re-ordering would silently break bit-exact reproducibility.
            let sub = F64SubOp::from_imm(old.imm)?;
            let a = canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?;
            let b = if sub.is_binary() {
                canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?
            } else {
                0
            };
            Node { a, b, ..old }
        }
        Op::Hash64
        | Op::NotBool
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64
        | Op::Lazy
        | Op::Force => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            ..old
        },
        Op::Output => return canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints),
        Op::AddI64
        | Op::MulI64
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::SatAddI64 => {
            let a_fp = subgraph_fingerprint(program, old.a as usize, fingerprints)?;
            let b_fp = subgraph_fingerprint(program, old.b as usize, fingerprints)?;
            let (old_a, old_b) = if a_fp <= b_fp { (old.a, old.b) } else { (old.b, old.a) };
            let a = canonical_node(program, old_a as usize, nodes, seen, old_to_new, fingerprints)?;
            let b = canonical_node(program, old_b as usize, nodes, seen, old_to_new, fingerprints)?;
            Node { a, b, ..old }
        }
        Op::SubI64
        | Op::DivI64Checked
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64
        | Op::LtI64
        | Op::LeI64 => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            b: canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?,
            ..old
        },
        Op::SelectI64 | Op::ClampI64 => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            b: canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?,
            imm: canonical_node(
                program,
                checked_imm_ref(old.imm, old_index)? as usize,
                nodes,
                seen,
                old_to_new,
                fingerprints,
            )? as i16,
            ..old
        },
        // Reduce ops are screened off by `canonicalize`'s `has_range_op`
        // guard; reaching this arm is a logic error.
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            return Err(KasmError::BadReduceCount {
                node: old_index,
                count: old.imm,
            });
        }
        // KASM v1.0 — recurse into a (and b for binary forms) but keep
        // imm and op shape. canonicalize doesn't perform v1.0-specific
        // commutative reordering — these ops are opaque to it.
        Op::Adaptive | Op::Comptime | Op::Memoize | Op::Grad
        | Op::Vmap | Op::Pmap | Op::VLenI64 | Op::VSumI64 | Op::VRangeI64
        | Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            ..old
        },
        Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VConcatI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VGetI64 => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            b: canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?,
            ..old
        },
        Op::Cond => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            b: canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?,
            imm: canonical_node(
                program, old.imm as usize, nodes, seen, old_to_new, fingerprints,
            )? as i16,
            ..old
        },
        Op::Pipeline | Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::Fractal | Op::Eval  // Wave 8 — opaque to canonicalizer
        => Node {
            a: canonical_node(program, old.a as usize, nodes, seen, old_to_new, fingerprints)?,
            b: canonical_node(program, old.b as usize, nodes, seen, old_to_new, fingerprints)?,
            ..old
        },
    };

    if let Some(index) = seen.get(&node).copied() {
        old_to_new[old_index] = Some(index);
        return Ok(index);
    }

    let new_index = u16::try_from(nodes.len()).map_err(|_| KasmError::BadNodeCount(nodes.len()))?;
    nodes.push(node);
    seen.insert(node, new_index);
    old_to_new[old_index] = Some(new_index);
    Ok(new_index)
}

fn subgraph_fingerprint(
    program: &Program,
    index: usize,
    memo: &mut HashMap<usize, Vec<u8>>,
) -> Result<Vec<u8>, KasmError> {
    if let Some(bytes) = memo.get(&index) {
        return Ok(bytes.clone());
    }

    let node = program.nodes()[index];
    let mut out = vec![node.op as u8, node.ty as u8];
    match node.op {
        Op::Input | Op::ConstI64 | Op::ConstF64 => {
            out.extend_from_slice(&node.imm.to_le_bytes());
        }
        Op::Lazy | Op::Force => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
        }
        Op::F64Op => {
            // Sub-op selector is part of the structural identity.
            out.extend_from_slice(&node.imm.to_le_bytes());
            let sub = F64SubOp::from_imm(node.imm)?;
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            if sub.is_binary() {
                out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
            }
        }
        Op::Hash64
        | Op::NotBool
        | Op::Output
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64 => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
        }
        Op::AddI64
        | Op::MulI64
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::SatAddI64 => {
            let mut a = subgraph_fingerprint(program, node.a as usize, memo)?;
            let mut b = subgraph_fingerprint(program, node.b as usize, memo)?;
            if b < a {
                std::mem::swap(&mut a, &mut b);
            }
            out.extend_from_slice(&a);
            out.extend_from_slice(&b);
        }
        Op::SubI64
        | Op::DivI64Checked
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64
        | Op::LtI64
        | Op::LeI64 => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
        }
        Op::SelectI64 | Op::ClampI64 => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(
                program,
                checked_imm_ref(node.imm, index)? as usize,
                memo,
            )?);
        }
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            // Same screening as canonicalize / simplify above.
            return Err(KasmError::BadReduceCount {
                node: index,
                count: node.imm,
            });
        }
        // KASM v1.0 — fingerprint includes imm + recursive subgraph
        // fingerprints of the referenced slots. v1.0 ops are NOT
        // commutative (Cond, Pipeline, Vmap, etc. — order matters), so
        // no swap.
        Op::Adaptive | Op::Comptime | Op::Memoize | Op::Grad
        | Op::Vmap | Op::Pmap | Op::VLenI64 | Op::VSumI64 | Op::VRangeI64
        | Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
            out.extend_from_slice(&node.imm.to_le_bytes());
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
        }
        Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VConcatI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VGetI64 => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
        }
        Op::Cond => {
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(
                program,
                checked_imm_ref(node.imm, index)? as usize,
                memo,
            )?);
        }
        Op::Pipeline | Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::Fractal | Op::Eval  // Wave 8 — fingerprint via a/b refs + imm
        => {
            out.extend_from_slice(&node.imm.to_le_bytes());
            out.extend_from_slice(&subgraph_fingerprint(program, node.a as usize, memo)?);
            out.extend_from_slice(&subgraph_fingerprint(program, node.b as usize, memo)?);
        }
    }

    let digest = digest(&out).to_vec();
    memo.insert(index, digest.clone());
    Ok(digest)
}

pub fn static_output(program: &Program) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for node in program.nodes() {
        if node.op != Op::Output {
            if node.op != Op::ConstI64 && node.op != Op::ConstF64 {
                return None;
            }
            continue;
        }

        let source = *program.nodes().get(node.a as usize)?;
        match (source.op, node.ty) {
            (Op::ConstI64, Ty::I64) => out.extend_from_slice(&(source.imm as i64).to_le_bytes()),
            (Op::ConstF64, Ty::F64) => {
                let bits = (source.imm as f64).to_bits();
                out.extend_from_slice(&(bits as i64).to_le_bytes());
            }
            _ => return None,
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

// ─── Semantic CSE ────────────────────────────────────────────────────
//
// Structural CSE (emit_node / canonical_node `seen` HashMap) catches
// syntactically identical subexpressions. Semantic CSE goes further:
// it evaluates the program on deterministic sample inputs, records
// intermediate values at every node, and merges nodes that produce
// identical traces — even when their structure differs.
//
// Example: `Shl(x, 1)`, `Add(x, x)`, `Mul(x, 2)` are structurally
// distinct but semantically equivalent. All three produce the same i64
// on every sample → the CSE pass keeps the first and redirects the
// others, then canonicalize prunes the dead nodes.
//
// Collision safety: 8 diverse i64 samples × 64-bit values = 512-bit
// fingerprint per node. Probability of false positive ≈ 2^-512 for
// non-degenerate functions.

const CSE_SAMPLES: usize = 8;

/// Trace-evaluate a scalar I64/Bool program, returning the i64 value at
/// each node position. Bool values map to 0/1, ConstF64 to bit pattern.
/// Returns `None` if the program contains ops the trace evaluator cannot
/// handle (F64Op, Vec, Reduce, meta-ops that need external dispatch).
fn trace_eval_i64(program: &Program, args: &[u8]) -> Option<Vec<i64>> {
    let n_inputs = program.inputs() as usize;
    if n_inputs * 8 != args.len() { return None; }

    let input_scalars: Vec<i64> = (0..n_inputs)
        .map(|i| i64::from_le_bytes(args[i * 8..(i + 1) * 8].try_into().unwrap()))
        .collect();

    let nodes = program.nodes();
    let mut vals: Vec<i64> = Vec::with_capacity(nodes.len());

    for node in nodes {
        let v = match node.op {
            Op::Input => *input_scalars.get(node.imm as usize)?,
            Op::ConstI64 => node.imm as i64,
            Op::ConstF64 => (node.imm as f64).to_bits() as i64,
            Op::AddI64 => vals[node.a as usize].wrapping_add(vals[node.b as usize]),
            Op::MulI64 => vals[node.a as usize].wrapping_mul(vals[node.b as usize]),
            Op::SubI64 => vals[node.a as usize].wrapping_sub(vals[node.b as usize]),
            Op::DivI64Checked => {
                let b = vals[node.b as usize];
                if b == 0 { 0 } else { vals[node.a as usize].wrapping_div(b) }
            }
            Op::ModI64Checked => {
                let b = vals[node.b as usize];
                if b == 0 { 0 } else { vals[node.a as usize].wrapping_rem(b) }
            }
            Op::MinI64 => vals[node.a as usize].min(vals[node.b as usize]),
            Op::MaxI64 => vals[node.a as usize].max(vals[node.b as usize]),
            Op::EqI64 => if vals[node.a as usize] == vals[node.b as usize] { 1 } else { 0 },
            Op::LtI64 => if vals[node.a as usize] < vals[node.b as usize] { 1 } else { 0 },
            Op::LeI64 => if vals[node.a as usize] <= vals[node.b as usize] { 1 } else { 0 },
            Op::Hash64 => hash_i64(vals[node.a as usize]),
            Op::BitAndI64 => vals[node.a as usize] & vals[node.b as usize],
            Op::BitOrI64 => vals[node.a as usize] | vals[node.b as usize],
            Op::BitXorI64 => vals[node.a as usize] ^ vals[node.b as usize],
            Op::ShlI64 => {
                let shift = (vals[node.b as usize] as u64) & 63;
                ((vals[node.a as usize] as u64).wrapping_shl(shift as u32)) as i64
            }
            Op::ShrI64 => {
                let shift = (vals[node.b as usize] as u64) & 63;
                ((vals[node.a as usize] as u64).wrapping_shr(shift as u32)) as i64
            }
            Op::SatAddI64 => vals[node.a as usize].saturating_add(vals[node.b as usize]),
            Op::SatSubI64 => vals[node.a as usize].saturating_sub(vals[node.b as usize]),
            Op::ClampI64 => {
                let v = vals[node.a as usize];
                let lo = vals[node.b as usize];
                let hi = vals[node.imm as usize];
                v.max(lo).min(hi)
            }
            Op::SelectI64 => {
                if vals[node.a as usize] != 0 {
                    vals[node.b as usize]
                } else {
                    vals[node.imm as usize]
                }
            }
            Op::Cond => {
                if vals[node.a as usize] != 0 {
                    vals[node.b as usize]
                } else {
                    vals[node.imm as usize]
                }
            }
            Op::AndBool => {
                if vals[node.a as usize] != 0 && vals[node.b as usize] != 0 { 1 } else { 0 }
            }
            Op::OrBool => {
                if vals[node.a as usize] != 0 || vals[node.b as usize] != 0 { 1 } else { 0 }
            }
            Op::NotBool => if vals[node.a as usize] == 0 { 1 } else { 0 },
            Op::BitFlipI64 => !vals[node.a as usize],
            Op::NegI64 => vals[node.a as usize].wrapping_neg(),
            Op::ReverseBitsI64 => vals[node.a as usize].reverse_bits(),
            Op::ByteswapI64 => vals[node.a as usize].swap_bytes(),
            Op::PopcntI64 => crate::cpu_bits::popcount_u64(vals[node.a as usize] as u64) as i64,
            Op::LzcntI64 => crate::cpu_bits::leading_zeros_u64(vals[node.a as usize] as u64) as i64,
            Op::TzcntI64 => crate::cpu_bits::trailing_zeros_u64(vals[node.a as usize] as u64) as i64,
            Op::PextI64 => crate::cpu_bits::pext_u64(
                vals[node.a as usize] as u64,
                vals[node.b as usize] as u64,
            ) as i64,
            Op::PdepI64 => crate::cpu_bits::pdep_u64(
                vals[node.a as usize] as u64,
                vals[node.b as usize] as u64,
            ) as i64,
            Op::Output => vals[node.a as usize],
            // Pass-through wrappers (folded by simplify, but handle
            // defensively in case they survive).
            Op::Comptime | Op::Memoize | Op::Adaptive => vals[node.a as usize],
            // Ops that need external dispatch or vector semantics —
            // bail, the structural CSE from simplify is all we can do.
            _ => return None,
        };
        vals.push(v);
    }
    Some(vals)
}

/// Semantic Common Subexpression Elimination.
///
/// Runs `simplify` first (structural CSE + constant folding), then detects
/// semantically equivalent subexpressions by tracing execution on 8
/// deterministic sample inputs. Nodes that produce identical value traces
/// are merged — the first occurrence survives, later duplicates are
/// redirected. A final `canonicalize` prunes dead nodes.
///
/// Falls back to `simplify` unchanged for programs with F64Op, Vec, Reduce,
/// or meta-ops that the trace evaluator cannot handle.
pub fn cse(program: &Program) -> Result<Program, KasmError> {
    let simplified = simplify(program)?;
    let nodes = simplified.nodes();
    let n = nodes.len();

    // Trivial programs — no CSE opportunity.
    if n <= 3 {
        return Ok(simplified);
    }

    // Phase 1: trace on CSE_SAMPLES deterministic inputs.
    let mut traces: Vec<[i64; CSE_SAMPLES]> = vec![[0; CSE_SAMPLES]; n];
    for s in 0..CSE_SAMPLES {
        let args = semantic_sample_args(simplified.inputs(), s as u8);
        let vals = match trace_eval_i64(&simplified, &args) {
            Some(v) => v,
            None => return Ok(simplified), // unsupported ops, bail
        };
        for i in 0..n {
            traces[i][s] = vals[i];
        }
    }

    // Phase 2: find semantic equivalences.
    // Key = (value trace, type). Representative = first occurrence.
    let mut representatives: HashMap<([i64; CSE_SAMPLES], Ty), u16> = HashMap::new();
    let mut redirect: Vec<u16> = (0..n as u16).collect();
    let mut eliminated = 0usize;

    for (i, node) in nodes.iter().enumerate() {
        // Don't merge Input or Output nodes.
        if node.op == Op::Input || node.op == Op::Output {
            continue;
        }
        // Φ.ν.7g — Skip branch-sensitive ops dans la dedupe par trace.
        //
        // Trace-equivalence est nécessaire mais PAS suffisante pour les
        // ops dont la sortie dépend d'une comparaison entre valeurs
        // (Min, Max, SelectI64, ClampI64, Cond). Exemple du bug
        // (reproduit session 2026-05-03 sur recognize_clamp_affine_program) :
        //
        //   Programme : `min(max(7x+13, -120), 180)`  (clamp valide)
        //   Samples   : 8 inputs où 7x+13 ∈ [-100, 100]
        //   Résultat  : ni le max ni le min ne fire sur ces samples
        //              → trace de `max(7x+13, -120)` == trace de `7x+13`
        //              → trace de `min(_, 180)` == trace de l'input
        //              → CSE les dedupe → clamp silencieusement supprimé
        //              → programme produit juste `7x+13` en production
        //              → bug observable sur inputs extrêmes
        //
        // Fix : ces ops gardent leur identité même si leur trace matche
        // un autre node. Coût minime (peu d'ops branch-sensitive en
        // pratique). CSE continue d'agir agressivement sur les ops
        // arithmétiques pures (Add, Mul, Shl, etc.) où trace-equivalence
        // EST suffisante (mêmes inputs → mêmes outputs partout).
        if matches!(
            node.op,
            Op::MinI64 | Op::MaxI64 | Op::SelectI64 | Op::ClampI64 | Op::Cond
        ) {
            continue;
        }
        let key = (traces[i], node.ty);
        if let Some(&repr) = representatives.get(&key) {
            redirect[i] = repr;
            eliminated += 1;
        } else {
            representatives.insert(key, i as u16);
        }
    }

    if eliminated == 0 {
        return Ok(simplified);
    }

    // Phase 3: redirect references, then canonicalize to prune dead nodes.
    let mut new_nodes: Vec<Node> = Vec::with_capacity(n);
    for node in nodes {
        let mut nn = *node;
        nn.a = redirect[nn.a as usize];
        nn.b = redirect[nn.b as usize];
        if matches!(nn.op, Op::SelectI64 | Op::ClampI64 | Op::Cond) {
            nn.imm = redirect[nn.imm as usize] as i16;
        }
        new_nodes.push(nn);
    }

    let redirected = Program::new(
        simplified.target(),
        simplified.inputs(),
        simplified.outputs(),
        new_nodes.len() as u32,
        new_nodes,
    )?;
    canonicalize(&redirected)
}
