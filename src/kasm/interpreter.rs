//! KASM interpreter and program composition.

use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::program::{checked_imm_ref, hash_i64, Program};
use super::types::{F64SubOp, KasmError, Node, Op, Ty, MAX_NODES};

/// Wave 8 (FULL) — Trait dispatcher pour Op::Fractal et Op::Eval.
///
/// Quand le bytecode interpreter rencontre Op::Fractal/Op::Eval, il
/// délègue au dispatcher fourni à l'appel `execute_with_fractal`.
/// Sans dispatcher, ces opcodes restent fail-loud (rétro-compat
/// avec les call sites historiques `execute(prog, args)`).
///
/// Encoding Wave 8 :
///   - Op::Fractal(callee_id_slot, arg_slot) : a → i64 callee_id,
///     b → i64 argument. Le dispatcher mappe `callee_id` vers un
///     programme KASM concret (callee table dans SelfHostingRuntime).
///   - Op::Eval(eval_id_slot, arg_slot) : pareil, dispatcher mappe
///     `eval_id` vers des bytes KASM à interpréter inline.
///
/// Output : i64 unique (single-output programs).
pub trait FractalDispatcher: Send + Sync {
    /// Résoudre Op::Fractal(callee_id, arg) → résultat i64.
    fn fractal(&self, callee_id: i64, arg: i64) -> Result<i64, KasmError>;
    /// Résoudre Op::Eval(eval_id, arg) → résultat i64.
    fn eval(&self, eval_id: i64, arg: i64) -> Result<i64, KasmError>;
}

/// Σ.1 + Wave 7b — the interpreter's per-node value representation.
///
/// Three variants, all `Copy` for fast slot-to-slot transit :
///   - `I64(i64)`   — i64 bit pattern (also carries f64 via `to_bits`)
///   - `Bool(bool)` — boolean value
///   - `VecI64(u32)` — handle into the per-`execute()` `vec_pool`,
///     resolved on demand. Keeping `Value` as `Copy` preserves the
///     Σ.1 unsafe fast-read helpers without a `Clone` tax.
#[derive(Clone, Copy, Debug)]
pub(super) enum Value {
    I64(i64),
    Bool(bool),
    VecI64(u32),
}

/// Wave 7b wire format helpers — Vec input/output bytes layout :
/// `[u32 LE count][count × 8 bytes i64 LE]`.
///
/// Backward compatible : a program with no Vec inputs sees the same
/// flat `inputs() * 8` bytes args layout as before. Vec slots add
/// the 4-byte count prefix.
mod vec_wire {
    use super::KasmError;

    /// Decode a length-prefixed `Vec<i64>` from `bytes` starting at
    /// `cursor`. Returns the parsed `Vec<i64>` and the new cursor.
    pub(super) fn read_vec(bytes: &[u8], cursor: usize) -> Result<(Vec<i64>, usize), KasmError> {
        if cursor + 4 > bytes.len() {
            return Err(KasmError::BadInputLength {
                expected: cursor + 4,
                got: bytes.len(),
            });
        }
        let count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        let payload_start = cursor + 4;
        let payload_end = payload_start + count * 8;
        if payload_end > bytes.len() {
            return Err(KasmError::BadInputLength {
                expected: payload_end,
                got: bytes.len(),
            });
        }
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let off = payload_start + i * 8;
            out.push(i64::from_le_bytes(bytes[off..off + 8].try_into().unwrap()));
        }
        Ok((out, payload_end))
    }

    /// Encode a `Vec<i64>` into the wire format, appending to `out`.
    pub(super) fn write_vec(out: &mut Vec<u8>, vec: &[i64]) {
        let count = vec.len() as u32;
        out.extend_from_slice(&count.to_le_bytes());
        for v in vec {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

pub fn execute(program: &Program, args: &[u8]) -> Result<Vec<u8>, KasmError> {
    execute_inner(program, args, None)
}

pub(crate) fn future_key_i64(program: &Program, args: &[u8], child_node_id: u16) -> ([u8; 32], i64) {
    let input_hash = Sha256::digest(args);
    let mut h = Sha256::new();
    h.update(b"KASM:FUTURE:v1");
    h.update(program.bytes());
    h.update(input_hash);
    h.update(child_node_id.to_le_bytes());
    let digest = h.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    let mut low = [0u8; 8];
    low.copy_from_slice(&key[..8]);
    (key, i64::from_le_bytes(low))
}

/// Stack-only fast path for the auto-router v2 hot path. Skips ALL
/// the per-call Vec allocations (input_scalars, vec_pool,
/// slot_vec_handle, values, out) of the general `execute` and runs
/// the program in a fixed `[i64; 64]` stack array.
///
/// Returns `None` (forcing fallback to the general interpreter) when :
///   - program has more than 1 input or 1 output
///   - program has more than 64 nodes (stack overflow risk)
///   - program contains an opcode this fast path doesn't handle
///   - program targets a non-CPU backend
///
/// Correctness : the supported opcodes are bit-exact equivalent to
/// `execute_inner`. Bool values are encoded as i64 0/1 ; Cond reads
/// the predicate with `pred != 0` matching the `Value::I64(n) =>
/// *n != 0` branch in the general interpreter.
///
/// Mesuré : ~30-100 ns/call vs ~500-1100 ns via `execute` (5-10x
/// faster on small programs). Voir DNA bench (cb583ac, 2dc169c).
pub fn try_execute_i64_inline(program: &Program, arg: i64) -> Option<i64> {
    if program.inputs() != 1 || program.outputs() != 1 {
        return None;
    }
    let target = program.target();
    if !matches!(target, super::types::Target::Cpu | super::types::Target::Auto) {
        return None;
    }
    let nodes = program.nodes();
    if nodes.len() > 64 {
        return None;
    }

    let mut stack = [0i64; 64];
    let mut future_child = [0u16; 64];
    let mut future_seen = [false; 64];
    let arg_bytes = arg.to_le_bytes();

    for (i, node) in nodes.iter().copied().enumerate() {
        let v: i64 = match node.op {
            Op::Input => {
                if node.imm != 0 {
                    return None;
                }
                arg
            }
            Op::ConstI64 => node.imm as i64,
            Op::Output => {
                return Some(stack[node.a as usize]);
            }
            Op::AddI64 => stack[node.a as usize].wrapping_add(stack[node.b as usize]),
            Op::SubI64 => stack[node.a as usize].wrapping_sub(stack[node.b as usize]),
            Op::MulI64 => stack[node.a as usize].wrapping_mul(stack[node.b as usize]),
            Op::Hash64 => hash_i64(stack[node.a as usize]),
            Op::MinI64 => stack[node.a as usize].min(stack[node.b as usize]),
            Op::MaxI64 => stack[node.a as usize].max(stack[node.b as usize]),
            Op::BitAndI64 => stack[node.a as usize] & stack[node.b as usize],
            Op::BitOrI64 => stack[node.a as usize] | stack[node.b as usize],
            Op::BitXorI64 => stack[node.a as usize] ^ stack[node.b as usize],
            Op::BitFlipI64 => !stack[node.a as usize],
            Op::NegI64 => stack[node.a as usize].wrapping_neg(),
            Op::ReverseBitsI64 => stack[node.a as usize].reverse_bits(),
            Op::ByteswapI64 => stack[node.a as usize].swap_bytes(),
            Op::PopcntI64 => crate::cpu_bits::popcount_u64(stack[node.a as usize] as u64) as i64,
            Op::LzcntI64 => crate::cpu_bits::leading_zeros_u64(stack[node.a as usize] as u64) as i64,
            Op::TzcntI64 => crate::cpu_bits::trailing_zeros_u64(stack[node.a as usize] as u64) as i64,
            Op::PextI64 => crate::cpu_bits::pext_u64(
                stack[node.a as usize] as u64,
                stack[node.b as usize] as u64,
            ) as i64,
            Op::PdepI64 => crate::cpu_bits::pdep_u64(
                stack[node.a as usize] as u64,
                stack[node.b as usize] as u64,
            ) as i64,
            Op::Lazy => {
                let (_, future) = future_key_i64(program, &arg_bytes, node.a);
                future_child[i] = node.a;
                future_seen[i] = true;
                future
            }
            Op::Force => {
                let future_idx = node.a as usize;
                if future_idx >= i || !future_seen[future_idx] {
                    return None;
                }
                stack[future_child[future_idx] as usize]
            }
            Op::ShlI64 => {
                let value = stack[node.a as usize];
                let s = (stack[node.b as usize] as u64) & 63;
                ((value as u64).wrapping_shl(s as u32)) as i64
            }
            Op::ShrI64 => {
                let value = stack[node.a as usize];
                let s = (stack[node.b as usize] as u64) & 63;
                ((value as u64).wrapping_shr(s as u32)) as i64
            }
            Op::LtI64 => {
                if stack[node.a as usize] < stack[node.b as usize] {
                    1
                } else {
                    0
                }
            }
            Op::LeI64 => {
                if stack[node.a as usize] <= stack[node.b as usize] {
                    1
                } else {
                    0
                }
            }
            Op::EqI64 => {
                if stack[node.a as usize] == stack[node.b as usize] {
                    1
                } else {
                    0
                }
            }
            Op::Cond => {
                // pred slot in `node.a`, then in `node.b`, else in `node.imm`.
                // Compatible with the general interpreter's pred handling
                // (Value::I64(n) => *n != 0). `node.imm as u16` recovers
                // the original else_slot index even for slots > i16::MAX.
                let pred = stack[node.a as usize] != 0;
                let chosen_idx = if pred {
                    node.b as usize
                } else {
                    (node.imm as u16) as usize
                };
                stack[chosen_idx]
            }
            // Tout autre opcode (F64, Vec, meta-ops Wave 8, etc.) →
            // fallback à l'interpréteur général via None.
            _ => return None,
        };
        stack[i] = v;
    }
    // Programme sans Output node — invalide, fallback safe.
    None
}

/// Wave 8 FULL — exécute un programme KASM en présence d'un
/// `FractalDispatcher` qui résout les Op::Fractal et Op::Eval.
/// Programmes sans ces opcodes : comportement identique à `execute`.
pub fn execute_with_fractal(
    program: &Program,
    args: &[u8],
    dispatcher: &dyn FractalDispatcher,
) -> Result<Vec<u8>, KasmError> {
    execute_inner(program, args, Some(dispatcher))
}

fn execute_inner(
    program: &Program,
    args: &[u8],
    dispatcher: Option<&dyn FractalDispatcher>,
) -> Result<Vec<u8>, KasmError> {
    // Wave 7b — parse args per-slot based on declared input types.
    // Scalar slots (I64/F64) consume 8 bytes; VecI64 slots consume
    // [u32 LE count | count × 8 bytes] (length-prefixed). Backward
    // compatible : a program with no Vec inputs sees the same flat
    // `inputs() * 8` bytes layout as before.
    let input_types = program.input_types();
    let mut input_scalars: Vec<i64> = Vec::with_capacity(program.inputs() as usize);
    let mut vec_pool: Vec<Arc<[i64]>> = Vec::new();
    let mut slot_vec_handle: Vec<Option<u32>> = vec![None; program.inputs() as usize];
    {
        let mut cursor = 0usize;
        for (slot, ty) in input_types.iter().enumerate() {
            match ty {
                Ty::I64 | Ty::F64 => {
                    if cursor + 8 > args.len() {
                        return Err(KasmError::BadInputLength {
                            expected: cursor + 8,
                            got: args.len(),
                        });
                    }
                    input_scalars
                        .push(i64::from_le_bytes(args[cursor..cursor + 8].try_into().unwrap()));
                    cursor += 8;
                }
                Ty::VecI64 => {
                    let (vec, next_cursor) = vec_wire::read_vec(args, cursor)?;
                    cursor = next_cursor;
                    let handle = vec_pool.len() as u32;
                    vec_pool.push(Arc::from(vec));
                    slot_vec_handle[slot] = Some(handle);
                    input_scalars.push(0); // placeholder, never read for Vec slots
                }
                Ty::Bool => return Err(KasmError::TypeMismatch { node: slot }),
            }
        }
        if cursor != args.len() {
            return Err(KasmError::BadInputLength {
                expected: cursor,
                got: args.len(),
            });
        }
    }
    // Hash-chain fast path requires a flat scalar args layout — only
    // applicable when no Vec inputs are in play.
    if vec_pool.is_empty() {
        if let Some(out) = execute_hash_chain(program, args)? {
            return Ok(out);
        }
    }

    let mut values = Vec::with_capacity(program.nodes().len());
    let mut future_slots: Vec<Option<(i64, u16)>> = Vec::with_capacity(program.nodes().len());
    let mut out = Vec::new();
    for (i, node) in program.nodes().iter().copied().enumerate() {
        let mut future_slot = None;
        let value = match node.op {
            Op::Input => match node.ty {
                Ty::I64 | Ty::F64 => Value::I64(input_scalars[node.imm as usize]),
                Ty::Bool => return Err(KasmError::TypeMismatch { node: i }),
                Ty::VecI64 => {
                    let slot = node.imm as usize;
                    let handle = slot_vec_handle.get(slot).and_then(|h| *h).ok_or(
                        KasmError::BadInputSlot { node: i, slot: node.imm },
                    )?;
                    Value::VecI64(handle)
                }
            },
            Op::ConstI64 => Value::I64(node.imm as i64),
            Op::AddI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.wrapping_add(unsafe { read_i64_fast(&values, node.b) })),
            Op::MulI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.wrapping_mul(unsafe { read_i64_fast(&values, node.b) })),
            Op::SubI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.wrapping_sub(unsafe { read_i64_fast(&values, node.b) })),
            Op::DivI64Checked => {
                Value::I64(unsafe { read_i64_fast(&values, node.a) }.checked_div(unsafe { read_i64_fast(&values, node.b) }).unwrap_or(0))
            }
            Op::MinI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.min(unsafe { read_i64_fast(&values, node.b) })),
            Op::MaxI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.max(unsafe { read_i64_fast(&values, node.b) })),
            Op::EqI64 => Value::Bool(unsafe { read_i64_fast(&values, node.a) } == unsafe { read_i64_fast(&values, node.b) }),
            Op::Hash64 => Value::I64(hash_i64(unsafe { read_i64_fast(&values, node.a) })),
            Op::BitFlipI64 => Value::I64(!unsafe { read_i64_fast(&values, node.a) }),
            Op::NegI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.wrapping_neg()),
            Op::ReverseBitsI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.reverse_bits()),
            Op::ByteswapI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.swap_bytes()),
            Op::PopcntI64 => Value::I64(crate::cpu_bits::popcount_u64(unsafe { read_i64_fast(&values, node.a) } as u64) as i64),
            Op::LzcntI64 => Value::I64(crate::cpu_bits::leading_zeros_u64(unsafe { read_i64_fast(&values, node.a) } as u64) as i64),
            Op::TzcntI64 => Value::I64(crate::cpu_bits::trailing_zeros_u64(unsafe { read_i64_fast(&values, node.a) } as u64) as i64),
            Op::PextI64 => Value::I64(crate::cpu_bits::pext_u64(
                unsafe { read_i64_fast(&values, node.a) } as u64,
                unsafe { read_i64_fast(&values, node.b) } as u64,
            ) as i64),
            Op::PdepI64 => Value::I64(crate::cpu_bits::pdep_u64(
                unsafe { read_i64_fast(&values, node.a) } as u64,
                unsafe { read_i64_fast(&values, node.b) } as u64,
            ) as i64),
            Op::Lazy => {
                let (_, future) = future_key_i64(program, args, node.a);
                future_slot = Some((future, node.a));
                Value::I64(future)
            }
            Op::Force => {
                let future = unsafe { read_i64_fast(&values, node.a) };
                let (expected, child) = future_slots
                    .get(node.a as usize)
                    .and_then(|slot| *slot)
                    .ok_or(KasmError::TypeMismatch { node: i })?;
                if future != expected {
                    return Err(KasmError::TypeMismatch { node: i });
                }
                *values.get(child as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: child,
                })?
            }
            Op::SelectI64 => {
                let if_false = checked_imm_ref(node.imm, i)?;
                if unsafe { read_bool_fast(&values, node.a) } {
                    Value::I64(unsafe { read_i64_fast(&values, node.b) })
                } else {
                    Value::I64(unsafe { read_i64_fast(&values, if_false) })
                }
            }
            Op::AndBool => Value::Bool(unsafe { read_bool_fast(&values, node.a) } && unsafe { read_bool_fast(&values, node.b) }),
            Op::OrBool => Value::Bool(unsafe { read_bool_fast(&values, node.a) } || unsafe { read_bool_fast(&values, node.b) }),
            Op::NotBool => Value::Bool(!unsafe { read_bool_fast(&values, node.a) }),
            Op::LtI64 => Value::Bool(unsafe { read_i64_fast(&values, node.a) } < unsafe { read_i64_fast(&values, node.b) }),
            Op::LeI64 => Value::Bool(unsafe { read_i64_fast(&values, node.a) } <= unsafe { read_i64_fast(&values, node.b) }),
            Op::BitAndI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) } & unsafe { read_i64_fast(&values, node.b) }),
            Op::BitOrI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) } | unsafe { read_i64_fast(&values, node.b) }),
            Op::BitXorI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) } ^ unsafe { read_i64_fast(&values, node.b) }),
            Op::ShlI64 => {
                let v = unsafe { read_i64_fast(&values, node.a) };
                let s = (unsafe { read_i64_fast(&values, node.b) } as u64) & 63;
                Value::I64(((v as u64).wrapping_shl(s as u32)) as i64)
            }
            Op::ShrI64 => {
                let v = unsafe { read_i64_fast(&values, node.a) } as u64;
                let s = (unsafe { read_i64_fast(&values, node.b) } as u64) & 63;
                Value::I64((v.wrapping_shr(s as u32)) as i64)
            }
            Op::SatAddI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.saturating_add(unsafe { read_i64_fast(&values, node.b) })),
            Op::SatSubI64 => Value::I64(unsafe { read_i64_fast(&values, node.a) }.saturating_sub(unsafe { read_i64_fast(&values, node.b) })),
            Op::ModI64Checked => {
                let a = unsafe { read_i64_fast(&values, node.a) };
                let b = unsafe { read_i64_fast(&values, node.b) };
                Value::I64(a.checked_rem(b).unwrap_or(0))
            }
            Op::ClampI64 => {
                let v = unsafe { read_i64_fast(&values, node.a) };
                let lo = unsafe { read_i64_fast(&values, node.b) };
                let hi_ref = checked_imm_ref(node.imm, i)?;
                let hi = unsafe { read_i64_fast(&values, hi_ref) };
                Value::I64(v.max(lo).min(hi))
            }
            Op::ReduceAddI64 => {
                let base = node.a as usize;
                let count = node.imm as usize;
                let mut acc: i64 = 0;
                for off in 0..count {
                    acc = acc.wrapping_add(unsafe { read_i64_fast(&values, (base + off) as u16) });
                }
                Value::I64(acc)
            }
            Op::ReduceMulI64 => {
                let base = node.a as usize;
                let count = node.imm as usize;
                let mut acc: i64 = 1;
                for off in 0..count {
                    acc = acc.wrapping_mul(unsafe { read_i64_fast(&values, (base + off) as u16) });
                }
                Value::I64(acc)
            }
            Op::ConstF64 => Value::I64(f64_to_bits_i64(node.imm as f64)),
            Op::F64Op => exec_f64_op(node, &values, i)?,
            Op::Output => {
                let value = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                encode_value(*value, node.ty, i, &mut out, &vec_pool)?;
                *value
            }
            // ─── KASM v1.0 mutation — interpreter implementations ─────
            Op::Adaptive | Op::Memoize => {
                // Pass-through wrappers: at the interpreter level, no
                // adaptive choice is made (all configurations equivalent
                // in the scalar interpreter). Real auto-tuning happens
                // at the GPU/SIMD muscle layer. Memoize is also a
                // pass-through here — the brain's content-addressed
                // cache already captures the value.
                let v = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                *v
            }
            Op::Comptime => {
                // At runtime we treat as pass-through. Real comptime
                // evaluation happens at program load, BEFORE the program
                // hash is computed — the content-addressed program never
                // contains an unfolded Op::Comptime if it could be
                // pre-evaluated. If we see one at runtime, the optimizer
                // chose to keep it (e.g. depends on input). Pass-through.
                let v = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                *v
            }
            Op::Cond => {
                // pred ? then_slot : else_slot
                let pred = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                let pred_true = match pred {
                    Value::Bool(b) => *b,
                    Value::I64(n) => *n != 0,
                    // Wave 7b — a Vec is never a valid predicate. The
                    // verifier guarantees Cond's pred slot is Bool or
                    // I64, so reaching this is a programming error
                    // (Op::Cond constructed manually with a Vec ref).
                    Value::VecI64(_) => return Err(KasmError::TypeMismatch { node: i }),
                };
                let chosen_idx = if pred_true {
                    node.b as usize
                } else {
                    node.imm as usize
                };
                let chosen = values.get(chosen_idx).ok_or(KasmError::BadRef {
                    node: i,
                    reference: chosen_idx as u16,
                })?;
                *chosen
            }
            Op::VLenI64 => {
                // Wave 7d — Vec length query. Verifier guarantees
                // values[a] is Value::VecI64(handle) ; the handle
                // indexes vec_pool to get the underlying Arc<[i64]>.
                let value = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                let len = match value {
                    Value::VecI64(handle) => {
                        let vec = vec_pool.get(*handle as usize).ok_or(
                            KasmError::BadRef { node: i, reference: *handle as u16 },
                        )?;
                        vec.len() as i64
                    }
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                Value::I64(len)
            }
            Op::VSumI64 => {
                // Wave 7d-bis — Vec sum reduction (wrapping).
                let value = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                let sum = match value {
                    Value::VecI64(handle) => {
                        let vec = vec_pool.get(*handle as usize).ok_or(
                            KasmError::BadRef { node: i, reference: *handle as u16 },
                        )?;
                        vec.iter().fold(0i64, |acc, &x| acc.wrapping_add(x))
                    }
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                Value::I64(sum)
            }
            Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
            | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64 => {
                // Wave 7d-bis + 7e + 7g — pairwise vec arithmetic.
                // Lengths must match exactly (no silent shape coercion).
                let va = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                let vb = values.get(node.b as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.b,
                })?;
                let (handle_a, handle_b) = match (va, vb) {
                    (Value::VecI64(ha), Value::VecI64(hb)) => (*ha, *hb),
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let vec_a = vec_pool.get(handle_a as usize).ok_or(
                    KasmError::BadRef { node: i, reference: handle_a as u16 },
                )?;
                let vec_b = vec_pool.get(handle_b as usize).ok_or(
                    KasmError::BadRef { node: i, reference: handle_b as u16 },
                )?;
                if vec_a.len() != vec_b.len() {
                    return Err(KasmError::TypeMismatch { node: i });
                }
                let result: Vec<i64> = match node.op {
                    Op::VAddI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x.wrapping_add(*y)).collect(),
                    Op::VMulI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x.wrapping_mul(*y)).collect(),
                    Op::VSubI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x.wrapping_sub(*y)).collect(),
                    Op::VMaxI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| (*x).max(*y)).collect(),
                    Op::VMinI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| (*x).min(*y)).collect(),
                    // Wave 7g — equality + bitwise.
                    Op::VEqI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| if x == y { 1i64 } else { 0i64 }).collect(),
                    Op::VAndI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x & y).collect(),
                    Op::VOrI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x | y).collect(),
                    Op::VXorI64 => vec_a.iter().zip(vec_b.iter())
                        .map(|(x, y)| x ^ y).collect(),
                    _ => unreachable!("guarded by outer match arm"),
                };
                let new_handle = vec_pool.len() as u32;
                vec_pool.push(Arc::from(result));
                Value::VecI64(new_handle)
            }
            Op::VRangeI64 => {
                // Wave 7e — produce [0, 1, ..., values[a]-1].
                // Negative or zero length → empty vec (no panic).
                let len = unsafe { read_i64_fast(&values, node.a) };
                let len_clamped = len.max(0) as usize;
                let result: Vec<i64> = (0..len_clamped as i64).collect();
                let new_handle = vec_pool.len() as u32;
                vec_pool.push(Arc::from(result));
                Value::VecI64(new_handle)
            }
            Op::VConcatI64 => {
                // Wave 7f — concatenate values[a] then values[b].
                let va = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i, reference: node.a,
                })?;
                let vb = values.get(node.b as usize).ok_or(KasmError::BadRef {
                    node: i, reference: node.b,
                })?;
                let (ha, hb) = match (va, vb) {
                    (Value::VecI64(ha), Value::VecI64(hb)) => (*ha, *hb),
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let vec_a = vec_pool.get(ha as usize).ok_or(
                    KasmError::BadRef { node: i, reference: ha as u16 },
                )?.clone();
                let vec_b = vec_pool.get(hb as usize).ok_or(
                    KasmError::BadRef { node: i, reference: hb as u16 },
                )?.clone();
                let mut result: Vec<i64> = Vec::with_capacity(vec_a.len() + vec_b.len());
                result.extend_from_slice(&vec_a);
                result.extend_from_slice(&vec_b);
                let new_handle = vec_pool.len() as u32;
                vec_pool.push(Arc::from(result));
                Value::VecI64(new_handle)
            }
            Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
                // Wave 7f + 7h — unary Vec → Vec transformations.
                let value = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i, reference: node.a,
                })?;
                let handle = match value {
                    Value::VecI64(h) => *h,
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                let vec = vec_pool.get(handle as usize).ok_or(
                    KasmError::BadRef { node: i, reference: handle as u16 },
                )?;
                let result: Vec<i64> = match node.op {
                    Op::VReverseI64 => vec.iter().rev().copied().collect(),
                    Op::VAbsI64 => vec.iter().map(|x| x.wrapping_abs()).collect(),
                    Op::VNegI64 => vec.iter().map(|x| x.wrapping_neg()).collect(),
                    Op::VBitFlipI64 => vec.iter().map(|x| !x).collect(),
                    _ => unreachable!("guarded by outer match arm"),
                };
                let new_handle = vec_pool.len() as u32;
                vec_pool.push(Arc::from(result));
                Value::VecI64(new_handle)
            }
            Op::VBroadcastI64 => {
                // Wave 7f — fill : Vec of length values[b] all = values[a].
                let value = unsafe { read_i64_fast(&values, node.a) };
                let len = unsafe { read_i64_fast(&values, node.b) };
                let len_clamped = len.max(0) as usize;
                let result: Vec<i64> = vec![value; len_clamped];
                let new_handle = vec_pool.len() as u32;
                vec_pool.push(Arc::from(result));
                Value::VecI64(new_handle)
            }
            Op::VGetI64 => {
                // Wave 7i — Vec random-access read : `vec[idx % len]` → i64.
                // Empty vec → 0. Index is interpreted unsigned modulo len so
                // negative indices wrap predictably (no panic, no UB).
                let value = values.get(node.a as usize).ok_or(KasmError::BadRef {
                    node: i,
                    reference: node.a,
                })?;
                let result = match value {
                    Value::VecI64(handle) => {
                        let vec = vec_pool.get(*handle as usize).ok_or(
                            KasmError::BadRef { node: i, reference: *handle as u16 },
                        )?;
                        if vec.is_empty() {
                            0i64
                        } else {
                            let idx = unsafe { read_i64_fast(&values, node.b) };
                            let pos = (idx as u64 % vec.len() as u64) as usize;
                            vec[pos]
                        }
                    }
                    _ => return Err(KasmError::TypeMismatch { node: i }),
                };
                Value::I64(result)
            }
            Op::Pipeline
            | Op::Grad
            | Op::Vmap
            | Op::Pmap
            | Op::Fori
            | Op::WhileLoop
            | Op::Reduce
            | Op::Scan => {
                // These v1.0 ops require Forge brain dispatch.
                return Err(KasmError::UnsupportedV1OpInScalarInterpreter {
                    node: i,
                    op_byte: node.op as u8,
                });
            }
            // Wave 8 FULL — Op::Fractal/Op::Eval : dispatcher si fourni,
            // sinon fail-loud (rétro-compat avec call sites historiques).
            Op::Fractal => {
                let callee_id = unsafe { read_i64_fast(&values, node.a) };
                let arg = unsafe { read_i64_fast(&values, node.b) };
                let dispatcher = dispatcher.ok_or(
                    KasmError::UnsupportedV1OpInScalarInterpreter {
                        node: i,
                        op_byte: node.op as u8,
                    },
                )?;
                let result = dispatcher.fractal(callee_id, arg)?;
                Value::I64(result)
            }
            Op::Eval => {
                let eval_id = unsafe { read_i64_fast(&values, node.a) };
                let arg = unsafe { read_i64_fast(&values, node.b) };
                let dispatcher = dispatcher.ok_or(
                    KasmError::UnsupportedV1OpInScalarInterpreter {
                        node: i,
                        op_byte: node.op as u8,
                    },
                )?;
                let result = dispatcher.eval(eval_id, arg)?;
                Value::I64(result)
            }
        };
        values.push(value);
        future_slots.push(future_slot);
    }
    Ok(out)
}

/// Φ.0 — Reinterpret an `f64` as the `i64` bit pattern that lives in
/// `Value::I64`. The wire format and on-the-stack representation of an
/// F64 value are byte-for-byte identical to its IEEE 754 bit pattern,
/// so this round-trips through the existing I64 plumbing without any
/// new `Value` enum variant.
#[inline]
fn f64_to_bits_i64(v: f64) -> i64 {
    v.to_bits() as i64
}

#[inline]
fn f64_from_bits_i64(b: i64) -> f64 {
    f64::from_bits(b as u64)
}

fn exec_f64_op(node: Node, values: &[Value], _i: usize) -> Result<Value, KasmError> {
    let sub = F64SubOp::from_imm(node.imm)?;
    // Σ.1 — verifier-guaranteed refs ; see read_i64_fast doc-block.
    let a_bits = unsafe { read_i64_fast(values, node.a) };
    let result_bits = match sub {
        F64SubOp::Add => {
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            f64_to_bits_i64(a + b)
        }
        F64SubOp::Sub => {
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            f64_to_bits_i64(a - b)
        }
        F64SubOp::Mul => {
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            f64_to_bits_i64(a * b)
        }
        F64SubOp::DivChecked => {
            // Total function: NaN / ±Inf / divide-by-zero → 0.0. The
            // kill-switch for the synthesizer is built in here so any
            // divergence immediately collapses to a deterministic 0.
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            let r = a / b;
            f64_to_bits_i64(if r.is_finite() { r } else { 0.0 })
        }
        F64SubOp::Min => {
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            // `f64::min` propagates the non-NaN argument; we collapse
            // dual-NaN to 0.0 so the function is total + deterministic.
            let r = a.min(b);
            f64_to_bits_i64(if r.is_nan() { 0.0 } else { r })
        }
        F64SubOp::Max => {
            let a = f64_from_bits_i64(a_bits);
            let b = f64_from_bits_i64(unsafe { read_i64_fast(values, node.b) });
            let r = a.max(b);
            f64_to_bits_i64(if r.is_nan() { 0.0 } else { r })
        }
        F64SubOp::Sqrt => {
            // `sqrt` of a negative is NaN; collapse to 0.0 — this is
            // the same total-function discipline as `DivI64Checked`.
            let a = f64_from_bits_i64(a_bits);
            let r = a.sqrt();
            f64_to_bits_i64(if r.is_finite() { r } else { 0.0 })
        }
        F64SubOp::Abs => f64_to_bits_i64(f64_from_bits_i64(a_bits).abs()),
        F64SubOp::Neg => f64_to_bits_i64(-f64_from_bits_i64(a_bits)),
        F64SubOp::FromI64 => f64_to_bits_i64(a_bits as f64),
        F64SubOp::ToI64 => {
            let a = f64_from_bits_i64(a_bits);
            if a.is_finite() {
                // Truncate toward zero, saturate at i64 bounds.
                if a >= i64::MAX as f64 {
                    i64::MAX
                } else if a <= i64::MIN as f64 {
                    i64::MIN
                } else {
                    a as i64
                }
            } else {
                0
            }
        }
        F64SubOp::Exp => {
            // Φ.7a — `e^a`. NaN/Inf → 0.0 keeps the op total. exp
            // overflows for `a > ~709.78` and underflows to 0.0 for
            // `a < ~-745.13`; both are absorbed by the kill-switch.
            let a = f64_from_bits_i64(a_bits);
            let r = a.exp();
            f64_to_bits_i64(if r.is_finite() { r } else { 0.0 })
        }
        F64SubOp::Ln => {
            // Φ.7a — `ln(|a|)` (natural log of absolute value). The
            // `|·|` is baked in so the op is total: ln(0) → 0.0,
            // ln(neg) becomes ln(|neg|). Cross-host divergence is
            // bounded by libc ULP differences.
            let a = f64_from_bits_i64(a_bits).abs();
            let r = if a == 0.0 { 0.0 } else { a.ln() };
            f64_to_bits_i64(if r.is_finite() { r } else { 0.0 })
        }
    };
    Ok(Value::I64(result_bits))
}

fn execute_hash_chain(program: &Program, args: &[u8]) -> Result<Option<Vec<u8>>, KasmError> {
    if program.outputs() != 1 {
        return Ok(None);
    }

    let Some((output_index, output)) = program
        .nodes()
        .iter()
        .copied()
        .enumerate()
        .find(|(_, node)| node.op == Op::Output)
    else {
        return Ok(None);
    };
    // Wave 7b — Vec outputs aren't a hash chain pattern by definition
    // (the chain is a sequence of Op::Hash64 over a single i64 input).
    // Decline to optimize, fall back to general execute.
    if output.ty != Ty::I64 {
        return Ok(None);
    }

    let mut rounds = 0usize;
    let mut current = output.a as usize;
    loop {
        let Some(node) = program.nodes().get(current).copied() else {
            return Err(KasmError::BadRef { node: output_index, reference: output.a });
        };
        match node.op {
            Op::Hash64 => {
                rounds += 1;
                current = node.a as usize;
            }
            Op::Input => {
                let start = node.imm as usize * 8;
                let Some(bytes) = args.get(start..start + 8) else {
                    return Err(KasmError::BadInputSlot { node: current, slot: node.imm });
                };
                let mut value = i64::from_le_bytes(bytes.try_into().unwrap());
                for _ in 0..rounds {
                    value = hash_i64(value);
                }
                return Ok(Some(value.to_le_bytes().to_vec()));
            }
            _ => return Ok(None),
        }
    }
}

pub fn compose(left: &Program, right: &Program, target: super::types::Target) -> Result<Program, KasmError> {
    let left_outputs = left.output_sources();
    if left_outputs.len() != right.inputs() as usize {
        return Err(KasmError::ComposeArity {
            left_outputs: left.outputs(),
            right_inputs: right.inputs(),
        });
    }
    for (slot, ((_, left_ty), right_ty)) in left_outputs
        .iter()
        .copied()
        .zip(right.input_types())
        .enumerate()
    {
        // Wave 7b — Vec types are first-class. Composition just
        // requires that left output ty matches right input ty (incl.
        // VecI64 ↔ VecI64). Mismatched types still error, exactly as
        // before for scalar types.
        if left_ty != right_ty {
            return Err(KasmError::ComposeType { slot, left: left_ty, right: right_ty });
        }
    }

    let mut nodes = Vec::new();
    let mut left_map = vec![None; left.nodes().len()];
    for (i, node) in left.nodes().iter().copied().enumerate() {
        if node.op == Op::Output {
            continue;
        }
        let remapped = remap_node(node, &left_map, i, true)?;
        let idx = push_node(&mut nodes, remapped, i)?;
        left_map[i] = Some(idx);
    }

    let wired_inputs = left_outputs
        .iter()
        .map(|(source, _)| remap_ref(&left_map, *source, *source as usize))
        .collect::<Result<Vec<_>, _>>()?;

    let mut right_map = vec![None; right.nodes().len()];
    for (i, node) in right.nodes().iter().copied().enumerate() {
        if node.op == Op::Input {
            right_map[i] = Some(*wired_inputs.get(node.imm as usize).ok_or(KasmError::BadInputSlot {
                node: i,
                slot: node.imm,
            })?);
            continue;
        }
        let remapped = remap_node(node, &right_map, i, false)?;
        let idx = push_node(&mut nodes, remapped, i)?;
        right_map[i] = Some(idx);
    }

    Program::new(target, left.inputs(), right.outputs(), nodes.len() as u32, nodes)
}

// ───────────────────────────────────────────────────────────────────
// Σ.1 (Phase Ω.10, 2026-05-01) — bounds-check elision on hot path
//
// `Program::new` and `Program::from_bytes` both run `verify_node` for
// every node before returning. `verify_node` calls `expect_ref(node,
// ref, expected_ty, types)` which guarantees that for every `node.a`,
// `node.b`, and immediate ref :
//   1. The reference is in range : `(ref as usize) < types.len()` at
//      the time the node was checked, where `types.len()` equals the
//      node's own index. Combined with the in-order build of
//      `values` inside `execute()`, this proves
//      `(ref as usize) < values.len()` when we read it.
//   2. The type at that slot matches what the op consumes.
//
// Therefore `read_i64_fast` and `read_bool_fast` use `get_unchecked`
// for bounds and `unreachable_unchecked` for the variant. The
// previously-existing safe `read_i64`/`read_bool` were dead code
// (Σ.1 confirmed via `grep` — no external callers) and got the
// chop. If a future caller needs runtime-checked reads (e.g. an
// experimental IR that hasn't been through `verify_node`), they
// should reintroduce the safe variants explicitly.
//
// Trade-off : ~5-15% faster on the slow-lane interpreter (where
// every node read pays a bounds check + variant match). Risk : if
// `verify_node` ever has a hole, we get UB instead of `KasmError`.
// Mitigation : `debug_assert!` keeps a runtime-checked path in dev
// builds ; the unsafe block is reached only in release.
// ───────────────────────────────────────────────────────────────────

/// SAFETY-checked read of an `i64` value — verifier-guaranteed by
/// construction. See module-level Σ.1 doc-block for the invariant.
#[inline(always)]
unsafe fn read_i64_fast(values: &[Value], idx: u16) -> i64 {
    debug_assert!(
        (idx as usize) < values.len(),
        "Σ.1 invariant broken: ref {idx} >= values.len() {} — verifier hole?",
        values.len()
    );
    debug_assert!(
        matches!(unsafe { values.get_unchecked(idx as usize) }, Value::I64(_)),
        "Σ.1 invariant broken: slot {idx} not Value::I64 — verifier hole?"
    );
    match unsafe { values.get_unchecked(idx as usize) } {
        Value::I64(v) => *v,
        // SAFETY: the verifier proved this slot's type matches what
        // the op consumes ; reaching this branch is impossible for a
        // verified Program.
        _ => unsafe { std::hint::unreachable_unchecked() },
    }
}

/// SAFETY-checked read of a `bool` value — same invariant as
/// `read_i64_fast`.
#[inline(always)]
unsafe fn read_bool_fast(values: &[Value], idx: u16) -> bool {
    debug_assert!(
        (idx as usize) < values.len(),
        "Σ.1 invariant broken: ref {idx} >= values.len() {} — verifier hole?",
        values.len()
    );
    debug_assert!(
        matches!(unsafe { values.get_unchecked(idx as usize) }, Value::Bool(_)),
        "Σ.1 invariant broken: slot {idx} not Value::Bool — verifier hole?"
    );
    match unsafe { values.get_unchecked(idx as usize) } {
        Value::Bool(v) => *v,
        _ => unsafe { std::hint::unreachable_unchecked() },
    }
}

fn remap_ref(map: &[Option<u16>], reference: u16, node: usize) -> Result<u16, KasmError> {
    map.get(reference as usize)
        .and_then(|mapped| *mapped)
        .ok_or(KasmError::BadRef { node, reference })
}

fn remap_node(
    node: Node,
    map: &[Option<u16>],
    index: usize,
    keep_inputs: bool,
) -> Result<Node, KasmError> {
    let map_ref = |reference| remap_ref(map, reference, index);
    Ok(match node.op {
        Op::Input if keep_inputs => node,
        Op::Input => unreachable!("right-side inputs are handled before remap_node"),
        Op::ConstI64 | Op::ConstF64 => node,
        Op::F64Op => {
            // `b` is meaningful for binary sub-ops; for unary sub-ops
            // it stays `0` (verified upstream).
            let sub = F64SubOp::from_imm(node.imm)?;
            if sub.is_binary() {
                Node { a: map_ref(node.a)?, b: map_ref(node.b)?, ..node }
            } else {
                Node { a: map_ref(node.a)?, ..node }
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
        | Op::TzcntI64
        | Op::Lazy
        | Op::Force => Node { a: map_ref(node.a)?, ..node },
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::LtI64
        | Op::LeI64
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64 => Node { a: map_ref(node.a)?, b: map_ref(node.b)?, ..node },
        Op::SelectI64 | Op::ClampI64 => Node {
            a: map_ref(node.a)?,
            b: map_ref(node.b)?,
            imm: map_ref(checked_imm_ref(node.imm, index)?)? as i16,
            ..node
        },
        // Reduce nodes refer to a *range* `[a, a + imm)`. Compose only
        // remaps individual slots — a generic remap can break the
        // contiguity invariant. Reject explicitly so callers know to
        // expand the reduce into N binary ops first.
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            return Err(KasmError::BadReduceCount {
                node: index,
                count: node.imm,
            });
        }
        // KASM v1.0 — pass-through wrappers and meta-ops. The compose
        // path remaps internal references; v1.0 ops follow the same
        // shape as their underlying ops (a, b refs to slots).
        Op::Adaptive | Op::Memoize | Op::Comptime | Op::Grad
        | Op::Vmap | Op::Pmap | Op::VLenI64 | Op::VSumI64 | Op::VRangeI64
        | Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64
            => Node { a: map_ref(node.a)?, ..node },
        Op::Pipeline | Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VConcatI64 | Op::VBroadcastI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
        | Op::VGetI64 => Node {
            a: map_ref(node.a)?,
            b: map_ref(node.b)?,
            ..node
        },
        Op::Cond => Node {
            a: map_ref(node.a)?,
            b: map_ref(node.b)?,
            imm: map_ref(checked_imm_ref(node.imm, index)?)? as i16,
            ..node
        },
        // Wave 8 — Fractal/Eval ne traversent pas remap_node car ils
        // sont interceptés au niveau dispatch (SelfHostingRuntime).
        // Si on en voit ici c'est un programme avec un Op::Fractal/Eval
        // qui a fuite jusqu'au remap : fail-loud cohérent avec
        // l'interpreter principal.
        Op::Fractal | Op::Eval => {
            return Err(KasmError::UnsupportedV1OpInScalarInterpreter {
                node: index,
                op_byte: node.op as u8,
            });
        }
    })
}

fn push_node(nodes: &mut Vec<Node>, node: Node, source_index: usize) -> Result<u16, KasmError> {
    if nodes.len() >= MAX_NODES {
        return Err(KasmError::BadNodeCount(nodes.len() + 1));
    }
    let idx = u16::try_from(nodes.len()).map_err(|_| KasmError::BadNodeCount(source_index))?;
    nodes.push(node);
    Ok(idx)
}

fn encode_value(
    value: Value,
    ty: Ty,
    node: usize,
    out: &mut Vec<u8>,
    vec_pool: &[Arc<[i64]>],
) -> Result<(), KasmError> {
    match (value, ty) {
        // F64 wire format is byte-identical to I64 — both emit the
        // 8-byte little-endian bit pattern. The semantic distinction
        // exists only at the type level (Ty) and at the operations
        // that consume the bytes downstream.
        (Value::I64(v), Ty::I64) | (Value::I64(v), Ty::F64) => {
            out.extend_from_slice(&v.to_le_bytes())
        }
        (Value::Bool(v), Ty::Bool) => out.push(u8::from(v)),
        // Wave 7b — Vec output uses the length-prefixed wire format
        // `[u32 LE count | count × 8 bytes]`. The Vec lives in
        // `vec_pool`, indexed by the handle carried in `Value::VecI64`.
        (Value::VecI64(handle), Ty::VecI64) => {
            let vec = vec_pool.get(handle as usize).ok_or(KasmError::BadRef {
                node,
                reference: handle as u16,
            })?;
            vec_wire::write_vec(out, vec);
        }
        _ => return Err(KasmError::ValueTypeMismatch { node }),
    }
    Ok(())
}
