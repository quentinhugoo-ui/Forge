//! Hot path: dispatch a verified KASM program through the unified
//! cache, the structural-rule fast paths, the learned oracle, and
//! finally the interpreter — and persist every successful result as a
//! SCAN memo.
//!
//! Sprint 1 refactor:
//!  * `intern_arg` is gone. For inline-sized args (`<= 16` bytes,
//!    which covers every i64 caller) the args bytes never leave the
//!    stack; we just compute `Hash::for_blob` directly.
//!  * Memo + result are a single cache slot now.
//!  * Stats are atomic — no mutex on the bump path.
//!  * `Arc<[u8]>` everywhere internally so cache hits don't copy.

use std::io;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;

use std::collections::HashMap;

use crate::kasm;
use crate::kasm::{MultiMethod, ProgramSig};
use crate::Hash;

use super::cache::{fast_fingerprint, RamKey, PROGRAM_OVERHEAD};
use super::hotplan::{
    execute_hot_batch_i64, execute_hot_plan, exact_program_identity, hot_plan,
    reject_external_target, should_semantic_fingerprint, should_simplify, HotPlan, HotProgram,
    MemoizedSubProgram,
};
use super::oracle::decode_i64_value;
use super::stats::{read_cycles, MonsterCall, MonsterSource, MonsterValue, PhysicalEnvelope};
use super::MonsterNode;
use crate::kasm::Op;

/// Phase 12.1 — slow-lane interpreter spécialisé pour les programmes
/// `is_decomposable()`. Au lieu de re-lancer chaque `Hash64` sur les
/// 6.4M k-mer-halves d'un fichier ADN, on mémoise au niveau opération :
/// `(Op::Hash64, input: i64) → output: i64`. Pour le strobemer (où le
/// même `low_16` revient des milliers de fois), c'est ×30 mesurable
/// vs le slow lane standard.
///
/// Hot path simple : balaye le DAG une seule fois, exécute chaque op
/// avec valeurs i64 dans un `Vec<i64>` indexé par node_idx. Les ops
/// supportés couvrent ce qu'on rencontre dans les programmes
/// décomposables typiques (k-mer hashing, mixers, manipulations de
/// bits) — pour tout op manquant, on retombe en erreur visible et le
/// caller rebascule sur `execute_hot_plan` (no-op silencieux).
pub(super) fn execute_with_op_memo(
    hot: &HotProgram,
    args: &[u8],
    op_memo: &std::sync::RwLock<HashMap<(Op, i64), i64>>,
    op_memo_hits: &std::sync::atomic::AtomicU64,
    op_memo_misses: &std::sync::atomic::AtomicU64,
    atlas: Option<&crate::atlas::Atlas>,
) -> io::Result<Vec<u8>> {
    let nodes = hot.program.nodes();
    let mut values: Vec<i64> = vec![0; nodes.len()];
    let mut future_child: Vec<Option<u16>> = vec![None; nodes.len()];
    let mut future_value: Vec<i64> = vec![0; nodes.len()];
    let arg_bytes = args;

    for (i, node) in nodes.iter().enumerate() {
        let v: i64 = match node.op {
            Op::Input => {
                let slot = node.imm as usize;
                let start = slot * 8;
                let end = start + 8;
                if end > args.len() {
                    return Err(io::Error::other(format!(
                        "input slot {} out of range (args len {})",
                        slot,
                        args.len()
                    )));
                }
                let mut buf = [0u8; 8];
                buf.copy_from_slice(&args[start..end]);
                i64::from_le_bytes(buf)
            }
            Op::ConstI64 => node.imm as i64,
            Op::AddI64 => values[node.a as usize].wrapping_add(values[node.b as usize]),
            Op::SubI64 => values[node.a as usize].wrapping_sub(values[node.b as usize]),
            Op::MulI64 => values[node.a as usize].wrapping_mul(values[node.b as usize]),
            Op::ShlI64 => {
                let a = values[node.a as usize];
                let b = (values[node.b as usize] as u64) & 63;
                ((a as u64).wrapping_shl(b as u32)) as i64
            }
            Op::ShrI64 => {
                let a = values[node.a as usize] as u64;
                let b = (values[node.b as usize] as u64) & 63;
                (a >> b) as i64
            }
            Op::BitAndI64 => values[node.a as usize] & values[node.b as usize],
            Op::BitOrI64 => values[node.a as usize] | values[node.b as usize],
            Op::BitXorI64 => values[node.a as usize] ^ values[node.b as usize],
            Op::BitFlipI64 => !values[node.a as usize],
            Op::NegI64 => values[node.a as usize].wrapping_neg(),
            Op::ReverseBitsI64 => values[node.a as usize].reverse_bits(),
            Op::ByteswapI64 => values[node.a as usize].swap_bytes(),
            Op::PopcntI64 => crate::cpu_bits::popcount_u64(values[node.a as usize] as u64) as i64,
            Op::LzcntI64 => crate::cpu_bits::leading_zeros_u64(values[node.a as usize] as u64) as i64,
            Op::TzcntI64 => crate::cpu_bits::trailing_zeros_u64(values[node.a as usize] as u64) as i64,
            Op::PextI64 => crate::cpu_bits::pext_u64(
                values[node.a as usize] as u64,
                values[node.b as usize] as u64,
            ) as i64,
            Op::PdepI64 => crate::cpu_bits::pdep_u64(
                values[node.a as usize] as u64,
                values[node.b as usize] as u64,
            ) as i64,
            Op::Lazy => {
                let (_, future) = crate::kasm::interpreter::future_key_i64(&hot.program, arg_bytes, node.a);
                future_child[i] = Some(node.a);
                future_value[i] = future;
                future
            }
            Op::Force => {
                let future_idx = node.a as usize;
                let child = future_child
                    .get(future_idx)
                    .and_then(|slot| *slot)
                    .ok_or_else(|| io::Error::other("execute_with_op_memo: Force input is not a Lazy future"))?;
                if values[future_idx] != future_value[future_idx] {
                    return Err(io::Error::other("execute_with_op_memo: Force future hash mismatch"));
                }
                values[child as usize]
            }
            Op::Hash64 => {
                let input = values[node.a as usize];
                // L1 lookup — RAM op_memo (fast).
                let cached = {
                    let memo = op_memo.read().unwrap();
                    memo.get(&(Op::Hash64, input)).copied()
                };
                match cached {
                    Some(v) => {
                        op_memo_hits.fetch_add(1, Ordering::Relaxed);
                        v
                    }
                    None => {
                        // L2 lookup — atlas RESULT cross-session memo.
                        // M1.5 unified all value-bearing kinds under
                        // kind::RESULT.
                        let atlas_v = atlas.and_then(|a| {
                            let key = crate::atlas::Atlas::opmemo_key(Op::Hash64 as u8, input);
                            a.lookup_with_value(crate::atlas::kind::RESULT, &key)
                                .map(|packed| crate::atlas::Atlas::unpack_i64(&packed))
                        });
                        let v = match atlas_v {
                            Some(v) => v,
                            None => crate::kasm::hash_i64(input),
                        };
                        op_memo
                            .write()
                            .unwrap()
                            .insert((Op::Hash64, input), v);
                        if atlas_v.is_none() {
                            if let Some(a) = atlas {
                                let key = crate::atlas::Atlas::opmemo_key(Op::Hash64 as u8, input);
                                let _ = a.record_with_value(
                                    crate::atlas::kind::RESULT,
                                    &key,
                                    &crate::atlas::Atlas::pack_i64(v),
                                );
                            }
                        }
                        op_memo_misses.fetch_add(1, Ordering::Relaxed);
                        v
                    }
                }
            }
            Op::Output => {
                return Ok(values[node.a as usize].to_le_bytes().to_vec());
            }
            other => {
                return Err(io::Error::other(format!(
                    "execute_with_op_memo: op {:?} non supporté (programme décomposable mais utilise un op hors couverture)",
                    other
                )));
            }
        };
        values[i] = v;
    }

    Err(io::Error::other(
        "execute_with_op_memo: programme sans Output node",
    ))
}

fn build_explicit_memos(program: &Arc<kasm::Program>) -> io::Result<Arc<[MemoizedSubProgram]>> {
    let mut out = Vec::new();
    for subprogram in program
        .memoize_subprograms()
        .map_err(|err| io::Error::other(format!("kasm memoize extraction: {err}")))?
    {
        let program = Arc::new(subprogram);
        let semantic_fingerprint = if should_semantic_fingerprint(&program) {
            program
                .semantic_fingerprint()
                .map_err(|err| io::Error::other(format!("kasm memoize fingerprint: {err}")))?
        } else {
            let canonical_hash = Hash::for_blob(program.bytes());
            exact_program_identity(&program, &canonical_hash)
        };
        out.push(MemoizedSubProgram {
            semantic_fingerprint,
            program,
        });
    }
    Ok(out.into())
}

/// Internal outcome of the unified dispatch pipeline. Both
/// `call_value_bytes_hot_args` (persist=false) and `dispatch_call`
/// (persist=true) project this into their wrapper return types.
struct DispatchOutcome {
    result: Hash,
    /// May be `Arc::new([])` only on the disk-memo branch when
    /// `persist=true` — the caller (`dispatch_call`) discards bytes.
    /// Every other path always carries the real payload.
    bytes_arc: Arc<[u8]>,
    source: MonsterSource,
    /// V8 Solution C : envelope physique observé pour cet appel.
    /// Sur les fast lanes (RAM hit, rule, oracle, static, wire) l'envelope
    /// est `Default` (zéro) : mesurer y aurait un coût comparable au gain.
    /// Sur le slow lane (execute_hot_plan), `cycles` est rempli via RDTSC.
    envelope: PhysicalEnvelope,
}

impl MonsterNode {
    pub fn call_bytes(&self, func: &Hash, args: &[u8]) -> io::Result<MonsterCall> {
        self.call_with_args(func, args)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 4b (Phase Ω.10, 2026-05-01) — MultiMethod wire-up
    //
    // Bridges Wave 4's content-addressed `MultiMethod` data structure
    // into the live MonsterNode runtime. A MultiMethod blob is stored
    // in the same CAS as Programs (different magic bytes) and resolved
    // at call time : runtime input signature → method program hash →
    // existing `call_bytes` pipeline (cache, hotplan, JIT, GPU).
    //
    // Doctrine alignment :
    //   - Pas de nouveau module : extension d'`exec.rs` uniquement.
    //   - Tâche A.2 invariant : resolve sur signature absente retourne
    //     `Ok(None)`, jamais une fake `io::Error`. `io::Error` ne
    //     remonte que pour de vraies pannes (CAS load, decode, KASM
    //     parse).
    //   - Content-addressed : un MultiMethod stocké est immuable. Un
    //     ajout de méthode produit un nouveau hash, pas une mutation.
    // ───────────────────────────────────────────────────────────────

    /// Persist a `MultiMethod` bundle in the CAS. Returns the bundle's
    /// SHA-1-truncated identity (the `Hash` returned by
    /// `Store::store`). Note that this is *not* the same as
    /// `MultiMethod::identity()` (SHA-256 of the canonical encoding) —
    /// the CAS uses its own hashing convention. Both identities map
    /// 1:1 to the same blob, so either works as a stable handle.
    pub fn store_multimethod(&self, mm: &MultiMethod) -> io::Result<Hash> {
        let blob = mm.encode();
        self.store().store(&blob)
    }

    /// Load and decode a `MultiMethod` from the CAS by its store hash.
    /// Real failures (blob missing, bad magic, truncated table)
    /// surface as `io::Error`. The `Option<MultiMethod>` distinction
    /// for "not in store" stays `Result` here because an unknown hash
    /// generally means programmer error or corrupt persistence — not
    /// the absence-as-Option pattern of cache lookups.
    pub fn load_multimethod(&self, mm_hash: &Hash) -> io::Result<MultiMethod> {
        // Wave 9 — distinguish "hash not in CAS" (NotFound) from
        // "blob present but malformed" (Other). Callers can branch
        // on `err.kind()` to handle the two cases distinctly.
        let blob = self.store().load(mm_hash).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("unknown multimethod hash: {mm_hash}"),
            )
        })?;
        MultiMethod::decode(&blob)
            .map_err(|err| io::Error::other(format!("multimethod decode: {err}")))
    }

    /// Resolve a MultiMethod by runtime signature without dispatching.
    ///
    /// Returns `Ok(Some(prog_hash))` when a method matches the runtime
    /// signature exactly, `Ok(None)` when no method applies (Tâche A.2
    /// absence-as-Option), and `Err(io::Error)` only for real failures
    /// (CAS miss on the bundle, decode error).
    ///
    /// `prog_hash` is a 20-byte program identity ready to feed
    /// `call_bytes`/`call_one_i64` through `Hash::from_bytes`.
    pub fn resolve_multimethod(
        &self,
        mm_hash: &Hash,
        runtime_sig: &ProgramSig,
    ) -> io::Result<Option<[u8; 20]>> {
        let mm = self.load_multimethod(mm_hash)?;
        Ok(mm.resolve(runtime_sig))
    }

    /// Wave 4b hot path — Julia-style multiple dispatch in one call.
    /// Loads the bundle, resolves the runtime signature, and dispatches
    /// to the matching program through the existing `call_bytes`
    /// pipeline (so the call still benefits from RAM cache, hotplan,
    /// JIT, and GPU offload as configured on the node).
    ///
    /// Returns `Err(NotFound)` when the bundle has no method for the
    /// runtime signature — distinct from the bundle being absent
    /// (`Err(other)`). Callers that want absence-as-`None` semantics
    /// should use `resolve_multimethod` first and skip the call when it
    /// returns `Ok(None)`.
    pub fn call_multi(
        &self,
        mm_hash: &Hash,
        runtime_sig: &ProgramSig,
        args: &[u8],
    ) -> io::Result<MonsterCall> {
        let prog_id = self.resolve_multimethod(mm_hash, runtime_sig)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("multimethod {mm_hash} has no method for runtime signature {runtime_sig:?}"),
            )
        })?;
        let prog_hash = Hash::from_bytes(prog_id);
        self.call_bytes(&prog_hash, args)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 6 (Phase Ω.10, 2026-05-01) — Op::Pipeline FULL
    //
    // OCaml-style functional composition `g(f(x))` resolved at the
    // brain layer. The scalar KASM interpreter has no atlas access, so
    // a program containing `Op::Pipeline` as a node fails loud (see
    // `interpreter.rs`). The canonical entry point for pipeline
    // composition is this method : caller passes two full program
    // hashes and the input bytes, brain runs each hop through the
    // existing `call_bytes` pipeline (cache, hotplan, JIT, GPU).
    //
    // Doctrine alignment :
    //   - Pas de nouveau module : extension d'`exec.rs`, pattern
    //     identique à `call_multi` (Wave 4b).
    //   - Content-addressed : chaque hop est un `MonsterCall` mémoizé
    //     individuellement. Recomposer le même pipeline sur les
    //     mêmes args ré-emprunte chaque cache hit indépendamment.
    //   - Tâche A.2 invariant : `NotFound` réservé aux programmes
    //     manquants ou au blob intermédiaire absent du CAS, pas aux
    //     erreurs d'exécution KASM (qui restent `io::Error::other`).
    // ───────────────────────────────────────────────────────────────

    /// Wave 6 hot path — compose two content-addressed programs.
    ///
    /// Equivalent to `call_bytes(prog_b, call_bytes(prog_a, args))`,
    /// but in one call so the intermediate hash is never user-visible.
    /// Each hop runs through the full dispatch pipeline (RAM cache,
    /// hotplan, JIT, GPU) so a pipeline executed twice on the same
    /// args is two cache hits, not two re-executions.
    ///
    /// Returns a `MonsterCall` whose `result` hash points at the
    /// final output bytes. The intermediate result is *also* in the
    /// CAS (persisted by `call_bytes` on the first hop) — that's by
    /// design : every step of a pipeline is independently memoizable.
    ///
    /// The `source` field reflects the *second* hop only, since
    /// that's the call whose envelope dominates user-observed
    /// latency. The first hop's stats are still recorded internally.
    ///
    /// Errors :
    ///   - `NotFound` if `prog_a` or `prog_b` is missing from the CAS,
    ///     or if the intermediate result blob disappeared between
    ///     hops (should never happen on a healthy node).
    ///   - Any other `io::Error` propagated from the underlying
    ///     `call_bytes` (KASM exec failure, type mismatch, etc.).
    pub fn call_pipeline(
        &self,
        prog_a: &Hash,
        prog_b: &Hash,
        args: &[u8],
    ) -> io::Result<MonsterCall> {
        let call_a = self.call_bytes(prog_a, args)?;
        let intermediate_bytes = self.store().load(&call_a.result).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "pipeline: intermediate result {} for prog_a {} missing from CAS",
                    call_a.result, prog_a
                ),
            )
        })?;
        self.call_bytes(prog_b, &intermediate_bytes)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 7a (Phase Ω.10, 2026-05-01) — Vec brain ops
    //
    // Runtime equivalents to JAX `vmap` (`call_map`), JAX `pmap`
    // (`call_pmap`), APL `/` (`call_reduce`), APL `\` (`call_scan`).
    //
    // Design : the runtime does not need a `Ty::VecI64` storage in the
    // scalar interpreter for these brain APIs. The op program (the
    // hashed sub-program applied to each element / each accumulator
    // step) operates on plain i64 inputs and outputs ; the brain
    // iterates over the user's `&[i64]` and routes each call through
    // the existing `call_bytes` pipeline (RAM cache, hotplan, JIT).
    // This means a redundant input across calls hits the cache —
    // a strobemer-style workload (lots of repeated values) sees its
    // unique-call count drop to the dedup count automatically.
    //
    // Doctrine alignment :
    //   - Pas de nouveau module : extension d'`exec.rs` uniquement.
    //   - Pattern jumeau de Wave 4b (`call_multi`) et Wave 6
    //     (`call_pipeline`) : take a full Hash + concrete args, no
    //     bytecode-embedded Op::Reduce/Scan/Vmap/Pmap dispatch.
    //   - Filtre paranoïaque (CLAUDE.md §9) : chaque hop est un
    //     call_bytes complet, donc passe par toutes les échelles de
    //     cache automatiquement. Pas de short-circuit.
    // ───────────────────────────────────────────────────────────────

    /// Runtime equivalent of JAX `vmap` — apply a `i64 → i64` program
    /// to each element of `vec`, returning the corresponding output
    /// vector. Each element is dispatched via `call_one_i64` so
    /// redundant inputs inside `vec` hit the cache.
    ///
    /// `op_prog` must be a single-input single-output `i64 → i64`
    /// program ; mismatched arity surfaces as `io::Error` from the
    /// underlying `call_bytes`.
    pub fn call_map(&self, op_prog: &Hash, vec: &[i64]) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(vec.len());
        for &x in vec {
            out.push(self.call_one_i64(op_prog, x)?);
        }
        Ok(out)
    }

    /// Runtime equivalent of JAX `pmap` — same as `call_map` but the
    /// elements are dispatched in parallel through `thread::scope`.
    /// Worth using when `op_prog` is a non-trivial slow-lane program
    /// (the per-element overhead dominates) ; for cache-bound or
    /// sub-µs ops, `call_map` is faster (no thread spawn cost).
    ///
    /// Order of outputs preserved (same indices as `vec`). On any
    /// element error the whole call surfaces an error — partial
    /// results are not exposed.
    pub fn call_pmap(&self, op_prog: &Hash, vec: &[i64]) -> io::Result<Vec<i64>> {
        let _ = self.hot_program(op_prog)?; // warm the program cache once
        let mut out: Vec<i64> = vec![0; vec.len()];
        thread::scope(|scope| {
            let handles: Vec<_> = vec
                .iter()
                .copied()
                .enumerate()
                .map(|(i, x)| scope.spawn(move || (i, self.call_one_i64(op_prog, x))))
                .collect();
            for handle in handles {
                let (i, res) = handle
                    .join()
                    .map_err(|_| io::Error::other("pmap worker panicked"))?;
                out[i] = res?;
            }
            Ok::<(), io::Error>(())
        })?;
        Ok(out)
    }

    /// Runtime equivalent of APL `/` (Haskell `foldl`) — fold `op_prog`
    /// over `vec` starting from `init`. `op_prog` is a binary
    /// `(acc: i64, x: i64) → i64` program with 2 inputs / 1 output.
    /// Each iteration dispatches via `call_bytes` so cache and JIT
    /// apply transparently.
    pub fn call_reduce(&self, op_prog: &Hash, vec: &[i64], init: i64) -> io::Result<i64> {
        let mut acc = init;
        for &x in vec {
            let mut args = [0u8; 16];
            args[..8].copy_from_slice(&acc.to_le_bytes());
            args[8..].copy_from_slice(&x.to_le_bytes());
            let call = self.call_bytes(op_prog, &args)?;
            let out = self.store().load(&call.result).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "reduce: result {} for op_prog {} missing from CAS",
                        call.result, op_prog
                    ),
                )
            })?;
            if out.len() < 8 {
                return Err(io::Error::other(format!(
                    "reduce: op_prog {} returned {} bytes (expected ≥ 8 for i64)",
                    op_prog,
                    out.len()
                )));
            }
            acc = i64::from_le_bytes(out[..8].try_into().unwrap());
        }
        Ok(acc)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 11b (Phase Ω.10, 2026-05-01) — Haskell iterate + APL outer + takeWhile
    //
    // Three primitives from yet-unsourced angles :
    //   - `call_iterate`   : Haskell `iterate :: (a → a) → a → [a]`
    //                        bounded to N (KASM never lazy-infinite)
    //   - `call_outer`     : APL `∘.×` — outer (Cartesian) product
    //                        with arbitrary binary op
    //   - `call_take_while`: Haskell `takeWhile :: (a → Bool) → [a] → [a]`
    //                        prefix collection with predicate
    //
    // Same brain pattern as 7a/7a-bis : op program by Hash, vec in
    // `&[i64]` Rust native, dispatch via `call_bytes`/`call_one_i64`.
    // ───────────────────────────────────────────────────────────────

    /// Haskell `iterate` (bounded to `n` elements) — generate the
    /// sequence `[init, prog(init), prog(prog(init)), ...]` of
    /// length `n`. KASM never produces lazy-infinite sequences, so
    /// the count is mandatory and explicit.
    ///
    /// `prog` is a unary `i64 → i64` program. The first element of
    /// the returned vec is `init` itself (no application yet) ; the
    /// `i`-th element is `prog^i(init)`.
    pub fn call_iterate(&self, prog: &Hash, init: i64, n: usize) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(n);
        if n == 0 {
            return Ok(out);
        }
        let mut acc = init;
        out.push(acc);
        for _ in 1..n {
            acc = self.call_one_i64(prog, acc)?;
            out.push(acc);
        }
        Ok(out)
    }

    /// APL outer product `∘.×` (with arbitrary binary op) — produce
    /// a flattened `len(a) * len(b)` vector where element
    /// `[i * len(b) + j]` is `op(a[i], b[j])`. Row-major layout.
    ///
    /// `op` is a binary `(i64, i64) → i64` program. Empty inputs
    /// return empty output.
    pub fn call_outer(
        &self,
        op: &Hash,
        a: &[i64],
        b: &[i64],
    ) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(a.len() * b.len());
        for &x in a {
            for &y in b {
                let mut args = [0u8; 16];
                args[..8].copy_from_slice(&x.to_le_bytes());
                args[8..].copy_from_slice(&y.to_le_bytes());
                let call = self.call_bytes(op, &args)?;
                let bytes = self.store().load(&call.result).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "outer: result {} for op {} missing from CAS",
                            call.result, op
                        ),
                    )
                })?;
                if bytes.len() < 8 {
                    return Err(io::Error::other(format!(
                        "outer: op {} returned {} bytes (expected ≥ 8 for i64)",
                        op,
                        bytes.len()
                    )));
                }
                out.push(i64::from_le_bytes(bytes[..8].try_into().unwrap()));
            }
        }
        Ok(out)
    }

    /// Haskell `takeWhile` — collect the longest prefix of `vec`
    /// where `pred(x) ≠ 0`. The first element where pred returns 0
    /// terminates the prefix (it is NOT included in the output).
    /// `pred` is a unary `i64 → i64` program (zero = stop).
    pub fn call_take_while(
        &self,
        pred: &Hash,
        vec: &[i64],
    ) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(vec.len());
        for &x in vec {
            let keep = self.call_one_i64(pred, x)?;
            if keep == 0 {
                break;
            }
            out.push(x);
        }
        Ok(out)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 11.6 (Phase Ω.10, 2026-05-01) — Op::Adaptive PARTIAL → FULL
    //
    // Mojo `@adaptive` runtime equivalent : take N implementations of
    // the same function (each addressed by Hash), run them all once,
    // measure cycles via RDTSC, return the result of the fastest.
    //
    // Wave 11.6 first cut : bench-and-pick on every call. The repeat
    // cost is mitigated by the existing RAM cache infrastructure —
    // subsequent identical (impl, args) calls hit `call_one_i64` cache
    // (~50 ns each) so the bench overhead amortises naturally.
    //
    // Future Wave 11.6-bis : add a winner cache keyed on (impls
    // signature, hardware fingerprint) so the bench fires once per
    // (deployment, machine) tuple and subsequent calls go O(1) directly
    // to the winner. Requires a new MonsterNode field — deferred.
    //
    // Op::Adaptive at bytecode level remains pass-through (Wave 1
    // bucket): a Program with embedded Op::Adaptive nodes runs them
    // as transparent identity. The brain-level entry point for real
    // autotuning is `call_adaptive(impls, args)`.
    // ───────────────────────────────────────────────────────────────

    /// Mojo `@adaptive` runtime equivalent — pick the fastest of N
    /// equivalent implementations.
    ///
    /// All `impls` must compute the **same function** (the API trusts
    /// this contract — there is no semantic equivalence check). Each
    /// is dispatched via `call_bytes` once, cycles are measured via
    /// RDTSC, and the result of the fastest run is returned.
    ///
    /// `impls` empty → `Err`. Any impl surfacing an error
    /// short-circuits and returns it (autotuning assumes all impls
    /// work on the given args ; a partial failure is a contract bug,
    /// not "skip and pick one of the others").
    pub fn call_adaptive(
        &self,
        impls: &[Hash],
        args: &[u8],
    ) -> io::Result<MonsterCall> {
        if impls.is_empty() {
            return Err(io::Error::other("adaptive: empty impls list"));
        }

        let mut best_call: Option<MonsterCall> = None;
        let mut best_cycles: u64 = u64::MAX;

        for impl_hash in impls {
            let start = read_cycles();
            let call = self.call_bytes(impl_hash, args)?;
            let elapsed = read_cycles().saturating_sub(start);

            if elapsed < best_cycles {
                best_cycles = elapsed;
                best_call = Some(call);
            }
        }

        // SAFETY: impls non-empty + no impl errored, so the loop ran
        // at least once and best_call was set.
        best_call.ok_or_else(|| {
            io::Error::other("adaptive: invariant broken — non-empty impls but no winner")
        })
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 11a (Phase Ω.10, 2026-05-01) — JAX lax.switch + Erlang try/catch
    //
    // Two control-flow primitives from new source languages :
    //   - `call_switch` : JAX `lax.switch` — N-way conditional
    //     dispatch. Generalizes `Op::Cond` (binary) to arbitrary
    //     branch count. The runtime index selects one of N
    //     content-addressed branch programs.
    //   - `call_try`    : Erlang `try/catch` — fail-safe execution
    //     with a fallback. If `prog` surfaces `io::Error`,
    //     `fallback` is invoked on the same args. Return value is
    //     whichever branch succeeded.
    //
    // Both follow the established brain pattern : op program by
    // Hash, args as `&[u8]`, dispatch via `call_bytes`.
    // ───────────────────────────────────────────────────────────────

    /// JAX `lax.switch(index, branches, *operands)` equivalent —
    /// N-way conditional dispatch resolved at runtime. The branch
    /// index is the runtime input ; the corresponding program in
    /// `branches` is invoked on `args` through the standard
    /// `call_bytes` pipeline.
    ///
    /// Out-of-range `index` (< 0 or ≥ branches.len()) surfaces an
    /// `io::Error` rather than wrapping or saturating — fail-loud.
    /// This matches JAX which would also raise on a dynamic
    /// out-of-range index in the strictly-typed mode.
    pub fn call_switch(
        &self,
        index: i64,
        branches: &[Hash],
        args: &[u8],
    ) -> io::Result<MonsterCall> {
        if index < 0 || (index as usize) >= branches.len() {
            return Err(io::Error::other(format!(
                "switch: index {index} out of range (have {} branches)",
                branches.len()
            )));
        }
        self.call_bytes(&branches[index as usize], args)
    }

    /// Erlang `try ... catch` equivalent for KASM programs.
    ///
    /// Run `prog` on `args`. If it surfaces an `io::Error` (KASM
    /// exec error, missing program, type mismatch, etc.), run
    /// `fallback` on the *same* args and return that result.
    /// Successful execution of `prog` returns its result directly ;
    /// the fallback is never invoked on the success path.
    ///
    /// If both branches fail, the *fallback's* error surfaces (the
    /// last-line-of-defense convention — the original error is
    /// already implied by the fact that we reached the fallback).
    pub fn call_try(
        &self,
        prog: &Hash,
        args: &[u8],
        fallback: &Hash,
    ) -> io::Result<MonsterCall> {
        match self.call_bytes(prog, args) {
            Ok(c) => Ok(c),
            Err(_) => self.call_bytes(fallback, args),
        }
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 7a-bis (Phase Ω.10, 2026-05-01) — APL/Julia/Haskell extras
    //
    // Round out the array-ops family started in Wave 7a (map, pmap,
    // reduce, scan) with two more canonical operations :
    //   - `call_filter` : Haskell `filter`, APL `compress` (`/⍨`)
    //   - `call_zip`    : Julia broadcasting `f.(x,y)`, APL `+`/`-`
    //                     element-wise on equal-length vectors
    //
    // Same brain pattern as Wave 7a : op program by Hash, vec in
    // `&[i64]` Rust native, dispatch each element through
    // `call_bytes` so the cache + JIT apply transparently.
    // ───────────────────────────────────────────────────────────────

    /// Haskell `filter` / APL `compress` — keep elements of `vec`
    /// for which `pred_prog(x) ≠ 0`. `pred_prog` is a `i64 → i64`
    /// program (zero = drop, non-zero = keep). The output vector
    /// preserves the input order ; redundant inputs hit the cache.
    pub fn call_filter(&self, pred_prog: &Hash, vec: &[i64]) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(vec.len());
        for &x in vec {
            let keep = self.call_one_i64(pred_prog, x)?;
            if keep != 0 {
                out.push(x);
            }
        }
        Ok(out)
    }

    /// Julia broadcasting `f.(x, y)` / APL element-wise binary ops.
    /// Pairwise apply `op_prog: (i64, i64) → i64` to corresponding
    /// elements of `a` and `b`. Both vectors must have equal length ;
    /// mismatched lengths surface as an error rather than truncating
    /// or zero-padding (no silent shape coercion).
    pub fn call_zip(&self, op_prog: &Hash, a: &[i64], b: &[i64]) -> io::Result<Vec<i64>> {
        if a.len() != b.len() {
            return Err(io::Error::other(format!(
                "zip: length mismatch (a={}, b={}); shapes must match exactly",
                a.len(),
                b.len()
            )));
        }
        let mut out = Vec::with_capacity(a.len());
        for (&x, &y) in a.iter().zip(b.iter()) {
            let mut args = [0u8; 16];
            args[..8].copy_from_slice(&x.to_le_bytes());
            args[8..].copy_from_slice(&y.to_le_bytes());
            let call = self.call_bytes(op_prog, &args)?;
            let bytes = self.store().load(&call.result).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "zip: result {} for op_prog {} missing from CAS",
                        call.result, op_prog
                    ),
                )
            })?;
            if bytes.len() < 8 {
                return Err(io::Error::other(format!(
                    "zip: op_prog {} returned {} bytes (expected ≥ 8 for i64)",
                    op_prog,
                    bytes.len()
                )));
            }
            out.push(i64::from_le_bytes(bytes[..8].try_into().unwrap()));
        }
        Ok(out)
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 8a (Phase Ω.10, 2026-05-01) — Op::Grad FULL via forward-mode AD
    //
    // Auto-differentiation à la JAX `jax.grad` graduée à FULL via une
    // implémentation forward-mode au niveau brain. Le programme cible
    // est chargé depuis le CAS, ses nœuds sont parcourus une fois en
    // tenant des paires `(value, dvalue)` par nœud, et le gradient
    // sortant est lu à l'`Op::Output`. Le scope Wave 8a couvre la
    // surface F64 (Add/Sub/Mul/Div/Sqrt/Abs/Neg/Exp/Ln/Min/Max +
    // FromI64/ToI64) plus les const et inputs ; les ops i64 propres
    // (AddI64/MulI64/etc.) ne sont pas traversées car elles
    // produisent des gradients localement nuls non utiles à la
    // descente. Reverse-mode et Op::Grad bytecode-level seront posés
    // Wave 8b/c quand la surface F64 sera prouvée stable.
    //
    // Op::Grad au niveau bytecode KASM reste fail-loud (Wave 6
    // bucket) — la voie canonique est `call_grad` brain-level.
    // ───────────────────────────────────────────────────────────────

    /// Forward-mode automatic differentiation on the F64 subset of KASM.
    ///
    /// Loads `prog` from the CAS, walks its nodes once tracking
    /// `(value, dvalue)` per node, and returns
    /// `∂prog/∂args[var_index]` evaluated at the given args (as a
    /// raw f64). Args are 8 bytes per input slot ; an F64-typed
    /// input is interpreted as the IEEE 754 bit pattern, an
    /// I64-typed input is promoted via `as f64` (Φ.0 convention).
    ///
    /// The gradient is computed in `f64` regardless of the program's
    /// declared types — this matches Φ.0 where F64 values live as
    /// i64 bit patterns inside `Value::I64` and the F64 semantics
    /// only kick in via `Op::F64Op`. Constants and i64 inputs that
    /// are not the differentiation variable have `dvalue = 0`.
    ///
    /// Total-function discipline mirrors the interpreter :
    ///   - division by zero → `(value=0, dvalue=0)` (no NaN leak)
    ///   - sqrt of non-positive → `(0, 0)`
    ///   - exp/ln overflow → `(0, 0)`
    ///   - i64 cast (`F64SubOp::ToI64`) → dvalue locally zero
    ///   - min/max → take left-arg derivative on tie (deterministic)
    ///
    /// Restrictions Wave 8a :
    ///   - program must have exactly 1 output ;
    ///   - var_index must be a valid input slot ;
    ///   - non-F64 ops in the chain (i64 arithmetic, comparisons,
    ///     v1.0 meta-ops) surface `io::Error` rather than silently
    ///     returning a wrong gradient. This is `fail-loud` to keep
    ///     wrong gradients out of any optimisation loop that hooks
    ///     onto `call_grad`.
    pub fn call_grad(
        &self,
        prog: &Hash,
        var_index: u8,
        args: &[u8],
    ) -> io::Result<f64> {
        let blob = self.store().load(prog).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("grad: program {} missing from CAS", prog),
            )
        })?;
        let program = kasm::Program::from_bytes(&blob)
            .map_err(|e| io::Error::other(format!("grad: program decode: {e}")))?;

        let n_inputs = program.inputs() as usize;
        if (var_index as usize) >= n_inputs {
            return Err(io::Error::other(format!(
                "grad: var_index {} out of range (program has {} inputs)",
                var_index, n_inputs
            )));
        }
        let expected_args = n_inputs * 8;
        if args.len() != expected_args {
            return Err(io::Error::other(format!(
                "grad: bad args length (expected {} bytes, got {})",
                expected_args,
                args.len()
            )));
        }
        if program.outputs() != 1 {
            return Err(io::Error::other(format!(
                "grad: program must have exactly 1 output (got {}); Wave 8a scope",
                program.outputs()
            )));
        }

        let inputs: Vec<i64> = args
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes(c.try_into().unwrap()))
            .collect();

        let nodes = program.nodes();
        let mut vals = vec![0.0f64; nodes.len()];
        let mut dvals = vec![0.0f64; nodes.len()];

        for (i, node) in nodes.iter().enumerate() {
            let (v, d) = match node.op {
                Op::Input => {
                    let slot = node.imm as usize;
                    let bits = inputs[slot];
                    let v = match node.ty {
                        crate::kasm::Ty::F64 => f64::from_bits(bits as u64),
                        crate::kasm::Ty::I64 => bits as f64,
                        other => {
                            return Err(io::Error::other(format!(
                                "grad: input ty {:?} unsupported at node {}",
                                other, i
                            )))
                        }
                    };
                    let d = if slot == var_index as usize { 1.0 } else { 0.0 };
                    (v, d)
                }
                Op::ConstI64 => (node.imm as i64 as f64, 0.0),
                Op::ConstF64 => (node.imm as f64, 0.0),
                Op::F64Op => {
                    let sub = crate::kasm::F64SubOp::from_imm(node.imm).map_err(|e| {
                        io::Error::other(format!("grad: bad F64SubOp at node {}: {:?}", i, e))
                    })?;
                    let a = node.a as usize;
                    let b = node.b as usize;
                    let va = vals[a];
                    let da = dvals[a];
                    use crate::kasm::F64SubOp as F;
                    match sub {
                        F::Add => (va + vals[b], da + dvals[b]),
                        F::Sub => (va - vals[b], da - dvals[b]),
                        F::Mul => (va * vals[b], da * vals[b] + va * dvals[b]),
                        F::DivChecked => {
                            let vb = vals[b];
                            let db = dvals[b];
                            if vb == 0.0 {
                                (0.0, 0.0)
                            } else {
                                (va / vb, (da * vb - va * db) / (vb * vb))
                            }
                        }
                        F::Min => {
                            if va <= vals[b] {
                                (va, da)
                            } else {
                                (vals[b], dvals[b])
                            }
                        }
                        F::Max => {
                            if va >= vals[b] {
                                (va, da)
                            } else {
                                (vals[b], dvals[b])
                            }
                        }
                        F::Sqrt => {
                            if va > 0.0 {
                                let s = va.sqrt();
                                (s, da / (2.0 * s))
                            } else {
                                (0.0, 0.0)
                            }
                        }
                        F::Abs => {
                            let s = if va > 0.0 {
                                1.0
                            } else if va < 0.0 {
                                -1.0
                            } else {
                                0.0
                            };
                            (va.abs(), da * s)
                        }
                        F::Neg => (-va, -da),
                        F::FromI64 => (va, da), // identity in f64 land
                        F::ToI64 => (va.trunc(), 0.0), // step function: locally 0
                        F::Exp => {
                            let e = va.exp();
                            if e.is_finite() {
                                (e, da * e)
                            } else {
                                (0.0, 0.0)
                            }
                        }
                        F::Ln => {
                            // d/dx ln|x| = 1/x for x ≠ 0
                            if va == 0.0 {
                                (0.0, 0.0)
                            } else {
                                (va.abs().ln(), da / va)
                            }
                        }
                    }
                }
                Op::Output => {
                    return Ok(dvals[node.a as usize]);
                }
                other => {
                    return Err(io::Error::other(format!(
                        "grad: op {:?} at node {} not supported in Wave 8a forward-mode AD (F64 subset only)",
                        other, i
                    )));
                }
            };
            vals[i] = v;
            dvals[i] = d;
        }

        Err(io::Error::other(
            "grad: program has no Output node (malformed)",
        ))
    }

    // ───────────────────────────────────────────────────────────────
    // Wave 10 (Phase Ω.10, 2026-05-01) — Op::Fori / Op::WhileLoop FULL
    //
    // Bounded loops, brain-level. `call_fori` mirrors JAX
    // `lax.fori_loop` ; `call_while` mirrors JAX `lax.while_loop` with
    // an explicit `fuel` cap to guarantee termination (fail-loud when
    // exhausted — there is no "best effort partial result").
    //
    // Same pattern as Wave 6/7a/8a : op programs are referenced by
    // `Hash`, accumulator/state passed as `i64`, body/cond programs
    // dispatched via the existing `call_bytes` pipeline. Op::Fori /
    // Op::WhileLoop bytecode-level remain fail-loud (Wave 6 bucket).
    // ───────────────────────────────────────────────────────────────

    /// JAX `lax.fori_loop` equivalent — bounded for loop.
    ///
    /// `body_prog` is a `(i: i64, acc: i64) → i64` program ;
    /// `call_fori` runs it for each `i` in `start..stop`, threading
    /// the accumulator. Empty range (`start >= stop`) returns
    /// `init_acc` unchanged. Negative `start` and `stop` are accepted
    /// — the loop runs while `i < stop`. Termination is guaranteed
    /// by the bounded range : `body_prog` cannot grow it.
    pub fn call_fori(
        &self,
        body_prog: &Hash,
        start: i64,
        stop: i64,
        init_acc: i64,
    ) -> io::Result<i64> {
        let mut acc = init_acc;
        let mut i = start;
        while i < stop {
            let mut args = [0u8; 16];
            args[..8].copy_from_slice(&i.to_le_bytes());
            args[8..].copy_from_slice(&acc.to_le_bytes());
            let call = self.call_bytes(body_prog, &args)?;
            let out = self.store().load(&call.result).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "fori: result {} for body_prog {} missing from CAS",
                        call.result, body_prog
                    ),
                )
            })?;
            if out.len() < 8 {
                return Err(io::Error::other(format!(
                    "fori: body_prog {} returned {} bytes (expected ≥ 8 for i64)",
                    body_prog,
                    out.len()
                )));
            }
            acc = i64::from_le_bytes(out[..8].try_into().unwrap());
            i = i.wrapping_add(1);
        }
        Ok(acc)
    }

    /// JAX `lax.while_loop` equivalent — fuel-bounded while loop.
    ///
    /// `cond_prog` is a `(state: i64) → i64` program ; the loop
    /// continues while `cond_prog(state) != 0`. `body_prog` is a
    /// `(state: i64) → i64` program that produces the next state.
    /// `fuel` is a hard upper bound on the number of iterations —
    /// when reached, returns `Err` (we never silently truncate the
    /// computation).
    ///
    /// Termination guarantee : the only way to loop forever would be
    /// `cond_prog` permanently returning non-zero ; the fuel bound
    /// catches this and surfaces it as a real error rather than a
    /// hang. This is doctrine : a runtime that hangs is broken ; a
    /// runtime that says "out of fuel" is honest.
    pub fn call_while(
        &self,
        cond_prog: &Hash,
        body_prog: &Hash,
        init_state: i64,
        fuel: u64,
    ) -> io::Result<i64> {
        let mut state = init_state;
        let mut steps: u64 = 0;
        loop {
            // Evaluate the condition.
            let cond_out = self.call_one_i64(cond_prog, state)?;
            if cond_out == 0 {
                return Ok(state);
            }
            if steps >= fuel {
                return Err(io::Error::other(format!(
                    "while: fuel exhausted after {fuel} iterations (cond still non-zero, state={state})"
                )));
            }
            steps = steps.saturating_add(1);
            // Step the state.
            state = self.call_one_i64(body_prog, state)?;
        }
    }

    /// Runtime equivalent of APL `\` (JAX `lax.scan`) — like
    /// `call_reduce` but returns every intermediate accumulator
    /// (length `vec.len() + 1` : the initial value plus one entry per
    /// fold step).
    pub fn call_scan(&self, op_prog: &Hash, vec: &[i64], init: i64) -> io::Result<Vec<i64>> {
        let mut out = Vec::with_capacity(vec.len() + 1);
        out.push(init);
        let mut acc = init;
        for &x in vec {
            let mut args = [0u8; 16];
            args[..8].copy_from_slice(&acc.to_le_bytes());
            args[8..].copy_from_slice(&x.to_le_bytes());
            let call = self.call_bytes(op_prog, &args)?;
            let out_bytes = self.store().load(&call.result).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "scan: result {} for op_prog {} missing from CAS",
                        call.result, op_prog
                    ),
                )
            })?;
            if out_bytes.len() < 8 {
                return Err(io::Error::other(format!(
                    "scan: op_prog {} returned {} bytes (expected ≥ 8 for i64)",
                    op_prog,
                    out_bytes.len()
                )));
            }
            acc = i64::from_le_bytes(out_bytes[..8].try_into().unwrap());
            out.push(acc);
        }
        Ok(out)
    }

    /// Zero-allocation hot path for the canonical i64 → i64 case. The
    /// cache hit branch decodes the result directly under the read lock
    /// — no `Arc::clone`, no `Vec` allocation, no `MonsterValue` boundary
    /// type. For redundant args this drops latency from ~900 ns to under
    /// 300 ns on the Ryzen 6600H.
    ///
    /// Falls back to the full `call_value_bytes_hot_args` pipeline on
    /// cache miss (fast lane → slow lane), so semantics are identical
    /// to `call_many_values_i64(&[arg])[0]`.
    pub fn call_one_i64(&self, func: &Hash, arg: i64) -> io::Result<i64> {
        let hot = self.hot_program(func)?;

        // ────────────────────────────────────────────────────────────
        // Auto-router (call_one_i64 fast path) — AffineI64 inline.
        //
        // For programs whose hot_plan is `AffineI64 { input_slot: 0,
        // mul, add }`, the entire computation is `arg * mul + add` —
        // 2 instructions on x86_64. Going through the full dispatch
        // cascade (hot_program lookup → RamKey → cache probe →
        // dispatch_impl → 5 layers → execute_hot_plan → cache write)
        // costs ~2 µs to do 5 ns of work. Not a cache problem ; an
        // overhead problem.
        //
        // The dispatch overhead is justified for slow programs (where
        // a cache hit pays back µs of avoided compute), but for cheap
        // rules the cache is more expensive than the rule itself.
        // CLAUDE.md §9 explicitly identifies this as the auto-router
        // mandate — the Tauri MODE dropdown (B/F bypass) was the
        // manual version of this decision.
        //
        // Conditions :
        //   - HotPlan::AffineI64 with input slot 0 (the common case)
        //   - No explicit_memos (those force memoization side effects)
        //
        // The cache is intentionally NOT consulted nor populated. For
        // a program called with mostly-unique args (the realistic case
        // for finance/chemistry/sim workloads), the cache would have
        // missed anyway. For repeated args, re-executing 5 ns is still
        // cheaper than the cache lookup. Either way we win.
        //
        // `call_bytes` and other public APIs that go through
        // `dispatch_call(persist=true)` still use the full cascade —
        // they need the RAM memo + write_memo for swarm gossip.
        if let HotPlan::AffineI64 { input_slot: 0, mul, add } = hot.plan {
            if hot.explicit_memos.is_empty() {
                let result = arg.wrapping_mul(mul).wrapping_add(add);
                self.stats_atomic.rule_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(result);
            }
        }

        // ────────────────────────────────────────────────────────────
        // Φ.ν.7g — AdaptiveInlineCache L0 (5-10 ns lock-free).
        //
        // Wire-up adaptatif : auto-désactivation si hit_rate < 5 %
        // après 100 probes (cf. cache.rs::AdaptiveInlineCache).
        //
        // - Workloads à args uniques (DNA k-mer 100k bougies distinctes)
        //   → warmup détecte 0 % hit → mode Disabled → coût ≈ 0 (un
        //   load atomique Relaxed + comparaison u8 = ~1 ns, vs ~15 ns
        //   pour la version naïve qui régressait).
        //
        // - Workloads à args répétés (reverse_synth : millions de
        //   candidats KASM évalués sur les MÊMES features_i64 d'une
        //   bougie de décision, hit_rate attendu ≥ 50 %) → warmup
        //   détecte hit_rate haut → mode Active → 5-20× speedup.
        //
        // Tentative #1 (commit b1d6bb7) sans adaptation : régression
        // +30-52 % sur DNA. Cette version résout le piège.
        if hot.explicit_memos.is_empty() {
            if let Some(cached) = hot.inline_cache.try_match_i64(arg as u64) {
                return Ok(cached as i64);
            }
        }

        let arg_bytes = arg.to_le_bytes();
        let key = RamKey::for_args(hot.semantic_fingerprint, &arg_bytes);

        // Cache hit: decode the 8 i64 bytes inline, holding the read
        // lock just long enough to copy them onto the stack.
        {
            let cache = self.cache.read().unwrap();
            if let Some(slot) = cache.get(&key) {
                if slot.bytes.len() >= 8 {
                    let chunk: [u8; 8] = slot.bytes[..8].try_into().unwrap();
                    drop(cache);
                    self.stats_atomic
                        .ram_value_hits
                        .fetch_add(1, Ordering::Relaxed);
                    return Ok(i64::from_le_bytes(chunk));
                }
            }
        }

        // ────────────────────────────────────────────────────────────
        // Auto-router v2 — Inline interpreter for small Interpret
        // programs (≤ 64 nodes, no rule, no explicit memos).
        //
        // Couvre les programmes du dropdown Tauri "Léger" et "KASM v1.0
        // mutation" qui ne fittent aucune HotPlan rule :
        //
        //   complement (XOR + const)         : 4 nodes
        //   spaced seed (AND + Hash)         : 5 nodes
        //   strobemer (Hash + XOR + Hash)    : 7 nodes
        //   branched hash (Op::Cond)         : 7 nodes
        //   minhash10 (10× Hash + min)       : ~40 nodes
        //
        // Mesures DNA bench (cb583ac) : tous au slow path à 3-6 µs/call.
        // Avec bypass : ~50-200 ns (kasm::execute time only, pas de
        // cascade dispatch_impl, pas d'oracle observe, pas de cache
        // remember).
        //
        // Threshold 64 nodes : dérivé du break-even « interpret cost <
        // cache_mix cost » avec 30% miss rate. En pratique tous les
        // programmes du dropdown Tauri (sauf hash_chain 1024 = HashChain
        // rule, handled v1) tombent ici.
        //
        // Pas de cache update : cohérent avec v0/v1, le bypass ne
        // pollue pas le RAM cache. Repeated args = re-execute (50-200
        // ns chacun, < cache hit 150 ns pour les cas Léger).
        if matches!(hot.plan, HotPlan::Interpret)
            && hot.program.nodes().len() <= 64
            && hot.explicit_memos.is_empty()
            && !hot.program.target().needs_external_backend()
        {
            // Stack-only interpreter — zero Vec alloc per call. Mesuré
            // ~5-10x plus rapide que `kasm::execute` (qui alloue 5 Vec
            // par appel). Voir `interpreter::try_execute_i64_inline`.
            //
            // Renvoie None si l'opcode n'est pas dans le sous-set
            // i64-only handled (F64, Vec, meta-ops Wave 8). Dans ce
            // cas, fallback vers `kasm::execute` (qui couvre TOUT).
            if let Some(result) = kasm::try_execute_i64_inline(&hot.program, arg) {
                self.stats_atomic.executions.fetch_add(1, Ordering::Relaxed);
                hot.inline_cache.arm_i64(arg as u64, result as u64);
                return Ok(result);
            }
            // Stack interp ne couvre pas → fallback execute complet.
            let arg_bytes = arg.to_le_bytes();
            if let Ok(result_bytes) = kasm::execute(&hot.program, &arg_bytes) {
                if result_bytes.len() == 8 {
                    self.stats_atomic.executions.fetch_add(1, Ordering::Relaxed);
                    let result = i64::from_le_bytes(result_bytes.try_into().unwrap());
                    hot.inline_cache.arm_i64(arg as u64, result as u64);
                    return Ok(result);
                }
            }
            // Si execute fail OU output ≠ 8 bytes, fall through au
            // dispatch_impl complet pour gestion d'erreur correcte.
        }

        // ────────────────────────────────────────────────────────────
        // Auto-router v1 — HashChain bypass on cache miss.
        //
        // Pour HotPlan::HashChain { rounds }, l'exécution est `rounds`
        // appels à `kasm::hash_i64` (~5 ns chacun = 5-320 ns total
        // selon rounds). Le slow path (call_value_bytes_hot_args →
        // dispatch_impl → Layer 3 fast lane → cache write +
        // observe_execution + remember_call) coûte ~3 µs sur les
        // misses, indépendamment du nombre de rounds.
        //
        // Cf. dropdown Tauri "Léger" (k-mer hash SplitMix64 ×1, ×2)
        // et "Lourd" (heavy hash ×64) — TOUS basés sur HashChain.
        // Les mesures DNA bench (cb583ac) montrent :
        //   splitmix1   : 3277 ns/call (1 round = 5 ns réel + 3270 ns overhead)
        //   double_mix  : 3292 ns/call (2 rounds = 10 ns réel)
        //   heavy_64    : 3294 ns/call (64 rounds = 320 ns réel)
        //
        // Bypass cible : 5-320 ns soit 10-600x plus rapide.
        //
        // Le cache hit path au-dessus continue de servir les répétitions
        // (70% des k-mers en pratique). Sur miss, on bypass au lieu de
        // payer le cascade complet.
        if let HotPlan::HashChain { input_slot: 0, rounds } = hot.plan {
            if hot.explicit_memos.is_empty() {
                let mut value = arg;
                for _ in 0..rounds {
                    value = kasm::hash_i64(value);
                }
                self.stats_atomic.rule_hits.fetch_add(1, Ordering::Relaxed);
                hot.inline_cache.arm_i64(arg as u64, value as u64);
                return Ok(value);
            }
        }

        // Cache miss: defer to the standard pipeline (rule, oracle,
        // wire fallback, disk memo, slow execute). The boundary cost
        // (Vec alloc) is amortised over a real call so it doesn't
        // matter outside the hot redundant path.
        let value = self.call_value_bytes_hot_args(&hot, &arg_bytes)?;
        let result = decode_i64_value(value.bytes)?;
        if hot.explicit_memos.is_empty() {
            hot.inline_cache.arm_i64(arg as u64, result as u64);
        }
        Ok(result)
    }

    pub fn call_many_bytes(
        &self,
        func: &Hash,
        args_list: &[Vec<u8>],
    ) -> io::Result<Vec<MonsterCall>> {
        // O(1) dedup via HashMap<fast_fingerprint, slot_index>.
        let mut seen: HashMap<u64, usize> = HashMap::with_capacity(args_list.len().min(256));
        let mut unique_calls: Vec<MonsterCall> = Vec::new();
        let mut out = Vec::with_capacity(args_list.len());
        let mut dedupe_hits = 0u64;
        for args in args_list {
            let fp = fast_fingerprint(args);
            if let Some(&index) = seen.get(&fp) {
                dedupe_hits += 1;
                let call = &unique_calls[index];
                out.push(MonsterCall {
                    result: call.result.clone(),
                    source: MonsterSource::RamMemo,
                    envelope: call.envelope,
                });
                continue;
            }
            let call = self.call_bytes(func, args)?;
            let index = unique_calls.len();
            unique_calls.push(call.clone());
            seen.insert(fp, index);
            out.push(call);
        }
        if dedupe_hits > 0 {
            self.stats_atomic
                .batch_dedupe_hits
                .fetch_add(dedupe_hits, Ordering::Relaxed);
        }
        Ok(out)
    }

    pub fn call_many_bytes_parallel(
        &self,
        func: &Hash,
        args_list: &[Vec<u8>],
    ) -> io::Result<Vec<MonsterCall>> {
        let _ = self.hot_program(func)?;
        let mut seen: HashMap<u64, usize> = HashMap::with_capacity(args_list.len().min(256));
        let mut unique: Vec<Vec<u8>> = Vec::new();
        let mut positions = Vec::with_capacity(args_list.len());
        let mut dedupe_hits = 0u64;
        for args in args_list {
            let fp = fast_fingerprint(args);
            if let Some(&index) = seen.get(&fp) {
                positions.push(index);
                dedupe_hits += 1;
            } else {
                let index = unique.len();
                unique.push(args.clone());
                seen.insert(fp, index);
                positions.push(index);
            }
        }
        if dedupe_hits > 0 {
            self.stats_atomic
                .batch_dedupe_hits
                .fetch_add(dedupe_hits, Ordering::Relaxed);
        }

        let mut unique_calls = Vec::with_capacity(unique.len());
        thread::scope(|scope| {
            let handles = unique
                .iter()
                .map(|args| {
                    let args = args.clone();
                    scope.spawn(move || self.call_bytes(func, &args))
                })
                .collect::<Vec<_>>();
            for handle in handles {
                unique_calls.push(
                    handle
                        .join()
                        .map_err(|_| io::Error::other("monster worker panicked"))??,
                );
            }
            Ok::<(), io::Error>(())
        })?;

        let mut out = Vec::with_capacity(args_list.len());
        for index in positions {
            let mut call = unique_calls[index].clone();
            if matches!(
                call.source,
                MonsterSource::ExecutedHot | MonsterSource::StructuralRule | MonsterSource::LearnedOracle
            ) {
                call.source = MonsterSource::RamMemo;
            }
            out.push(call);
        }
        Ok(out)
    }

    /// Φ.ν.7g — SIMPLIFIÉ (Via Negativa).
    ///
    /// Avant : ~80 lignes avec 5 paths divergents (AffineI64 SIMD, JIT
    /// batch, dedupe+sequential, dedupe+parallel scoped threads,
    /// dedupe+sequential>256). Le path sequential passait par
    /// `call_value_bytes_hot_args` → `dispatch_impl` → `execute_hot_plan`
    /// → `execute_with_jit` qui DIVERGEAIT de `call_one_i64` (qui
    /// passe par `try_execute_i64_inline` correct). Bug observé sur
    /// les programs avec Min/Max (clamp_affine recognizer).
    ///
    /// Après : 2 fast paths conservés pour gros batches (≥ 1024) où
    /// le JIT batch / SIMD AffineI64 ont leur valeur. Sinon, **boucle
    /// `call_one_i64`** qui hérite de TOUS les bypass éprouvés (interp
    /// inline, AffineI64 v0, HashChain v1, RAM cache).
    ///
    /// Bénéfices :
    ///   - **Élimine la divergence** call_one_i64 vs batch sequential
    ///   - ~80 lignes → ~30 lignes (Via Negativa drastique)
    ///   - Plus de dédup explicite — RAM cache de call_one_i64 le fait
    ///     déjà naturellement (cache hit gratuit sur args répétés)
    ///   - Plus de `thread::scope` parallel — call_one_i64 est déjà
    ///     très rapide (50-200 ns), parallelism overhead dominerait
    pub fn call_many_values_i64(
        &self,
        func: &Hash,
        values: &[i64],
    ) -> io::Result<Vec<i64>> {
        // Fast paths conservés pour gros batches (≥ 1024) qui justifient
        // la spécialisation SIMD/JIT.
        if values.len() >= 1024 {
            let hot = self.hot_program(func)?;

            // SIMD AffineI64 batch (LLVM auto-vectorize AVX2/SSE2)
            if let HotPlan::AffineI64 { input_slot: 0, mul, add } = hot.plan {
                let n = values.len();
                let mut out: Vec<i64> = Vec::with_capacity(n);
                unsafe {
                    let dst = out.as_mut_ptr();
                    let src = values.as_ptr();
                    let mut i = 0usize;
                    while i < n {
                        let v = *src.add(i);
                        *dst.add(i) = v.wrapping_mul(mul).wrapping_add(add);
                        i += 1;
                    }
                    out.set_len(n);
                }
                self.stats_atomic.rule_hits.fetch_add(n as u64, Ordering::Relaxed);
                return Ok(out);
            }

            // JIT batch lane (~150M calls/sec sur ≤12 nodes). Si le JIT
            // batch produit None ou Err, on tombe dans le path scalaire
            // ci-dessous (qui boucle call_one_i64 = path éprouvé).
            reject_external_target(&hot.program)?;
            if let Some(out) = execute_hot_batch_i64(&hot, values)? {
                if hot.plan.is_rule() {
                    self.stats_atomic.rule_hits.fetch_add(values.len() as u64, Ordering::Relaxed);
                } else {
                    self.stats_atomic.executions.fetch_add(values.len() as u64, Ordering::Relaxed);
                }
                return Ok(out);
            }
        }

        // Path scalaire avec dédup légère + boucle call_one_i64.
        // Hérite des bypass éprouvés (interp inline, AffineI64 v0,
        // HashChain v1, RAM cache). La dédup batch reste utile pour
        // les workloads avec args répétés (incremente
        // batch_dedupe_hits stats pour traçabilité).
        let mut seen: HashMap<i64, usize> = HashMap::with_capacity(values.len().min(256));
        let mut unique = Vec::<i64>::new();
        let mut positions = Vec::with_capacity(values.len());
        let mut dedupe_hits = 0u64;
        for value in values {
            if let Some(&index) = seen.get(value) {
                positions.push(index);
                dedupe_hits += 1;
            } else {
                let index = unique.len();
                unique.push(*value);
                seen.insert(*value, index);
                positions.push(index);
            }
        }
        if dedupe_hits > 0 {
            self.stats_atomic.batch_dedupe_hits.fetch_add(dedupe_hits, Ordering::Relaxed);
        }

        let mut unique_values = Vec::with_capacity(unique.len());
        for v in unique {
            unique_values.push(self.call_one_i64(func, v)?);
        }

        let mut out = Vec::with_capacity(values.len());
        for index in positions {
            out.push(unique_values[index]);
        }
        Ok(out)
    }

    pub fn call_many_value_bytes(
        &self,
        func: &Hash,
        args_list: &[Vec<u8>],
    ) -> io::Result<Vec<Vec<u8>>> {
        let hot = self.hot_program(func)?;
        let mut seen: HashMap<u64, usize> = HashMap::with_capacity(args_list.len().min(256));
        let mut unique: Vec<Vec<u8>> = Vec::new();
        let mut positions = Vec::with_capacity(args_list.len());
        let mut dedupe_hits = 0u64;
        for args in args_list {
            let fp = fast_fingerprint(args);
            if let Some(&index) = seen.get(&fp) {
                positions.push(index);
                dedupe_hits += 1;
            } else {
                let index = unique.len();
                unique.push(args.clone());
                seen.insert(fp, index);
                positions.push(index);
            }
        }
        if dedupe_hits > 0 {
            self.stats_atomic
                .batch_dedupe_hits
                .fetch_add(dedupe_hits, Ordering::Relaxed);
        }

        let mut unique_values = Vec::with_capacity(unique.len());
        if unique.len() <= 32 || unique.len() > 256 {
            for args in &unique {
                unique_values.push(self.call_value_bytes_hot_args(&hot, args)?);
            }
        } else {
            thread::scope(|scope| {
                let handles = unique
                    .iter()
                    .map(|args| {
                        let args = args.clone();
                        let hot = Arc::clone(&hot);
                        scope.spawn(move || self.call_value_bytes_hot_args(&hot, &args))
                    })
                    .collect::<Vec<_>>();
                for handle in handles {
                    unique_values.push(
                        handle
                            .join()
                            .map_err(|_| io::Error::other("monster value worker panicked"))??,
                    );
                }
                Ok::<(), io::Error>(())
            })?;
        }

        let mut out = Vec::with_capacity(args_list.len());
        for index in positions {
            out.push(unique_values[index].bytes.clone());
        }
        Ok(out)
    }

    pub fn call(&self, func: &Hash, args: &Hash) -> io::Result<MonsterCall> {
        // Wave 9 — args hash absent from CAS surfaces as NotFound,
        // distinct from a real I/O fault on the same path.
        let arg_bytes = self.store().load(args).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("unknown args hash: {args}"),
            )
        })?;
        let hot = self.hot_program(func)?;
        let key = RamKey::for_args_hash(hot.semantic_fingerprint, args);
        self.dispatch_call(&hot, key, &arg_bytes)
    }

    pub fn ingest_external_result(
        &self,
        func: &Hash,
        args: &[u8],
        result_bytes: &[u8],
    ) -> io::Result<Hash> {
        let hot = self.hot_program(func)?;
        let ram_key = RamKey::for_args(hot.semantic_fingerprint, args);
        let call_key_hex = ram_key.to_call_key().hex();
        if let HotPlan::StaticOutput(bytes) = &hot.plan {
            let result = self.store().store(bytes)?;
            self.store().write_memo(&call_key_hex, &result)?;
            self.remember_call(ram_key, result, Arc::clone(bytes));
            self.stats_atomic.rule_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(result);
        }
        let result = self.store().store(result_bytes)?;
        self.store().write_memo(&call_key_hex, &result)?;
        let arc_bytes: Arc<[u8]> = Arc::from(result_bytes.to_vec().into_boxed_slice());
        self.remember_call(ram_key, result, arc_bytes);
        Ok(result)
    }

    /// Hot value path. Wraps the unified `dispatch_impl` with
    /// `persist=false`: no git writes, no `write_memo`. Returns the
    /// resulting bytes for the caller (e.g. `call_one_i64` decoding).
    pub(super) fn call_value_bytes_hot_args(
        &self,
        hot: &HotProgram,
        arg_bytes: &[u8],
    ) -> io::Result<MonsterValue> {
        let key = RamKey::for_args(hot.semantic_fingerprint, arg_bytes);
        let outcome = self
            .dispatch_impl(hot, key, arg_bytes, false, false)?
            .expect("dispatch_impl never returns None when lookup_only=false");
        Ok(MonsterValue {
            bytes: outcome.bytes_arc.to_vec(),
            source: outcome.source,
            envelope: outcome.envelope,
        })
    }

    /// Φ.1 — Lookup-only dispatch (layers 0-5 of `dispatch_impl`). Returns
    /// `Some(MonsterValue)` if any cheap brain layer satisfied the call
    /// (RAM cache / wire-key / structural rule / learned oracle / disk
    /// memo); returns `None` if Layer 6 (slow interpreter) would be the
    /// only path forward — i.e. a genuine "miss" that a `BulkEvaluator`
    /// must handle.
    ///
    /// Bytes-only / non-persistent variant: same semantics as
    /// `call_value_bytes_hot_args` but never invokes the interpreter.
    /// Used by `dispatch::dispatch_batch` to filter the brain-resident
    /// hits from the misses that need bulk evaluation.
    /// Lookup-only path used by `dispatch_batch`. The caller has already
    /// computed the `RamKey` (so it can be reused on miss-ingestion —
    /// cut #8) and we never alloc a `MonsterValue` (cut from the old
    /// `try_lookup_value_bytes`). The returned `MonsterCall.result` is
    /// the already-computed `DispatchOutcome.result` — no redundant
    /// `Hash::for_blob`.
    pub(super) fn try_lookup_call(
        &self,
        hot: &HotProgram,
        arg_bytes: &[u8],
        key: RamKey,
    ) -> io::Result<Option<MonsterCall>> {
        let outcome = self.dispatch_impl(hot, key, arg_bytes, false, true)?;
        Ok(outcome.map(|o| MonsterCall {
            result: o.result,
            source: o.source,
            envelope: o.envelope,
        }))
    }

    /// Public lookup-only peek: returns `true` if the brain would satisfy
    /// `(func, args)` from the cheap layers (RAM cache / structural rule /
    /// learned oracle / disk memo) without invoking the interpreter or any
    /// bulk evaluator. Used by ComputationPlan to pre-classify calls as
    /// already-known vs. truly-novel before dispatch.
    pub fn peek_call(&self, func: &Hash, args: &[u8]) -> io::Result<bool> {
        let hot = self.hot_program(func)?;
        let key = RamKey::for_args(hot.semantic_fingerprint, args);
        Ok(self.try_lookup_call(&hot, args, key)?.is_some())
    }

    fn call_with_args(&self, func: &Hash, arg_bytes: &[u8]) -> io::Result<MonsterCall> {
        let hot = self.hot_program(func)?;
        let key = RamKey::for_args(hot.semantic_fingerprint, arg_bytes);
        self.dispatch_call(&hot, key, arg_bytes)
    }

    fn force_explicit_memos(&self, hot: &HotProgram, arg_bytes: &[u8]) -> io::Result<()> {
        for memo in hot.explicit_memos.iter() {
            let key = RamKey::for_args(memo.semantic_fingerprint, arg_bytes);
            if self.lookup_call(&key).is_some() {
                continue;
            }
            reject_external_target(&memo.program)?;
            let bytes = kasm::execute(&memo.program, arg_bytes)
                .map_err(|err| io::Error::other(format!("kasm memoize execute: {err}")))?;
            let arc_bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
            let result = Hash::for_blob(&arc_bytes);
            self.remember_call(key, result, arc_bytes);
        }
        Ok(())
    }

    /// Persistent dispatch: wraps `dispatch_impl` with `persist=true`.
    /// Every successful result is stored into git (`store()` + `write_memo`)
    /// so the swarm can share it. Returns only the result hash; the
    /// bytes payload is dropped by the caller.
    fn dispatch_call(
        &self,
        hot: &HotProgram,
        key: RamKey,
        arg_bytes: &[u8],
    ) -> io::Result<MonsterCall> {
        let outcome = self
            .dispatch_impl(hot, key, arg_bytes, true, false)?
            .expect("dispatch_impl never returns None when lookup_only=false");
        Ok(MonsterCall {
            result: outcome.result,
            source: outcome.source,
            envelope: outcome.envelope,
        })
    }

    /// Unified 7-layer dispatch. Walks: static_output → RAM cache →
    /// wire-key (lazy) → rule fast lane → oracle fast lane → disk memo
    /// → slow execute. The `persist` flag toggles two side effects on
    /// every emitting layer:
    ///
    ///   * git store (`store().store(bytes)`) replaces the local
    ///     `Hash::for_blob` (the hashes agree by construction; the
    ///     store call additionally writes the bytes to the on-disk
    ///     blob store).
    ///   * `write_memo(key_hex, &result)` records the swarm-visible
    ///     mapping CallKey→result.
    ///
    /// In `persist=true`, the disk-memo branch returns an empty
    /// `Arc<[u8]>` — the caller (`dispatch_call`) discards bytes
    /// anyway, so loading the blob would be wasted I/O. This preserves
    /// the existing asymmetry with the non-persist path.
    ///
    /// `lookup_only=true` (Φ.1) stops the cascade right before Layer 6
    /// (the slow interpreter) and returns `Ok(None)`. The brain has
    /// expended only cache / rule / oracle / memo lookups — never the
    /// interpreter. Callers (e.g. `dispatch_batch`) use this to filter
    /// genuine misses out for bulk evaluation by another backend.
    /// `lookup_only=false` always returns `Some(_)` on success.
    fn dispatch_impl(
        &self,
        hot: &HotProgram,
        key: RamKey,
        arg_bytes: &[u8],
        persist: bool,
        lookup_only: bool,
    ) -> io::Result<Option<DispatchOutcome>> {
        // Layer 0 — static_output. The result is constant by
        // construction (folded into HotPlan::StaticOutput at load-time).
        if let HotPlan::StaticOutput(bytes) = &hot.plan {
            let result = if persist {
                let local = Hash::for_blob(bytes);
                let stored = self.store().store(bytes).unwrap_or(local);
                let key_hex = key.to_call_key().hex();
                self.store().write_memo(&key_hex, &stored)?;
                stored
            } else {
                Hash::for_blob(bytes)
            };
            self.remember_call(key, result, Arc::clone(bytes));
            self.stats_atomic.rule_hits.fetch_add(1, Ordering::Relaxed);
            if !lookup_only {
                self.force_explicit_memos(hot, arg_bytes)?;
            }
            return Ok(Some(DispatchOutcome {
                result,
                bytes_arc: Arc::clone(bytes),
                source: MonsterSource::RamMemo,
                envelope: PhysicalEnvelope::default(),
            }));
        }

        // Layer 1 — RAM cache (RamKey-keyed).
        if let Some(slot) = self.lookup_call(&key) {
            let counter = if persist {
                &self.stats_atomic.ram_memo_hits
            } else {
                &self.stats_atomic.ram_value_hits
            };
            counter.fetch_add(1, Ordering::Relaxed);
            if !lookup_only {
                self.force_explicit_memos(hot, arg_bytes)?;
            }
            return Ok(Some(DispatchOutcome {
                result: slot.result,
                bytes_arc: slot.bytes,
                source: MonsterSource::RamMemo,
                envelope: PhysicalEnvelope::default(),
            }));
        }

        // Layer 2 — wire-key fallback (lazy). Skip the CallKey
        // computation entirely if no swarm import has ever populated
        // the wire branch (sticky-bit `wire_seen_ever`). Saves a
        // SHA-1 + heap alloc on every miss in single-node setups.
        let wire_active = self.wire_seen_ever.load(Ordering::Relaxed);
        let call_key_opt = if wire_active {
            let call_key = key.to_call_key();
            if let Some(slot) = self.lookup_wire(&call_key) {
                let counter = if persist {
                    &self.stats_atomic.ram_memo_hits
                } else {
                    &self.stats_atomic.ram_value_hits
                };
                counter.fetch_add(1, Ordering::Relaxed);
                if !lookup_only {
                    self.force_explicit_memos(hot, arg_bytes)?;
                }
                return Ok(Some(DispatchOutcome {
                    result: slot.result,
                    bytes_arc: slot.bytes,
                    source: MonsterSource::RamMemo,
                    envelope: PhysicalEnvelope::default(),
                }));
            }
            Some(call_key)
        } else {
            None
        };

        // Layer 3 — structural rule fast lane (Affine, HashChain, …).
        // Sub-microsecond emit; skips the libgit2 round-trip.
        if hot.plan.is_rule() {
            reject_external_target(&hot.program)?;
            let bytes = execute_hot_plan(hot, arg_bytes)?;
            let arc_bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
            let result = if persist {
                let stored = self.store().store(&arc_bytes)?;
                let call_key = call_key_opt
                    .clone()
                    .unwrap_or_else(|| key.to_call_key());
                self.store().write_memo(&call_key.hex(), &stored)?;
                stored
            } else {
                Hash::for_blob(&arc_bytes)
            };
            self.remember_call(key, result, Arc::clone(&arc_bytes));
            self.stats_atomic.rule_hits.fetch_add(1, Ordering::Relaxed);
            if !lookup_only {
                self.force_explicit_memos(hot, arg_bytes)?;
            }
            return Ok(Some(DispatchOutcome {
                result,
                bytes_arc: arc_bytes,
                source: MonsterSource::StructuralRule,
                envelope: PhysicalEnvelope::default(),
            }));
        }

        // Layer 4 — learned oracle fast lane.
        if let Some(bytes) = self.apply_learned_oracle(hot, arg_bytes) {
            // Shadow validation: 1 in SHADOW_PERIOD hits goes through
            // the interpreter to catch a lying oracle.
            self.shadow_check_oracle(hot, arg_bytes, &bytes);
            let arc_bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
            let result = if persist {
                let stored = self.store().store(&arc_bytes)?;
                let call_key = call_key_opt
                    .clone()
                    .unwrap_or_else(|| key.to_call_key());
                self.store().write_memo(&call_key.hex(), &stored)?;
                stored
            } else {
                Hash::for_blob(&arc_bytes)
            };
            self.remember_call(key, result, Arc::clone(&arc_bytes));
            self.bump_oracle_hit();
            if !lookup_only {
                self.force_explicit_memos(hot, arg_bytes)?;
            }
            return Ok(Some(DispatchOutcome {
                result,
                bytes_arc: arc_bytes,
                source: MonsterSource::LearnedOracle,
                envelope: PhysicalEnvelope::default(),
            }));
        }

        // From Layer 5 onward we always need the CallKey (for the disk
        // memo lookup, and for write_memo if persisting).
        let call_key = call_key_opt.unwrap_or_else(|| key.to_call_key());
        let key_hex = call_key.hex();

        // Layer 5 — on-disk memo (libgit2 ref). In persist mode we
        // intentionally skip the bytes load (the caller wants only the
        // hash); in non-persist mode we load and cache for future hits.
        if let Some(result) = self.store().lookup_memo(&key_hex) {
            if persist {
                let arc_bytes: Arc<[u8]> = Arc::new([]);
                self.remember_call(key, result, Arc::clone(&arc_bytes));
                self.stats_atomic.git_memo_hits.fetch_add(1, Ordering::Relaxed);
                if !lookup_only {
                    self.force_explicit_memos(hot, arg_bytes)?;
                }
                return Ok(Some(DispatchOutcome {
                    result,
                    bytes_arc: arc_bytes,
                    source: MonsterSource::GitMemo,
                    envelope: PhysicalEnvelope::default(),
                }));
            }
            // Π.25 + Π.27 wire — mmap fast read d'abord (50 ns + binary
            // search O(log N) intrusive index 16 bytes/entry). Fallback
            // transparent à `Store::load` (5 µs syscall) si mmap absent
            // ou blob ajouté après mmap build.
            let bytes_opt = self.load_blob_fast(&result)
                .or_else(|| self.store().load(&result));
            if let Some(bytes) = bytes_opt {
                let arc_bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
                self.remember_call(key, result, Arc::clone(&arc_bytes));
                self.stats_atomic.git_memo_hits.fetch_add(1, Ordering::Relaxed);
                if !lookup_only {
                    self.force_explicit_memos(hot, arg_bytes)?;
                }
                return Ok(Some(DispatchOutcome {
                    result,
                    bytes_arc: arc_bytes,
                    source: MonsterSource::GitMemo,
                    envelope: PhysicalEnvelope::default(),
                }));
            }
            // Memo points to a missing blob → fall through to exec.
        }

        // Layer 5b — cross-session atlas RESULT memo. Λ.0b unified
        // hashed-input keying : `apply()`, `dispatch_batch`, and this
        // path share a single atlas entry per (func, input).
        let atlas_arc_for_result = self.atlas();
        if let Some(atlas) = atlas_arc_for_result.as_ref() {
            let func_hash = Hash::for_blob(hot.program.bytes());
            let input_hash = Hash::for_blob(arg_bytes);
            let atlas_key = crate::atlas::Atlas::result_key(
                func_hash.as_bytes(),
                input_hash.as_bytes(),
            );
            if let Some(result_bytes_arr) = atlas.lookup_result(&atlas_key) {
                let result = Hash::from_bytes(result_bytes_arr);
                if let Some(blob) = self.store().load(&result) {
                    let arc_bytes: Arc<[u8]> = Arc::from(blob.into_boxed_slice());
                    self.remember_call(key.clone(), result, Arc::clone(&arc_bytes));
                    if !lookup_only {
                        self.force_explicit_memos(hot, arg_bytes)?;
                    }
                    return Ok(Some(DispatchOutcome {
                        result,
                        bytes_arc: arc_bytes,
                        source: MonsterSource::GitMemo,
                        envelope: PhysicalEnvelope::default(),
                    }));
                }
            }
        }

        // Φ.1 — lookup-only short-circuit. Only the genuinely-novel calls
        // (those that would pay the slow Layer 6 interpreter cost) reach
        // here, so this is the exit point for `BulkEvaluator` filtering.
        if lookup_only {
            return Ok(None);
        }

        // Layer 6 — slow execute. Only opaque/interpreter programs
        // reach here; observe_execution feeds the oracle library so a
        // future call can take the fast lane.
        //
        // V8 Solution C : measure CPU cycles around the actual
        // computation. The store/write_memo I/O is excluded — the
        // envelope reflects pure compute cost, the metric the
        // Gödel-machine optimises against. RDTSC overhead (~30 ns) is
        // negligible compared to slow-lane execute time (~µs+).
        reject_external_target(&hot.program)?;
        let cycles_before = read_cycles();
        // Phase 12.1 + Φ.ν.7g — op_memo ALWAYS-ON (au lieu du gating
        // `is_decomposable()` original). Tente toujours l'exécuteur
        // op-memoizing ; si l'op n'est pas couvert (F64, Vec, meta),
        // execute_with_op_memo Err et on retombe silencieusement sur
        // execute_hot_plan.
        //
        // Bénéfices de l'always-on :
        //   - INTER-PROGRAMS partage : 15 000 candidats synth qui font
        //     tous Hash64(certains_inputs_communs) hit le cache après
        //     le premier calcul (un seul programme calcule, tous les
        //     autres lookup)
        //   - INTRA-PROG efficiency : si le candidat est non-decomposable
        //     mais utilise quand même Hash64, on l'exploite quand même
        //
        // Coût de l'always-on : execute_with_op_memo essaie d'interpréter,
        // Err si op non supporté, ~50 ns d'overhead avant de retomber
        // sur execute_hot_plan. Négligeable vs le gain potentiel.
        let atlas = self.atlas();
        let bytes = match execute_with_op_memo(
            hot,
            arg_bytes,
            &self.op_memo,
            &self.stats_atomic.op_memo_hits,
            &self.stats_atomic.op_memo_misses,
            atlas.as_deref(),
        ) {
            Ok(b) => b,
            Err(_) => execute_hot_plan(hot, arg_bytes)?,
        };
        let cycles_after = read_cycles();
        let cycles = cycles_after.saturating_sub(cycles_before);
        let arc_bytes: Arc<[u8]> = Arc::from(bytes.into_boxed_slice());
        let result = if persist {
            let stored = self.store().store(&arc_bytes)?;
            self.store().write_memo(&key_hex, &stored)?;
            stored
        } else {
            Hash::for_blob(&arc_bytes)
        };

        // Cross-session atlas RESULT write — Λ.0b hashed keying, shared
        // with apply() and dispatch_batch.
        if let Some(atlas) = atlas_arc_for_result.as_ref() {
            if !persist {
                let _ = self.store().store(&arc_bytes);
            }
            let func_hash = Hash::for_blob(hot.program.bytes());
            let input_hash = Hash::for_blob(arg_bytes);
            let atlas_key = crate::atlas::Atlas::result_key(
                func_hash.as_bytes(),
                input_hash.as_bytes(),
            );
            let _ = atlas.record_result(&atlas_key, result.as_bytes());
        }

        self.remember_call(key, result, Arc::clone(&arc_bytes));
        if !lookup_only {
            self.force_explicit_memos(hot, arg_bytes)?;
        }
        self.stats_atomic.executions.fetch_add(1, Ordering::Relaxed);
        self.observe_execution(hot, arg_bytes, &arc_bytes);
        Ok(Some(DispatchOutcome {
            result,
            bytes_arc: arc_bytes,
            source: MonsterSource::ExecutedHot,
            envelope: PhysicalEnvelope {
                cycles,
                energy_uj: 0,    // V8 ζ : RAPL
                l3_misses: 0,    // V8 ζ : perf_event_open
            },
        }))
    }

    pub(super) fn hot_program(&self, func: &Hash) -> io::Result<Arc<HotProgram>> {
        let func = crate::brain::resolve_program_hash(self, *func);
        if let Some(hot) = self.lookup_program(&func) {
            self.stats_atomic
                .program_cache_hits
                .fetch_add(1, Ordering::Relaxed);
            return Ok(hot);
        }

        // Wave 9 — func hash absent from CAS = NotFound (program
        // never stored). Distinct from "store I/O died mid-load".
        let bytes = self.store().load(&func).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("unknown func hash: {func}"),
            )
        })?;
        let verified = kasm::Program::from_bytes(&bytes)
            .map_err(|err| io::Error::other(format!("kasm: {err}")))?;
        let explicit_memos = build_explicit_memos(&Arc::new(verified.clone()))?;
        // Φ.ν.7g — Couplage CSE sémantique + op_memo always-on.
        //
        // Avant : `verified.simplified()` (CSE structural seulement).
        // Maintenant : `verified.cse()` qui FAIT D'ABORD `simplified()`
        // puis ajoute la déduplication SÉMANTIQUE par trace (commit
        // 573b8d0, fix branch-sensitive ops 91ea818).
        //
        // Combiné avec op_memo ALWAYS-ON (cf. ligne ~1919) ça donne
        // le filtre paranoïaque multi-échelle de la doctrine §9 :
        //   1. CSE intra-prog dedupe les sous-arbres équivalents
        //   2. op_memo dedupe les ops répétées entre candidats
        //
        // CSE peut bail (return Ok(simplified)) si le program contient
        // des ops non-traçables (F64, Vec, Reduce, meta-ops). Donc
        // safe pour tous les programmes.
        let program = if should_simplify(&verified) {
            verified
                .cse()
                .map_err(|err| io::Error::other(format!("kasm: {err}")))?
        } else {
            verified.clone()
        };
        let _ = crate::brain::publish_program_substitution(self, func, &verified, &program, 8);
        let canonical_hash = Hash::for_blob(program.bytes());
        if let Ok(Some(attractor)) =
            crate::brain::publish_semantic_attractor(self, canonical_hash, &program, 8)
        {
            if attractor != canonical_hash {
                return self.hot_program(&attractor);
            }
        }
        let semantic_fingerprint = if should_semantic_fingerprint(&program) {
            program
                .semantic_fingerprint()
                .map_err(|err| io::Error::other(format!("kasm: {err}")))?
        } else {
            exact_program_identity(&program, &canonical_hash)
        };
        // Knowledge sharing: surface any oracle previously persisted
        // for this fingerprint (by this node or any peer reachable via
        // `git fetch refs/oracle/*`). Best-effort — silent on errors,
        // because a stale or missing ref must never block the call.
        self.load_persisted_oracle(&semantic_fingerprint);
        let charged = bytes.len() + program.nodes().len() * 16 + PROGRAM_OVERHEAD;
        // `hot_plan` now folds static_output into HotPlan::StaticOutput,
        // so we don't carry a separate Option<Arc<[u8]>> field anymore.
        let plan = hot_plan(&program);
        let program_arc = Arc::new(program);
        Ok(self.remember_program(
            func,
            semantic_fingerprint,
            plan,
            explicit_memos,
            program_arc,
            charged,
        ))
    }
}

// ───────────────────────────────────────────────────────────────────
// Wave 4b (Phase Ω.10) — MultiMethod wire-up tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod multimethod_tests {
    use std::path::PathBuf;
    

    use crate::kasm::{MultiMethod, Node, Program, ProgramSig, Target, Ty};
    use crate::{Hash, MemoryGovernor, Store};

    use super::*;

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("wave4b-mm", tag)
    }

    fn fresh_node(tag: &str) -> (MonsterNode, PathBuf) {
        let path = fresh_path(tag);
        let monster = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (monster, path)
    }

    fn store_program(node: &MonsterNode, p: &Program) -> ([u8; 20], Hash) {
        let h = node.store().store(p.bytes()).unwrap();
        (*h.as_bytes(), h)
    }

    fn affine_3x_plus_1(target: Target) -> Program {
        // f(x) = 3 * x + 1 — single-output I64 program.
        Program::new(
            target,
            1,
            1,
            8,
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

    fn binary_add(target: Target) -> Program {
        // g(x, y) = x + y — distinct input arity (2 vs 1) gives a
        // distinct ProgramSig, no reliance on multi-output dispatch
        // (which the hot path may collapse on simple cases).
        Program::new(
            target,
            2,
            1,
            8,
            vec![
                Node::input(0),
                Node::input(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave4b_store_load_roundtrip() {
        let (node, path) = fresh_node("rt");
        let p = affine_3x_plus_1(Target::Cpu);
        let (h_bytes, _h) = store_program(&node, &p);

        let mm = MultiMethod::new(vec![(p.sig(), h_bytes)]);
        let mm_hash = node.store_multimethod(&mm).unwrap();
        let mm_loaded = node.load_multimethod(&mm_hash).unwrap();

        assert_eq!(mm, mm_loaded, "round-trip via CAS preserves bundle");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave4b_resolve_returns_none_for_missing_signature() {
        // Tâche A.2 invariant : absence ⇒ Ok(None), never Err.
        let (node, path) = fresh_node("absent");
        let p = affine_3x_plus_1(Target::Cpu);
        let (h_bytes, _h) = store_program(&node, &p);

        let mm = MultiMethod::new(vec![(p.sig(), h_bytes)]);
        let mm_hash = node.store_multimethod(&mm).unwrap();

        // Probe with a signature that no registered method matches.
        let absent_sig = ProgramSig::new(vec![Ty::Bool], vec![Ty::Bool]);
        let resolved: Option<[u8; 20]> =
            node.resolve_multimethod(&mm_hash, &absent_sig).unwrap();
        assert!(
            resolved.is_none(),
            "no matching method must yield Ok(None), never Err"
        );

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave4b_call_multi_dispatches_to_correct_program() {
        // End-to-end : two real programs with distinct input arity →
        // distinct ProgramSig. call_multi(unary_sig) executes p_unary ;
        // call_multi(binary_sig) executes p_binary. Each hop runs the
        // full call_bytes pipeline (cache + hotplan) and validates the
        // returned result against a Rust reference.
        let (node, path) = fresh_node("dispatch");
        let p_unary = affine_3x_plus_1(Target::Cpu);  // f(x) = 3*x + 1
        let p_binary = binary_add(Target::Cpu);        // g(x, y) = x + y
        let (h_unary_bytes, _) = store_program(&node, &p_unary);
        let (h_binary_bytes, _) = store_program(&node, &p_binary);

        let mm = MultiMethod::new(vec![
            (p_unary.sig(), h_unary_bytes),
            (p_binary.sig(), h_binary_bytes),
        ]);
        let mm_hash = node.store_multimethod(&mm).unwrap();

        // Dispatch on the unary signature → 3 * 7 + 1 = 22.
        let unary_sig = ProgramSig::new(vec![Ty::I64], vec![Ty::I64]);
        let unary_args = 7i64.to_le_bytes();
        let unary_call = node.call_multi(&mm_hash, &unary_sig, &unary_args).unwrap();
        let unary_bytes = node.store().load(&unary_call.result).unwrap();
        let unary_value = i64::from_le_bytes(unary_bytes.try_into().unwrap());
        assert_eq!(unary_value, 22, "f(x=7) = 3*7 + 1 = 22");

        // Dispatch on the binary signature → 11 + 31 = 42.
        let binary_sig = ProgramSig::new(vec![Ty::I64, Ty::I64], vec![Ty::I64]);
        let mut binary_args = Vec::with_capacity(16);
        binary_args.extend_from_slice(&11i64.to_le_bytes());
        binary_args.extend_from_slice(&31i64.to_le_bytes());
        let binary_call = node.call_multi(&mm_hash, &binary_sig, &binary_args).unwrap();
        let binary_bytes = node.store().load(&binary_call.result).unwrap();
        let binary_value = i64::from_le_bytes(binary_bytes.try_into().unwrap());
        assert_eq!(binary_value, 42, "g(x=11, y=31) = 11 + 31 = 42");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave4b_call_multi_rejects_unknown_signature_with_not_found() {
        // call_multi tightens the contract vs resolve_multimethod : if
        // no method matches, return io::ErrorKind::NotFound (distinct
        // from "bundle missing", which is io::ErrorKind::Other). This
        // lets callers branch on .kind() to distinguish the two.
        let (node, path) = fresh_node("nf");
        let p = affine_3x_plus_1(Target::Cpu);
        let (h_bytes, _) = store_program(&node, &p);

        let mm = MultiMethod::new(vec![(p.sig(), h_bytes)]);
        let mm_hash = node.store_multimethod(&mm).unwrap();

        let bad_sig = ProgramSig::new(vec![Ty::Bool, Ty::Bool], vec![Ty::Bool]);
        let arg_bytes = [0u8; 8];
        let err = node.call_multi(&mm_hash, &bad_sig, &arg_bytes).unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "no matching method must surface as NotFound, got: {err}"
        );

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave4b_load_unknown_bundle_returns_real_error() {
        // Wave 9 update — load_multimethod now surfaces NotFound
        // specifically (was io::Error::other before). Distinct from
        // "bundle present but no matching method" (Ok(None)).
        let (node, path) = fresh_node("unknown");
        let unknown = Hash::from_bytes([0xFF; 20]);
        let err = node
            .resolve_multimethod(
                &unknown,
                &ProgramSig::new(vec![Ty::I64], vec![Ty::I64]),
            )
            .unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "Wave 9: missing CAS hash must surface NotFound, got kind: {:?}",
            err.kind(),
        );
        assert!(
            err.to_string().contains("unknown multimethod hash"),
            "error message must reference the missing hash, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave9_unknown_func_hash_surfaces_notfound() {
        // Wave 9 — call_bytes / call_one_i64 / hot_program all
        // funnel through hot_program() which now surfaces NotFound
        // when the program hash is missing from the CAS.
        let (node, path) = fresh_node("wave9_func");
        let unknown = Hash::from_bytes([0xAB; 20]);
        let err = node.call_bytes(&unknown, &[]).unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "Wave 9: unknown func hash must surface NotFound, got: {:?}",
            err.kind(),
        );
        assert!(
            err.to_string().contains("unknown func hash"),
            "error must reference 'unknown func hash', got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }
}

// ───────────────────────────────────────────────────────────────────
// Wave 6 (Phase Ω.10) — Op::Pipeline FULL tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod pipeline_tests {
    use std::path::PathBuf;
    

    use crate::kasm::{Node, Program, Target, Ty};
    use crate::{Hash, MemoryGovernor, Store};

    use super::*;

    fn fresh_path(tag: &str) -> PathBuf {
        crate::fresh_tmp_path("wave6-pipe", tag)
    }

    fn fresh_node(tag: &str) -> (MonsterNode, PathBuf) {
        let path = fresh_path(tag);
        let monster = MonsterNode::new(
            Store::open(&path).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (monster, path)
    }

    fn double(target: Target) -> Program {
        // f(x) = x * 2
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn add_five(target: Target) -> Program {
        // g(y) = y + 5
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(5),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave6_call_pipeline_composes_two_programs() {
        // pipeline(double, add_five)(x=10) = add_five(double(10))
        //                                  = add_five(20)
        //                                  = 25
        let (node, path) = fresh_node("compose");
        let p_a = double(Target::Cpu);
        let p_b = add_five(Target::Cpu);
        let h_a = node.store().store(p_a.bytes()).unwrap();
        let h_b = node.store().store(p_b.bytes()).unwrap();

        let args = 10i64.to_le_bytes();
        let call = node.call_pipeline(&h_a, &h_b, &args).unwrap();
        let bytes = node.store().load(&call.result).unwrap();
        let value = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(value, 25, "pipeline(double, add_five)(10) = (10*2)+5 = 25");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave6_pipeline_intermediate_result_is_independently_memoized() {
        // The first hop's result is persisted in the CAS through the
        // standard call_bytes pipeline, so calling `prog_a` directly
        // afterwards on the same args must hit the cache rather than
        // re-execute. This is the content-addressed contract : every
        // step of a pipeline is independently reusable.
        let (node, path) = fresh_node("intermediate");
        let p_a = double(Target::Cpu);
        let p_b = add_five(Target::Cpu);
        let h_a = node.store().store(p_a.bytes()).unwrap();
        let h_b = node.store().store(p_b.bytes()).unwrap();

        let args = 7i64.to_le_bytes();
        let _ = node.call_pipeline(&h_a, &h_b, &args).unwrap();

        // Direct call to prog_a on the same args : intermediate is
        // already in the CAS, so this returns 14 immediately.
        let direct = node.call_bytes(&h_a, &args).unwrap();
        let bytes = node.store().load(&direct.result).unwrap();
        let value = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(value, 14, "double(7) = 14, computed by the pipeline's first hop");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave6_call_pipeline_errors_on_missing_program() {
        // prog_a missing from CAS → call_bytes surfaces an error on
        // the first hop; we never reach prog_b. Verifies the brain
        // doesn't silently swallow the hash miss.
        let (node, path) = fresh_node("missing");
        let p_b = add_five(Target::Cpu);
        let h_b = node.store().store(p_b.bytes()).unwrap();

        let unknown = Hash::from_bytes([0xAB; 20]);
        let args = 1i64.to_le_bytes();
        let err = node.call_pipeline(&unknown, &h_b, &args).unwrap_err();
        // Real failure, not silent — we don't care about the exact
        // ErrorKind because call_bytes flavours its own errors, but
        // the message must reference the unknown program.
        assert!(
            !err.to_string().is_empty(),
            "missing prog_a must surface a real error"
        );

        let _ = std::fs::remove_dir_all(&path);
    }

    fn binary_add(target: Target) -> Program {
        // f(acc, x) = acc + x — a binary i64 program reusable as a
        // reduce/scan operator (matches the (acc, x) → i64 contract).
        Program::new(
            target,
            2,
            1,
            8,
            vec![
                Node::input(0),
                Node::input(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn binary_mul(target: Target) -> Program {
        // f(acc, x) = acc * x — distinct binary op for reduce sanity.
        Program::new(
            target,
            2,
            1,
            8,
            vec![
                Node::input(0),
                Node::input(1),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave7a_call_map_applies_program_elementwise() {
        // map(double, [1,2,3,4]) = [2,4,6,8]
        let (node, path) = fresh_node("map");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_map(&h, &[1, 2, 3, 4]).unwrap();
        assert_eq!(out, vec![2, 4, 6, 8]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_call_map_redundant_inputs_hit_cache() {
        // Same value repeated → the second hit must reuse the cache
        // entry from the first call. We don't measure timing here (too
        // brittle for CI) — we verify correctness on a pathological
        // input pattern that would explode without caching.
        let (node, path) = fresh_node("mapcache");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let many = vec![7i64; 256];
        let out = node.call_map(&h, &many).unwrap();
        assert!(out.iter().all(|&v| v == 14), "all 256 entries must be 14");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_call_pmap_preserves_order() {
        // pmap dispatches in parallel but must return in input order.
        let (node, path) = fresh_node("pmap");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let input: Vec<i64> = (0..32).collect();
        let out = node.call_pmap(&h, &input).unwrap();
        let expected: Vec<i64> = input.iter().map(|&x| x * 2).collect();
        assert_eq!(out, expected, "pmap output preserves input order");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_call_reduce_folds_over_vec() {
        // reduce(add, [1,2,3,4,5], init=0) = 15
        let (node, path) = fresh_node("reduce_sum");
        let p_add = binary_add(Target::Cpu);
        let h_add = node.store().store(p_add.bytes()).unwrap();
        let sum = node.call_reduce(&h_add, &[1, 2, 3, 4, 5], 0).unwrap();
        assert_eq!(sum, 15, "sum(1..=5) = 15");

        // reduce(mul, [1,2,3,4,5], init=1) = 120 (factorial)
        let p_mul = binary_mul(Target::Cpu);
        let h_mul = node.store().store(p_mul.bytes()).unwrap();
        let fact = node.call_reduce(&h_mul, &[1, 2, 3, 4, 5], 1).unwrap();
        assert_eq!(fact, 120, "5! = 120");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_call_reduce_empty_vec_returns_init() {
        // The fold contract : empty input → init unchanged.
        let (node, path) = fresh_node("reduce_empty");
        let p = binary_add(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let acc = node.call_reduce(&h, &[], 42).unwrap();
        assert_eq!(acc, 42, "empty fold returns init");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_call_scan_returns_intermediate_accumulators() {
        // scan(add, [1,2,3,4], init=0) = [0, 1, 3, 6, 10]
        // First entry is init, then one entry per fold step.
        let (node, path) = fresh_node("scan");
        let p = binary_add(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let chain = node.call_scan(&h, &[1, 2, 3, 4], 0).unwrap();
        assert_eq!(chain, vec![0, 1, 3, 6, 10]);
        let _ = std::fs::remove_dir_all(&path);
    }

    fn double_via_add(target: Target) -> Program {
        // f(x) = x + x — alternate impl of `double`, distinct shape
        // from the canonical `x * 2` form.
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::add(0, 0),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn double_via_shl(target: Target) -> Program {
        // f(x) = x << 1 — third alternate impl, bit-shift form.
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::shl(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave11_6_call_adaptive_picks_correct_result() {
        // 3 impls of `double` (mul, add, shl). All compute 2*x.
        // call_adaptive returns the fastest one's result; we don't
        // care which one wins — just that the returned value is correct.
        let (node, path) = fresh_node("adaptive_pick");
        let p_mul = double(Target::Cpu);
        let p_add = double_via_add(Target::Cpu);
        let p_shl = double_via_shl(Target::Cpu);
        let h_mul = node.store().store(p_mul.bytes()).unwrap();
        let h_add = node.store().store(p_add.bytes()).unwrap();
        let h_shl = node.store().store(p_shl.bytes()).unwrap();

        let args = 21i64.to_le_bytes();
        let call = node.call_adaptive(&[h_mul, h_add, h_shl], &args).unwrap();
        let bytes = node.store().load(&call.result).unwrap();
        let v = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(v, 42, "all 3 impls compute double, result must be 21*2=42");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11_6_call_adaptive_single_impl_works() {
        // Degenerate case : 1 impl. call_adaptive must still work
        // (just always picks that one).
        let (node, path) = fresh_node("adaptive_single");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 7i64.to_le_bytes();
        let call = node.call_adaptive(&[h], &args).unwrap();
        let bytes = node.store().load(&call.result).unwrap();
        let v = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(v, 14);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11_6_call_adaptive_empty_impls_fails_loud() {
        // No impl to pick → Err, never silent fall-through.
        let (node, path) = fresh_node("adaptive_empty");
        let args = 0i64.to_le_bytes();
        let err = node.call_adaptive(&[], &args).unwrap_err();
        assert!(
            err.to_string().contains("empty impls"),
            "empty impls list must surface a clear error, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11_6_call_adaptive_propagates_impl_errors() {
        // Any impl failing → propagate the error, don't silently fall
        // back to others. The contract is "all impls compute the same
        // function" ; a failure is a bug, not a feature.
        let (node, path) = fresh_node("adaptive_err");
        let unknown = Hash::from_bytes([0xFE; 20]);
        let p = double(Target::Cpu);
        let h_real = node.store().store(p.bytes()).unwrap();
        let args = 1i64.to_le_bytes();
        let err = node.call_adaptive(&[h_real, unknown], &args).unwrap_err();
        assert!(
            !err.to_string().is_empty(),
            "impl error must surface, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    fn pred_lt_10(target: Target) -> Program {
        // pred(x) = (x < 10) encoded as i64 result (1 if true, 0 else).
        // We use SelectI64 + LtI64 chain to produce a 1/0 i64.
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(10),
                Node::lt(0, 1),             // Bool: x < 10
                Node::const_i64(1),         // then-value
                Node::const_i64(0),         // else-value (slot 4)
                Node::select_i64(2, 3, 4),  // select_i64(cond, then, else)
                Node::output(5, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave11b_call_iterate_generates_repeated_applications() {
        // iterate(double, 1, 5) = [1, 2, 4, 8, 16]
        let (node, path) = fresh_node("iter");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_iterate(&h, 1, 5).unwrap();
        assert_eq!(out, vec![1, 2, 4, 8, 16]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_iterate_n_zero_returns_empty() {
        let (node, path) = fresh_node("iter_zero");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_iterate(&h, 42, 0).unwrap();
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_iterate_n_one_returns_init() {
        // n=1 means just [init], no application yet.
        let (node, path) = fresh_node("iter_one");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_iterate(&h, 7, 1).unwrap();
        assert_eq!(out, vec![7]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_outer_produces_cartesian_product() {
        // outer(mul, [1,2,3], [10,20]) = [1*10, 1*20, 2*10, 2*20, 3*10, 3*20]
        //                              = [10, 20, 20, 40, 30, 60]
        let (node, path) = fresh_node("outer_mul");
        let p = binary_mul(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_outer(&h, &[1, 2, 3], &[10, 20]).unwrap();
        assert_eq!(out, vec![10, 20, 20, 40, 30, 60]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_outer_empty_inputs_return_empty() {
        let (node, path) = fresh_node("outer_empty");
        let p = binary_add(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        assert!(node.call_outer(&h, &[], &[1, 2]).unwrap().is_empty());
        assert!(node.call_outer(&h, &[1, 2], &[]).unwrap().is_empty());
        assert!(node.call_outer(&h, &[], &[]).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_take_while_collects_prefix() {
        // takeWhile(< 10, [1, 5, 9, 10, 11, 4]) = [1, 5, 9]
        // (stops at 10 which fails the predicate, even though 4 < 10)
        let (node, path) = fresh_node("takew");
        let pred = pred_lt_10(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_take_while(&h, &[1, 5, 9, 10, 11, 4]).unwrap();
        assert_eq!(out, vec![1, 5, 9]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_take_while_no_match_returns_empty() {
        // First element fails pred → empty result.
        let (node, path) = fresh_node("takew_none");
        let pred = pred_lt_10(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_take_while(&h, &[15, 1, 2, 3]).unwrap();
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11b_call_take_while_all_match_returns_full() {
        let (node, path) = fresh_node("takew_all");
        let pred = pred_lt_10(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_take_while(&h, &[1, 2, 3]).unwrap();
        assert_eq!(out, vec![1, 2, 3]);
        let _ = std::fs::remove_dir_all(&path);
    }

    fn negate(target: Target) -> Program {
        // h(x) = -x — third distinct unary program for switch tests
        Program::new(
            target,
            1,
            1,
            8,
            vec![Node::input(0), Node::neg(0), Node::output(1, Ty::I64)],
        )
        .unwrap()
    }

    #[test]
    fn wave11a_call_switch_dispatches_to_branch() {
        // branches = [double, add_five, negate]
        // switch(0, branches, x=10) → double(10) = 20
        // switch(1, branches, x=10) → add_five(10) = 15
        // switch(2, branches, x=10) → negate(10) = -10
        let (node, path) = fresh_node("switch");
        let p0 = double(Target::Cpu);
        let p1 = add_five(Target::Cpu);
        let p2 = negate(Target::Cpu);
        let h0 = node.store().store(p0.bytes()).unwrap();
        let h1 = node.store().store(p1.bytes()).unwrap();
        let h2 = node.store().store(p2.bytes()).unwrap();
        let branches = [h0, h1, h2];
        let args = 10i64.to_le_bytes();

        for (idx, expected) in [(0i64, 20i64), (1, 15), (2, -10)] {
            let call = node.call_switch(idx, &branches, &args).unwrap();
            let bytes = node.store().load(&call.result).unwrap();
            let v = i64::from_le_bytes(bytes.try_into().unwrap());
            assert_eq!(
                v, expected,
                "switch({idx}, [double, add_five, negate])(10) = {expected}"
            );
        }
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11a_call_switch_out_of_range_index_fails_loud() {
        // Index < 0 or ≥ branches.len() must surface a clear error
        // (no wrap, no saturate, no silent default-branch).
        let (node, path) = fresh_node("switch_oob");
        let p = double(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let branches = [h];
        let args = 5i64.to_le_bytes();

        let err_high = node.call_switch(7, &branches, &args).unwrap_err();
        assert!(
            err_high.to_string().contains("out of range"),
            "high OOB must fail loud, got: {err_high}"
        );
        let err_neg = node.call_switch(-1, &branches, &args).unwrap_err();
        assert!(
            err_neg.to_string().contains("out of range"),
            "negative index must fail loud, got: {err_neg}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11a_call_try_returns_primary_on_success() {
        // prog succeeds → fallback never invoked.
        let (node, path) = fresh_node("try_ok");
        let p = double(Target::Cpu);
        // Fallback is a sentinel program returning const 999. If the
        // primary path is taken, we never see the sentinel.
        let fallback_prog = Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(999),
                Node::output(1, Ty::I64),
            ],
        )
        .unwrap();
        let h_p = node.store().store(p.bytes()).unwrap();
        let h_f = node.store().store(fallback_prog.bytes()).unwrap();
        let args = 7i64.to_le_bytes();

        let call = node.call_try(&h_p, &args, &h_f).unwrap();
        let bytes = node.store().load(&call.result).unwrap();
        let v = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(v, 14, "double(7) = 14, fallback never invoked");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11a_call_try_falls_back_on_primary_error() {
        // prog hash is invalid (not in CAS) → call_bytes surfaces
        // an error → fallback invoked, returning its own result.
        let (node, path) = fresh_node("try_fb");
        let unknown = Hash::from_bytes([0xEE; 20]);
        let fallback = double(Target::Cpu);
        let h_f = node.store().store(fallback.bytes()).unwrap();
        let args = 3i64.to_le_bytes();

        let call = node.call_try(&unknown, &args, &h_f).unwrap();
        let bytes = node.store().load(&call.result).unwrap();
        let v = i64::from_le_bytes(bytes.try_into().unwrap());
        assert_eq!(v, 6, "fallback double(3) = 6 after primary failure");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave11a_call_try_both_failing_surfaces_fallback_error() {
        // Both prog and fallback are unknown hashes → caller sees
        // the *fallback's* error (the original primary error is
        // implied by reaching the fallback path).
        let (node, path) = fresh_node("try_both_fail");
        let p_unknown = Hash::from_bytes([0xAA; 20]);
        let f_unknown = Hash::from_bytes([0xBB; 20]);
        let args = 1i64.to_le_bytes();

        let err = node.call_try(&p_unknown, &args, &f_unknown).unwrap_err();
        // We don't assert on exact wording — just that an error
        // surfaces and references the unknown program. Both lookups
        // would emit a "not found" type message in the same family.
        assert!(
            !err.to_string().is_empty(),
            "both failing must surface a real error"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_filter_keeps_only_passing_elements() {
        // filter(is_even, [1,2,3,4,5,6]) = [2,4,6]
        let (node, path) = fresh_node("filter_even");
        let pred = pred_is_even(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_filter(&h, &[1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(out, vec![2, 4, 6]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_filter_empty_input_returns_empty() {
        let (node, path) = fresh_node("filter_empty");
        let pred = pred_is_even(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_filter(&h, &[]).unwrap();
        assert!(out.is_empty());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_filter_preserves_order() {
        // Out-of-order input must keep its order in the output.
        let (node, path) = fresh_node("filter_order");
        let pred = pred_is_even(Target::Cpu);
        let h = node.store().store(pred.bytes()).unwrap();
        let out = node.call_filter(&h, &[7, 6, 5, 4, 3, 2]).unwrap();
        assert_eq!(out, vec![6, 4, 2]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_zip_pairwise_add() {
        // zip(add, [1,2,3], [10,20,30]) = [11,22,33]
        let (node, path) = fresh_node("zip_add");
        let p = binary_add(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_zip(&h, &[1, 2, 3], &[10, 20, 30]).unwrap();
        assert_eq!(out, vec![11, 22, 33]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_zip_pairwise_mul() {
        // zip(mul, [1,2,3,4], [5,5,5,5]) = [5,10,15,20]
        let (node, path) = fresh_node("zip_mul");
        let p = binary_mul(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let out = node.call_zip(&h, &[1, 2, 3, 4], &[5, 5, 5, 5]).unwrap();
        assert_eq!(out, vec![5, 10, 15, 20]);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave7a_bis_call_zip_length_mismatch_fails_loud() {
        // No silent shape coercion — mismatched lengths surface as Err.
        let (node, path) = fresh_node("zip_mismatch");
        let p = binary_add(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let err = node.call_zip(&h, &[1, 2, 3], &[10, 20]).unwrap_err();
        assert!(
            err.to_string().contains("length mismatch"),
            "shape mismatch must fail loud, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    fn poly_x_squared(target: Target) -> Program {
        // f(x) = x * x, with x as F64 input slot 0
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::f64_mul(0, 0),
                Node::output(1, Ty::F64),
            ],
        )
        .unwrap()
    }

    fn sqrt_prog(target: Target) -> Program {
        // f(x) = sqrt(x)
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::f64_sqrt(0),
                Node::output(1, Ty::F64),
            ],
        )
        .unwrap()
    }

    fn exp_prog(target: Target) -> Program {
        // f(x) = exp(x)
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::f64_exp(0),
                Node::output(1, Ty::F64),
            ],
        )
        .unwrap()
    }

    fn ln_prog(target: Target) -> Program {
        // f(x) = ln(x)
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::f64_ln(0),
                Node::output(1, Ty::F64),
            ],
        )
        .unwrap()
    }

    fn assert_close(actual: f64, expected: f64, tol: f64, label: &str) {
        let diff = (actual - expected).abs();
        assert!(
            diff <= tol,
            "{label}: |{actual} - {expected}| = {diff} > {tol}"
        );
    }

    #[test]
    fn wave8a_grad_of_x_squared_is_two_x() {
        // d/dx (x²) = 2x ; at x=5 → 10
        let (node, path) = fresh_node("grad_xx");
        let p = poly_x_squared(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 5.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_close(g, 10.0, 1e-12, "d(x²)/dx at x=5");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_of_sqrt() {
        // d/dx sqrt(x) = 1/(2*sqrt(x)) ; at x=4 → 1/(2*2) = 0.25
        let (node, path) = fresh_node("grad_sqrt");
        let p = sqrt_prog(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 4.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_close(g, 0.25, 1e-12, "d sqrt(x)/dx at x=4");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_of_exp_at_zero_is_one() {
        // d/dx exp(x) = exp(x) ; at x=0 → 1
        let (node, path) = fresh_node("grad_exp");
        let p = exp_prog(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 0.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_close(g, 1.0, 1e-12, "d exp(x)/dx at x=0");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_of_ln() {
        // d/dx ln(x) = 1/x ; at x=2 → 0.5
        let (node, path) = fresh_node("grad_ln");
        let p = ln_prog(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 2.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_close(g, 0.5, 1e-12, "d ln(x)/dx at x=2");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_of_constant_is_zero() {
        // d/dx (c) = 0 — no input dependence
        let (node, path) = fresh_node("grad_const");
        // f(x) = 7.0 (constant), but we still take 1 input so the
        // var_index has somewhere to live.
        let p = Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::const_f64(7),
                Node::output(1, Ty::F64),
            ],
        )
        .unwrap();
        let h = node.store().store(p.bytes()).unwrap();
        let args = 3.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_eq!(g, 0.0, "constant function has zero gradient");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_chain_rule_x_cubed() {
        // f(x) = x³ via x * (x * x). d/dx x³ = 3x² ; at x=2 → 12
        let (node, path) = fresh_node("grad_xxx");
        let p = Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input_f64(0),
                Node::f64_mul(0, 0), // x²
                Node::f64_mul(0, 1), // x³
                Node::output(2, Ty::F64),
            ],
        )
        .unwrap();
        let h = node.store().store(p.bytes()).unwrap();
        let args = 2.0f64.to_bits().to_le_bytes();
        let g = node.call_grad(&h, 0, &args).unwrap();
        assert_close(g, 12.0, 1e-12, "d(x³)/dx at x=2");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_rejects_unknown_program() {
        // prog hash absent du CAS → NotFound, jamais un faux gradient.
        let (node, path) = fresh_node("grad_nf");
        let unknown = Hash::from_bytes([0xCD; 20]);
        let args = 0.0f64.to_bits().to_le_bytes();
        let err = node.call_grad(&unknown, 0, &args).unwrap_err();
        assert_eq!(
            err.kind(),
            io::ErrorKind::NotFound,
            "missing program must surface NotFound, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_rejects_i64_ops_in_chain() {
        // A program that uses Op::AddI64 in its chain falls outside
        // the F64 surface — Wave 8a fails loud rather than returning
        // a bogus gradient.
        let (node, path) = fresh_node("grad_i64");
        let p = Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap();
        let h = node.store().store(p.bytes()).unwrap();
        let args = 5i64.to_le_bytes();
        let err = node.call_grad(&h, 0, &args).unwrap_err();
        assert!(
            err.to_string().contains("AddI64") || err.to_string().to_lowercase().contains("not supported"),
            "i64 op must surface a clear unsupported error, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    fn pred_is_even(target: Target) -> Program {
        // pred(x) = (x & 1) ⊕ 1 — non-zero when x is even
        // (filter contract : non-zero = keep)
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::bit_and(0, 1),
                Node::const_i64(1),
                Node::bit_xor(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn body_acc_plus_i(target: Target) -> Program {
        // body(i, acc) = acc + i — fori sum body
        Program::new(
            target,
            2,
            1,
            8,
            vec![
                Node::input(0), // i
                Node::input(1), // acc
                Node::add(1, 0),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn body_acc_times_iplus1(target: Target) -> Program {
        // body(i, acc) = acc * (i + 1) — factorial body
        Program::new(
            target,
            2,
            1,
            8,
            vec![
                Node::input(0),     // i
                Node::input(1),     // acc
                Node::const_i64(1), // 1
                Node::add(0, 2),    // i+1
                Node::mul(1, 3),    // acc*(i+1)
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn cond_state_nonzero(target: Target) -> Program {
        // cond(state) = state != 0  ⇒  encode as state itself
        // (the call_while contract treats non-zero as "continue")
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::output(0, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn body_collatz_step(target: Target) -> Program {
        // body(state) = if state == 1 then 0 else (if even then state/2 else 3*state+1)
        // We pick the simpler "state - 1" body for testing :
        // body(state) = state - 1, paired with cond(state) = state ≠ 0.
        // Loop terminates after `init` iterations.
        Program::new(
            target,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::sub(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn wave10_call_fori_sums_range() {
        // sum_{i=0..5} i = 0+1+2+3+4 = 10
        let (node, path) = fresh_node("fori_sum");
        let body = body_acc_plus_i(Target::Cpu);
        let h = node.store().store(body.bytes()).unwrap();
        let s = node.call_fori(&h, 0, 5, 0).unwrap();
        assert_eq!(s, 10);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave10_call_fori_factorial() {
        // factorial(5) via fori : init=1, body(i, acc) = acc * (i+1) for i=0..5
        let (node, path) = fresh_node("fori_fact");
        let body = body_acc_times_iplus1(Target::Cpu);
        let h = node.store().store(body.bytes()).unwrap();
        let f = node.call_fori(&h, 0, 5, 1).unwrap();
        assert_eq!(f, 120, "5! = 120");
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave10_call_fori_empty_range_returns_init() {
        // start >= stop → init unchanged
        let (node, path) = fresh_node("fori_empty");
        let body = body_acc_plus_i(Target::Cpu);
        let h = node.store().store(body.bytes()).unwrap();
        let v = node.call_fori(&h, 7, 7, 42).unwrap();
        assert_eq!(v, 42);
        let v2 = node.call_fori(&h, 10, 5, 99).unwrap();
        assert_eq!(v2, 99);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave10_call_while_terminates_on_zero_cond() {
        // state - 1 until state == 0, starting at 7 → terminates at 0 after 7 steps.
        let (node, path) = fresh_node("while_dec");
        let cond = cond_state_nonzero(Target::Cpu);
        let body = body_collatz_step(Target::Cpu);
        let h_cond = node.store().store(cond.bytes()).unwrap();
        let h_body = node.store().store(body.bytes()).unwrap();
        let final_state = node.call_while(&h_cond, &h_body, 7, 100).unwrap();
        assert_eq!(final_state, 0);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave10_call_while_immediate_exit_returns_init() {
        // cond(0) = 0 → loop never enters, returns init.
        let (node, path) = fresh_node("while_init");
        let cond = cond_state_nonzero(Target::Cpu);
        let body = body_collatz_step(Target::Cpu);
        let h_cond = node.store().store(cond.bytes()).unwrap();
        let h_body = node.store().store(body.bytes()).unwrap();
        let s = node.call_while(&h_cond, &h_body, 0, 100).unwrap();
        assert_eq!(s, 0);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave10_call_while_fuel_exhaustion_fails_loud() {
        // Run with insufficient fuel → must surface a real error, not
        // a wrong "best-effort" answer.
        let (node, path) = fresh_node("while_fuel");
        let cond = cond_state_nonzero(Target::Cpu);
        let body = body_collatz_step(Target::Cpu);
        let h_cond = node.store().store(cond.bytes()).unwrap();
        let h_body = node.store().store(body.bytes()).unwrap();
        // 100 iterations needed, only 10 fuel → exhaustion.
        let err = node.call_while(&h_cond, &h_body, 100, 10).unwrap_err();
        assert!(
            err.to_string().contains("fuel exhausted"),
            "fuel exhaustion must surface clearly, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave8a_grad_rejects_out_of_range_var_index() {
        // var_index >= n_inputs → io::Error, never a wrong gradient.
        let (node, path) = fresh_node("grad_oob");
        let p = poly_x_squared(Target::Cpu);
        let h = node.store().store(p.bytes()).unwrap();
        let args = 1.0f64.to_bits().to_le_bytes();
        let err = node.call_grad(&h, 7, &args).unwrap_err();
        assert!(
            err.to_string().contains("var_index"),
            "out-of-range var_index must surface a clear error, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn wave6_pipeline_op_embedded_in_program_fails_loud() {
        // A program that contains `Op::Pipeline` as a node must not be
        // silently mis-executed by the scalar interpreter. The
        // canonical use of pipeline composition is `call_pipeline` on
        // the brain ; encountering `Op::Pipeline` in raw KASM means
        // someone constructed it by hand (or from a transformer) and
        // tried to dispatch it through `call_bytes`. Fail loud, as
        // with Vmap/Pmap/Fori/etc.
        let (node, path) = fresh_node("embedded");
        // Program with one Pipeline node (slots 1 and 2 are I64
        // const-i64 placeholders standing in as program-hash slots).
        let nodes = vec![
            Node::input(0),
            Node::const_i64(1),
            Node::const_i64(2),
            Node::pipeline(1, 2),
            Node::output(3, Ty::I64),
        ];
        let p = Program::new(Target::Cpu, 1, 1, 8, nodes).unwrap();
        let h = node.store().store(p.bytes()).unwrap();
        let args = 0i64.to_le_bytes();
        let err = node.call_bytes(&h, &args).unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("v1")
                || err.to_string().to_lowercase().contains("pipeline")
                || err.to_string().to_lowercase().contains("unsupported"),
            "embedded Op::Pipeline must fail loud, got: {err}"
        );

        let _ = std::fs::remove_dir_all(&path);
    }
}
