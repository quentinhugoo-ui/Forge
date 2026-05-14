//! KASM core types: opcodes, value kinds, errors, reports.
//!
//! Pure data — no algorithms live here. Verification, execution and
//! optimisation all import these definitions.

use std::fmt;

pub const HEADER_LEN: usize = 32;
pub const NODE_LEN: usize = 8;
pub const FOOTER_LEN: usize = 32;
pub const MAX_NODES: usize = 4096;
pub const MAX_SLOTS: u8 = 16;

pub(super) const MAGIC: &[u8; 4] = b"KASM";
pub(super) const VERSION: u8 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Auto = 0,
    Cpu = 1,
    Kernel = 2,
    Gpu = 3,
    Qpu = 4,
}

impl Target {
    pub(super) fn from_byte(b: u8) -> Result<Self, KasmError> {
        match b {
            0 => Ok(Target::Auto),
            1 => Ok(Target::Cpu),
            2 => Ok(Target::Kernel),
            3 => Ok(Target::Gpu),
            4 => Ok(Target::Qpu),
            _ => Err(KasmError::BadTarget(b)),
        }
    }

    pub fn needs_external_backend(self) -> bool {
        self == Target::Qpu
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ty {
    I64 = 1,
    Bool = 2,
    /// Φ.0 — IEEE 754 double precision. **Storage-polymorphic** : at
    /// runtime an `F64` value lives in the same `Value::I64(i64)` slot
    /// as integer values; only the operations interpret the bit pattern
    /// via `f64::from_bits` / `f64::to_bits`. The wire format is byte-
    /// identical to `I64` (8 bytes little-endian bit pattern), so a
    /// program that takes I64 inputs and a program that takes F64 inputs
    /// share the same `args: &[u8]` calling convention.
    F64 = 3,
    /// KASM v1.0 scaffolding: bytecode may now name a vector-of-i64
    /// type for future `Vmap`/`Reduce`/`Scan` support, but every
    /// runtime path must still fail loud until storage semantics exist.
    VecI64 = 4,
}

impl Ty {
    pub(super) fn from_byte(b: u8) -> Result<Self, KasmError> {
        match b {
            1 => Ok(Ty::I64),
            2 => Ok(Ty::Bool),
            3 => Ok(Ty::F64),
            4 => Ok(Ty::VecI64),
            _ => Err(KasmError::BadType(b)),
        }
    }

    /// Wave 4 (Φ.11.3) — Wire byte for `ProgramSig` / `MultiMethod`
    /// encoding. Mirrors the discriminant since the enum is `repr`-less
    /// but the values are stable.
    pub fn to_byte(self) -> u8 {
        match self {
            Ty::I64 => 1,
            Ty::Bool => 2,
            Ty::F64 => 3,
            Ty::VecI64 => 4,
        }
    }
}

/// Wave 4 (Phase Ω.10, 2026-05-01) — first real Julia feature stolen :
/// **multiple dispatch**. A `ProgramSig` is the type signature of a
/// Forge program — its input arity and per-input type, plus its output
/// arity and per-output type. Two programs sharing the same `ProgramSig`
/// are interchangeable from the dispatcher's point of view.
///
/// Used as the key in `MultiMethod` bundles so the runtime can pick the
/// correct specialization based on the runtime types of arguments —
/// exactly Julia's `f(x::Int)` vs `f(x::Float64)` mechanism, but
/// content-addressed (the bundle itself is hashed, methods are added by
/// forking, never mutated).
///
/// Equality is structural (Vec equality on inputs and outputs). Ordering
/// is lex on `(inputs, outputs)` so canonical encoding sorts methods
/// deterministically.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProgramSig {
    pub inputs: Vec<Ty>,
    pub outputs: Vec<Ty>,
}

impl ProgramSig {
    pub fn new(inputs: Vec<Ty>, outputs: Vec<Ty>) -> Self {
        Self { inputs, outputs }
    }

    /// Wire-encode the signature for inclusion in a `MultiMethod` blob.
    /// Layout: `[inputs.len() as u8, inputs[0].to_byte(), ...,
    ///          outputs.len() as u8, outputs[0].to_byte(), ...]`.
    pub(super) fn encode_into(&self, out: &mut Vec<u8>) {
        debug_assert!(self.inputs.len() <= u8::MAX as usize);
        debug_assert!(self.outputs.len() <= u8::MAX as usize);
        out.push(self.inputs.len() as u8);
        for ty in &self.inputs {
            out.push(ty.to_byte());
        }
        out.push(self.outputs.len() as u8);
        for ty in &self.outputs {
            out.push(ty.to_byte());
        }
    }

    /// Wire-decode. Returns `(sig, bytes_consumed)`.
    pub(super) fn decode(bytes: &[u8]) -> Result<(Self, usize), KasmError> {
        let mut cursor = 0;
        let read_arity = |cursor: &mut usize| -> Result<usize, KasmError> {
            if *cursor >= bytes.len() {
                return Err(KasmError::BadMultiMethod("truncated signature".into()));
            }
            let n = bytes[*cursor] as usize;
            *cursor += 1;
            Ok(n)
        };
        let read_types = |cursor: &mut usize, n: usize| -> Result<Vec<Ty>, KasmError> {
            if *cursor + n > bytes.len() {
                return Err(KasmError::BadMultiMethod("truncated type list".into()));
            }
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                out.push(Ty::from_byte(bytes[*cursor])?);
                *cursor += 1;
            }
            Ok(out)
        };
        let n_in = read_arity(&mut cursor)?;
        let inputs = read_types(&mut cursor, n_in)?;
        let n_out = read_arity(&mut cursor)?;
        let outputs = read_types(&mut cursor, n_out)?;
        Ok((Self { inputs, outputs }, cursor))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    // --- v0.1 (frozen on disk) ---
    Input = 0,
    ConstI64 = 1,
    AddI64 = 2,
    MulI64 = 3,
    EqI64 = 4,
    Hash64 = 5,
    Output = 6,
    SubI64 = 7,
    DivI64Checked = 8,
    MinI64 = 9,
    MaxI64 = 10,
    SelectI64 = 11,
    AndBool = 12,
    OrBool = 13,
    NotBool = 14,

    // --- v0.2 expansion (still pure, still terminating) ---
    /// `a < b` (signed) → Bool.
    LtI64 = 15,
    /// `a <= b` (signed) → Bool.
    LeI64 = 16,
    /// `a & b` bitwise → I64.
    BitAndI64 = 17,
    /// `a | b` bitwise → I64.
    BitOrI64 = 18,
    /// `a ^ b` bitwise → I64.
    BitXorI64 = 19,
    /// `a << (b mod 64)` → I64. The mask makes it a total function.
    ShlI64 = 20,
    /// Logical right shift `(u64(a) >> (b mod 64))` → I64. Total.
    ShrI64 = 21,
    /// Saturating `a + b` → I64. No wrap-around, fully deterministic.
    SatAddI64 = 22,
    /// Saturating `a - b` → I64. No wrap-around, fully deterministic.
    SatSubI64 = 23,
    /// Checked Euclidean `a mod b` → I64 (`0` when `b == 0`). Total.
    ModI64Checked = 24,
    /// `clamp(a, b, imm_ref)` — `lo ≤ hi` is *not* required:
    /// implementation falls back to `min(max(a, lo), hi)` so it stays a
    /// total function regardless of input ordering. Three I64 refs.
    ClampI64 = 25,
    /// Bounded reduce by addition: sum the I64 nodes at indices
    /// `[a, a + imm)`. `imm > 0`, all I64, all referenced backwards.
    ReduceAddI64 = 26,
    /// Bounded reduce by multiplication, same shape as `ReduceAddI64`.
    ReduceMulI64 = 27,

    // --- v0.3 Ω-6.1 native reversible (bijective) unary ops ---
    /// Bitwise complement: `a → !a`. Bijective, total, Landauer-cost zero.
    BitFlipI64 = 28,
    /// Two's complement negation: `a → 0_i64.wrapping_sub(a)`.
    /// Bijective on `u64` (and thus on `i64` via wrapping). `i64::MIN`
    /// maps to itself, preserving total bijectivity.
    NegI64 = 29,
    /// Reverse the 64 bits of `a`. Bijective, involutive.
    ReverseBitsI64 = 30,
    /// Swap the 8 bytes of `a` (endian flip). Bijective, involutive.
    ByteswapI64 = 31,

    // --- Φ.0 IEEE 754 layer (storage-polymorphic over Value::I64 bits) ---
    /// Φ.0 — F64 small-integer constant. `imm` is an `i16` cast to
    /// `f64` (range `-32_768.0..=32_767.0`). For floats outside this
    /// range or with non-integer parts, build via `ConstI64` +
    /// `F64Op::I64ToF64` + `F64Op::DivF64Checked`.
    ConstF64 = 32,
    /// Φ.0 — Multi-headed F64 op. The low byte of `imm` selects the
    /// sub-operation (see `f64_sub_op_*` constants below). This is the
    /// **only** new opcode required to extend KASM with a full IEEE 754
    /// surface — Via Negativa applied to ISA expansion: one opcode, one
    /// arm in every external match, dispatched internally by `imm`.
    F64Op = 33,

    // ─── KASM v1.0 mutation — features piquées à JAX/Julia/Mojo/APL ──
    //
    // Addition 2026-05-01. Le bytecode reste compatible avec les
    // programmes v0.x (ops 0-33 inchangés) ; les nouveaux opcodes 34+
    // étendent KASM vers un dialecte de calcul scientifique de niveau
    // industriel. Chaque opcode est documenté avec sa source d'inspiration
    // et son effet sémantique.

    /// **Φ.11.6 — Auto-tuning adaptatif** (origine : Mojo `@adaptive`)
    ///
    /// `Op::Adaptive(a, _, imm)` — wrappe le résultat de `values[a]` avec
    /// une décision auto-tunée pour le hardware courant. Le paramètre
    /// `imm` indexe une famille de configurations à tester (block_size,
    /// tile_size, vec_lanes...). Au premier appel, l'interpréteur essaie
    /// les configurations, mesure, garde la meilleure. Les appels
    /// suivants utilisent la config cachée. Stocké dans atlas avec clé
    /// `(prog_hash, hardware_fingerprint)` — chaque machine du swarm
    /// converge vers son optimum sans config manuelle.
    /// Pseudo-code: `return autotune(family = imm, expr = values[a])`
    /// Limitation actuelle: wrapper accepté partout, mais la vraie
    /// recherche per-hardware n'est pas encore branchée.
    Adaptive = 34,

    /// **Φ.11.7 — Évaluation load-time** (origine : Mojo / Zig `comptime`)
    ///
    /// `Op::Comptime(a, _, _)` — marque `values[a]` pour évaluation au
    /// LOAD du Program (pas au runtime). Le résultat est inliné dans le
    /// bytecode comme constante. Le hash du programme change avec la
    /// valeur évaluée → content-addressing prend en compte le résultat
    /// partiel. Pas un macro système séparé — le **même langage** avant
    /// et après. Cas d'usage : pré-calculer sin/cos table, inliner π/e,
    /// spécialiser un programme générique selon des paramètres connus.
    /// Pseudo-code: `splice(Const(eval_at_load_time(values[a])))`
    /// Limitation actuelle: toléré au runtime en pass-through; le
    /// pliage load-time réel reste partiel côté loader/optimizer.
    Comptime = 35,

    /// **Φ.11.8 — Auto-différentiation symbolique** (origine : JAX `grad`)
    ///
    /// `Op::Grad(a, _, var_idx)` — produit la dérivée symbolique de
    /// `values[a]` par rapport à `Input(var_idx)`. Transformation chain
    /// rule appliquée nœud par nœud sur le DAG : `D[f∘g] = D[f]∘g × D[g]`.
    /// Le programme dérivé est lui-même content-addressed (nouveau hash),
    /// stocké dans atlas, exécutable comme tout programme. Débloque ML
    /// training (Phase 9 BitNet) — sans framework externe.
    /// Pseudo-code: `return derivative(values[a], with_respect_to = imm)`
    /// Limitation actuelle: méta-op documentée/typée, mais encore
    /// fail-loud dans l'interpréteur scalaire.
    Grad = 36,

    /// **JAX `lax.cond`** — branchement fonctionnel pur
    ///
    /// `Op::Cond(pred, then_slot, else_slot)` — retourne `values[then_slot]`
    /// si `values[pred] != 0`, sinon `values[else_slot]`. Différent de
    /// `SelectI64` : sémantique explicite, optimisation différente
    /// (les deux branches sont calculées dans SelectI64 ; Cond peut
    /// suivre des règles de short-circuit dans certains backends).
    /// Pseudo-code: `if truthy(values[a]) { values[b] } else { values[imm] }`
    /// Limitation actuelle: sémantique scalaire implémentée, mais pas
    /// encore de lowering/backends spécialisés.
    Cond = 37,

    /// **Memoization explicite par hash** (origine : Mathematica `f[x_]:=...`)
    ///
    /// `Op::Memoize(a, _, _)` — force la mise en cache du résultat de
    /// `values[a]` même si pas atteint via le brain naturel. Utilisé
    /// pour les sous-expressions coûteuses qui ne sont pas en hot path
    /// mais qu'on veut pré-payer. Distinct du cache content-addressed
    /// global — c'est une indication explicite de l'utilisateur.
    /// Pseudo-code: `cache.force_insert(subgraph(a), args, values[a]); return values[a]`
    /// Limitation actuelle: effet réel au brain (`RamMemo`) ; le
    /// scalar interpreter reste volontairement pass-through.
    Memoize = 38,

    /// **Composition de programmes** (origine : OCaml `|>`, Elixir, F#)
    ///
    /// `Op::Pipeline(prog_a, prog_b, _)` — applique le programme dont le
    /// hash est dans `values[prog_a]` à l'input courant, puis applique
    /// le programme dont le hash est dans `values[prog_b]` au résultat.
    /// Composition fonctionnelle native, équivalent à `g(f(x))` mais
    /// content-addressed et exécutable séparément.
    /// Pseudo-code: `run(values[b], run(values[a], input))`
    /// Limitation actuelle: accepté structurellement, mais le scalar
    /// interpreter n'a pas d'accès atlas et utilise un placeholder.
    Pipeline = 39,

    /// **Vectorize map** (origine : JAX `vmap`)
    ///
    /// `Op::Vmap(prog_hash_slot, _, _)` — méta-opération : produit un
    /// nouveau programme hash qui est la version vectorisée du programme
    /// dans `values[prog_hash_slot]`. Le programme produit prend un
    /// vec_input et retourne un vec_output. Stocké dans atlas. Stub —
    /// implémentation complète repoussée (nécessite Ty::Vec).
    /// Pseudo-code: `return vectorize(program = values[a])`
    /// Limitation actuelle: stub; dépend de `Ty::VecI64` et d'un
    /// wire format vectoriel encore absent.
    Vmap = 40,

    /// **Parallel map** (origine : JAX `pmap`)
    ///
    /// `Op::Pmap(prog_hash_slot, n_devices, _)` — comme Vmap mais
    /// distribue sur `imm` devices physiques. Stub — implémentation
    /// complète post-Phase D (WGSL universel) pour multi-GPU.
    /// Pseudo-code: `return shard_map(program = values[a], devices = imm)`
    /// Limitation actuelle: stub pur; aucun scheduler multi-device ni
    /// layout vectoriel n'est encore défini dans KASM.
    Pmap = 41,

    /// **Bounded fori loop** (origine : JAX `lax.fori_loop`)
    ///
    /// `Op::Fori(start, stop, body_prog)` — for i in start..stop {
    /// body_prog(i, accumulator) }. Bornée (start/stop sont des slots
    /// I64), pure (body_prog est référencé par hash), JIT-vectorisable.
    /// Stub — semantics définies, impl runtime TBD.
    /// Pseudo-code: `for i in start..stop { acc = run(body_prog, i, acc) }`
    /// Limitation actuelle: sémantique bornée spécifiée, mais aucun
    /// moteur de boucle concret n'est encore branché.
    Fori = 42,

    /// **Bounded while loop** (origine : JAX `lax.while_loop`)
    ///
    /// `Op::WhileLoop(cond_prog, body_prog, init_state)` — exécute
    /// body_prog tant que cond_prog(state) est vrai. Bornée par fuel
    /// pour garantir terminaison. Stub.
    /// Pseudo-code: `while fuel > 0 && run(cond_prog, state) { state = run(body_prog, state) }`
    /// Limitation actuelle: la borne par fuel existe dans la spec,
    /// mais pas encore dans un exécuteur réel.
    WhileLoop = 43,

    /// **Reduce / Fold** (origine : APL `/`, Haskell `foldl`)
    ///
    /// `Op::Reduce(prog_hash_slot, vec_slot, init_slot)` — fold
    /// l'opérateur (programme à 2 inputs) sur le vecteur, partant de
    /// init. Stub — nécessite Ty::Vec.
    /// Pseudo-code: `acc = init; for x in vec { acc = run(op, acc, x) }; return acc`
    /// Limitation actuelle: stub pur; dépend de `Ty::VecI64` et d'un
    /// stockage/wire format vectoriel non défini.
    Reduce = 44,

    /// **Scan / Prefix-sum** (origine : APL `\`, JAX `lax.scan`)
    ///
    /// `Op::Scan(prog_hash_slot, vec_slot, init_slot)` — comme Reduce
    /// mais retourne tous les résultats intermédiaires (vecteur de
    /// même longueur). Permet RNN/iterations sans boucle externe. Stub.
    /// Pseudo-code: `acc = init; for x in vec { acc = run(op, acc, x); out.push(acc) }`
    /// Limitation actuelle: stub pur; même blocage que `Reduce`, avec
    /// en plus une sortie vectorielle encore non représentable.
    Scan = 45,

    /// **Vec length query** (origine : APL `⍴` shape, NumPy `len()`,
    /// Julia `length()`)
    ///
    /// `Op::VLenI64(vec_slot)` — produit `i64` égal à la longueur du
    /// `Ty::VecI64` à `values[vec_slot]`. Première op arithmétique
    /// **runtime** sur Vec (Wave 7d) : pas de transformation, juste
    /// une query. Pseudo-code: `len(values[a])`
    /// Limitation : nécessite `Ty::VecI64` storage (Wave 7b ✅).
    VLenI64 = 46,

    /// **Vec sum reduction** (origine : APL `+/`, NumPy `np.sum()`,
    /// Julia `sum()`)
    ///
    /// `Op::VSumI64(vec_slot)` — produit `i64` égal à la somme
    /// (wrapping) des éléments du `Ty::VecI64` à `values[vec_slot]`.
    /// Empty vec → 0. Wave 7d-bis (KASM v1.1).
    /// Pseudo-code: `values[a].iter().sum::<i64>()` (wrapping)
    VSumI64 = 47,

    /// **Vec element-wise add** (origine : APL `+`, Julia `f.(x,y)`,
    /// NumPy `a + b`)
    ///
    /// `Op::VAddI64(vec_a, vec_b)` — produit nouveau `Ty::VecI64`
    /// dont les éléments sont la somme pairwise wrapping de
    /// `values[a]` et `values[b]`. Lengths doivent matcher exactement
    /// (no silent shape coercion). Wave 7d-bis (KASM v1.1).
    /// Pseudo-code: `zip(a, b).map(|(x,y)| x.wrapping_add(y)).collect()`
    VAddI64 = 48,

    /// **Vec element-wise mul** (origine : APL `×`, Julia `f.(x,y)`,
    /// NumPy `a * b`)
    ///
    /// `Op::VMulI64(vec_a, vec_b)` — produit nouveau `Ty::VecI64`
    /// dont les éléments sont le produit pairwise wrapping de
    /// `values[a]` et `values[b]`. Lengths matchent. Wave 7d-bis.
    /// Pseudo-code: `zip(a, b).map(|(x,y)| x.wrapping_mul(y)).collect()`
    VMulI64 = 49,

    /// **Vec element-wise sub** (origine : APL `-`, NumPy `a - b`,
    /// Julia `f.(x,y)`)
    ///
    /// `Op::VSubI64(vec_a, vec_b)` — pairwise wrapping subtract,
    /// lengths must match. Wave 7e (KASM v1.1).
    VSubI64 = 50,

    /// **Vec element-wise max** (origine : APL `⌈`, NumPy `np.maximum`,
    /// Julia `max.(x,y)`)
    ///
    /// `Op::VMaxI64(vec_a, vec_b)` — pairwise i64::max, lengths
    /// must match. Wave 7e.
    VMaxI64 = 51,

    /// **Vec element-wise min** (origine : APL `⌊`, NumPy `np.minimum`,
    /// Julia `min.(x,y)`)
    ///
    /// `Op::VMinI64(vec_a, vec_b)` — pairwise i64::min, lengths
    /// must match. Wave 7e.
    VMinI64 = 52,

    /// **Vec range / iota** (origine : APL `⍳`, NumPy `np.arange()`,
    /// Julia `1:n`, Haskell `[0..n-1]`)
    ///
    /// `Op::VRangeI64(len_slot)` — produit `Ty::VecI64` =
    /// `[0, 1, 2, ..., values[len_slot]-1]`. Length read from i64
    /// slot at runtime. Negative or zero length → empty vec.
    /// Wave 7e (KASM v1.1).
    /// Pseudo-code: `(0..max(0, values[a])).collect::<Vec<i64>>()`
    VRangeI64 = 53,

    /// **Vec concatenation** (origine : APL `,`, NumPy `np.concatenate`,
    /// Julia `vcat`, Haskell `++`)
    ///
    /// `Op::VConcatI64(vec_a, vec_b)` — produit nouveau Vec dont les
    /// éléments sont ceux de `values[a]` suivis de ceux de `values[b]`.
    /// Lengths peuvent différer (concatenation, pas pairwise).
    /// Wave 7f (KASM v1.1).
    /// Pseudo-code: `[values[a], values[b]].concat()`
    VConcatI64 = 54,

    /// **Vec reverse** (origine : APL `⌽`, NumPy `[::-1]`,
    /// Julia `reverse()`, Haskell `reverse`)
    ///
    /// `Op::VReverseI64(vec_slot)` — produit nouveau Vec en ordre
    /// inverse de `values[a]`. Wave 7f.
    /// Pseudo-code: `values[a].iter().rev().collect()`
    VReverseI64 = 55,

    /// **Vec broadcast / fill** (origine : NumPy `np.full(n, v)`,
    /// APL `n⍴v` (reshape with scalar), Julia `fill(v, n)`)
    ///
    /// `Op::VBroadcastI64(value_slot, len_slot)` — produit Vec de
    /// longueur `values[b]` rempli avec `values[a]` (scalaire i64).
    /// Negative ou zero length → empty vec. Wave 7f.
    /// Pseudo-code: `vec![values[a]; max(0, values[b]) as usize]`
    VBroadcastI64 = 56,

    /// **Vec element-wise equality** (origine : NumPy `a == b`,
    /// APL `=`, Julia `f.(x,y)`)
    ///
    /// `Op::VEqI64(vec_a, vec_b)` — pairwise equality, produces Vec
    /// where each element is 1 if equal, 0 otherwise. Lengths must
    /// match. Wave 7g (KASM v1.1).
    VEqI64 = 57,

    /// **Vec element-wise bitwise AND** (origine : NumPy `a & b`,
    /// APL `∧`, Julia `f.(x,y)`)
    ///
    /// `Op::VAndI64(vec_a, vec_b)` — pairwise bitwise AND, lengths
    /// must match. Wave 7g.
    VAndI64 = 58,

    /// **Vec element-wise bitwise OR** (origine : NumPy `a | b`,
    /// APL `∨`, Julia `f.(x,y)`)
    ///
    /// `Op::VOrI64(vec_a, vec_b)` — pairwise bitwise OR, lengths
    /// must match. Wave 7g.
    VOrI64 = 59,

    /// **Vec element-wise bitwise XOR** (origine : NumPy `a ^ b`,
    /// APL `≠` (not-equal), Julia `f.(x,y)`)
    ///
    /// `Op::VXorI64(vec_a, vec_b)` — pairwise bitwise XOR, lengths
    /// must match. Wave 7g.
    VXorI64 = 60,

    /// **Vec absolute value** (origine : NumPy `np.abs`, APL `|`,
    /// Julia `abs.(x)`)
    ///
    /// `Op::VAbsI64(vec_slot)` — element-wise i64::wrapping_abs.
    /// Wave 7h.
    VAbsI64 = 61,

    /// **Vec negate** (origine : NumPy `-x`, APL `-`, Julia `-x`)
    ///
    /// `Op::VNegI64(vec_slot)` — element-wise wrapping_neg. Wave 7h.
    VNegI64 = 62,

    /// **Vec bitwise NOT** (origine : NumPy `~x`, APL `~`, Julia `.~x`)
    ///
    /// `Op::VBitFlipI64(vec_slot)` — element-wise bitwise NOT.
    /// Wave 7h.
    VBitFlipI64 = 63,

    /// **Fractal** (origine : Wave 8 self-hosting, Forge écrite en Forge)
    ///
    /// `Op::Fractal(callee_hash_slot, args_slot)` — invoque un autre
    /// programme KASM identifié par hash, en passant `args_slot` comme
    /// inputs. La sortie devient la valeur du Node Fractal.
    ///
    /// Wave 8 minimal : STUB fail-loud dans tous les consumers
    /// (interpreter, JIT, optimizer, MLIR, agent rebuild, CUDA).
    /// La sémantique réelle vit dans `kasm::self_host::SelfHostingRuntime`
    /// au niveau runtime (pas du bytecode interprété directement).
    /// Future Wave 11+ : wiring complet vers le bytecode interpreter.
    Fractal = 64,

    /// **Eval** (origine : Wave 8 self-hosting, programme-as-data)
    ///
    /// `Op::Eval(prog_bytes_slot, args_slot)` — interprète un programme
    /// KASM construit à l'exécution (bytes d'un programme valide stockés
    /// dans un slot). Permet la métaprogrammation : un programme peut
    /// **construire et exécuter** un autre programme KASM.
    ///
    /// Wave 8 minimal : STUB fail-loud (idem Fractal). La self-hosting
    /// runtime gère cette sémantique au niveau RPC.
    Eval = 65,

    /// **Vec random-access read** (origine : NumPy `a[i]`, APL `i⊃v`,
    /// Julia `v[i]`, C `v[i]`)
    ///
    /// `Op::VGetI64(vec_slot, idx_slot)` — reads `values[vec][idx]` and
    /// produces the resulting `i64`. The `idx_slot` carries an `i64`
    /// index which is interpreted as **unsigned modulo `len`** for total-
    /// function discipline : empty vec → 0, otherwise
    /// `vec[(idx as u64 % len) as usize]`. No panic, no UB, deterministic.
    ///
    /// Wave 7i (KASM v1.2). Unlocks self-hosted KASM interpreters
    /// (Λ.2) — a program receiving its own bytecode as a `Ty::VecI64`
    /// input can now decode individual node fields by index. Same
    /// primitive enables Λ.3 (synth-as-KASM) which needs random access
    /// into example arrays.
    ///
    /// Pseudo-code: `values[a][((values[b] as u64) % values[a].len() as u64) as usize]`
    /// (returns 0 when `values[a].len() == 0`)
    VGetI64 = 66,

    /// Hardware-friendly population count over the 64-bit bit pattern.
    /// Uses POPCNT when the CPU exposes it, with a portable scalar fallback.
    PopcntI64 = 67,

    /// Count leading zero bits over the 64-bit bit pattern.
    /// Uses LZCNT when available, preserving Rust semantics for zero (64).
    LzcntI64 = 68,

    /// Count trailing zero bits over the 64-bit bit pattern.
    /// Uses TZCNT/BMI1 when available, preserving Rust semantics for zero (64).
    TzcntI64 = 69,

    /// Parallel bit extract: compact bits from `a` selected by mask `b`.
    /// Uses BMI2 PEXT when available, otherwise a bit-exact software fallback.
    PextI64 = 70,

    /// Parallel bit deposit: scatter low bits from `a` into mask `b`.
    /// Uses BMI2 PDEP when available, otherwise a bit-exact software fallback.
    PdepI64 = 71,

    /// Deferred computation marker.
    ///
    /// `Op::Lazy(child_ref)` produces a deterministic future hash in an
    /// ordinary i64 slot. The full 32-byte future key is
    /// `H("KASM:FUTURE:v1", program_bytes, input_bytes, child_node_id)`;
    /// the i64 value is the low 64 bits of that key. `Force` resolves the
    /// future in the runtime that owns the atlas/store, or falls back to
    /// the referenced child value in the scalar interpreter.
    Lazy = 72,

    /// Resolve a future produced by `Op::Lazy`.
    ///
    /// Bare scalar KASM validates the future comes from a `Lazy` node in
    /// this execution and returns the child value. Forge brain runtimes can
    /// intercept the full future key for RESULT atlas lookup before any
    /// child computation is scheduled.
    Force = 73,
}

// --- Φ.0 — F64Op sub-op selectors (low byte of `Node::imm`) ---
//
// Layout chosen so binary ops cluster (0-5), F64-unary (6-8), then the
// two cross-domain conversion ops (9-10). Synthesizer can enumerate
// `0..=10` to cover the full F64 surface without case analysis.

pub const F64_ADD: u8 = 0;
pub const F64_SUB: u8 = 1;
pub const F64_MUL: u8 = 2;
pub const F64_DIV: u8 = 3;
pub const F64_MIN: u8 = 4;
pub const F64_MAX: u8 = 5;
pub const F64_SQRT: u8 = 6;
pub const F64_ABS: u8 = 7;
pub const F64_NEG: u8 = 8;
pub const F64_FROM_I64: u8 = 9;
pub const F64_TO_I64: u8 = 10;
/// Φ.7a — transcendentals via libstd. **Non-deterministic across libc
/// versions**: paired memos shared across heterogeneous hosts MAY
/// disagree on the last few ULPs of an exp/log result. Same caveat
/// as KASM-Tensor's TanhF32/SigmoidF32 — the synthesizer's holdout
/// kill-switch absorbs cross-host drift on the 1-ULP boundary.
pub const F64_EXP: u8 = 11;
pub const F64_LN: u8 = 12;
/// Highest legal sub-op selector for `F64Op`. Used in régression
/// tests (`kasm/tests.rs`) ; `#[allow(dead_code)]` car aucun call
/// site dans le lib release path — la constante est `pub` pour les
/// consommateurs externes (lab_runner, future fuzzer).
#[allow(dead_code)]
pub const F64_OP_MAX: u8 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum F64SubOp {
    Add,
    Sub,
    Mul,
    DivChecked,
    Min,
    Max,
    Sqrt,
    Abs,
    Neg,
    FromI64,
    ToI64,
    /// Φ.7a — `e^a`. Total: NaN/Inf collapses to 0.0 (same kill-switch
    /// discipline as DivChecked / Sqrt). Implementation uses
    /// `f64::exp` from libstd — non-deterministic across libc versions
    /// at the last few ULPs.
    Exp,
    /// Φ.7a — `ln(|a|)` (natural log of absolute value). The `|·|`
    /// is baked in so the op is total: ln(0) → 0.0, ln(neg) →
    /// ln(|neg|). Implementation uses `f64::ln` from libstd.
    Ln,
}

impl F64SubOp {
    /// Decode the sub-op from the low byte of a `F64Op` `imm` field.
    /// `imm` is `i16` but only the low byte carries semantics — high
    /// byte is reserved (must be zero on encode).
    pub fn from_imm(imm: i16) -> Result<Self, KasmError> {
        let high = (imm >> 8) & 0xff;
        if high != 0 {
            return Err(KasmError::BadF64SubOp(imm));
        }
        match (imm & 0xff) as u8 {
            F64_ADD => Ok(F64SubOp::Add),
            F64_SUB => Ok(F64SubOp::Sub),
            F64_MUL => Ok(F64SubOp::Mul),
            F64_DIV => Ok(F64SubOp::DivChecked),
            F64_MIN => Ok(F64SubOp::Min),
            F64_MAX => Ok(F64SubOp::Max),
            F64_SQRT => Ok(F64SubOp::Sqrt),
            F64_ABS => Ok(F64SubOp::Abs),
            F64_NEG => Ok(F64SubOp::Neg),
            F64_FROM_I64 => Ok(F64SubOp::FromI64),
            F64_TO_I64 => Ok(F64SubOp::ToI64),
            F64_EXP => Ok(F64SubOp::Exp),
            F64_LN => Ok(F64SubOp::Ln),
            _ => Err(KasmError::BadF64SubOp(imm)),
        }
    }

    pub fn imm(self) -> i16 {
        match self {
            F64SubOp::Add => F64_ADD as i16,
            F64SubOp::Sub => F64_SUB as i16,
            F64SubOp::Mul => F64_MUL as i16,
            F64SubOp::DivChecked => F64_DIV as i16,
            F64SubOp::Min => F64_MIN as i16,
            F64SubOp::Max => F64_MAX as i16,
            F64SubOp::Sqrt => F64_SQRT as i16,
            F64SubOp::Abs => F64_ABS as i16,
            F64SubOp::Neg => F64_NEG as i16,
            F64SubOp::FromI64 => F64_FROM_I64 as i16,
            F64SubOp::ToI64 => F64_TO_I64 as i16,
            F64SubOp::Exp => F64_EXP as i16,
            F64SubOp::Ln => F64_LN as i16,
        }
    }

    /// Result type (ty stored on the Node).
    pub fn result_ty(self) -> Ty {
        match self {
            F64SubOp::ToI64 => Ty::I64,
            _ => Ty::F64,
        }
    }

    /// Type of the `a` operand.
    pub fn a_ty(self) -> Ty {
        match self {
            F64SubOp::FromI64 => Ty::I64,
            _ => Ty::F64,
        }
    }

    /// Type of the `b` operand if binary, else `None`.
    pub fn b_ty(self) -> Option<Ty> {
        match self {
            F64SubOp::Add
            | F64SubOp::Sub
            | F64SubOp::Mul
            | F64SubOp::DivChecked
            | F64SubOp::Min
            | F64SubOp::Max => Some(Ty::F64),
            _ => None,
        }
    }

    pub fn is_binary(self) -> bool {
        self.b_ty().is_some()
    }
}

impl Op {
    pub(super) fn from_byte(b: u8) -> Result<Self, KasmError> {
        match b {
            0 => Ok(Op::Input),
            1 => Ok(Op::ConstI64),
            2 => Ok(Op::AddI64),
            3 => Ok(Op::MulI64),
            4 => Ok(Op::EqI64),
            5 => Ok(Op::Hash64),
            6 => Ok(Op::Output),
            7 => Ok(Op::SubI64),
            8 => Ok(Op::DivI64Checked),
            9 => Ok(Op::MinI64),
            10 => Ok(Op::MaxI64),
            11 => Ok(Op::SelectI64),
            12 => Ok(Op::AndBool),
            13 => Ok(Op::OrBool),
            14 => Ok(Op::NotBool),
            15 => Ok(Op::LtI64),
            16 => Ok(Op::LeI64),
            17 => Ok(Op::BitAndI64),
            18 => Ok(Op::BitOrI64),
            19 => Ok(Op::BitXorI64),
            20 => Ok(Op::ShlI64),
            21 => Ok(Op::ShrI64),
            22 => Ok(Op::SatAddI64),
            23 => Ok(Op::SatSubI64),
            24 => Ok(Op::ModI64Checked),
            25 => Ok(Op::ClampI64),
            26 => Ok(Op::ReduceAddI64),
            27 => Ok(Op::ReduceMulI64),
            28 => Ok(Op::BitFlipI64),
            29 => Ok(Op::NegI64),
            30 => Ok(Op::ReverseBitsI64),
            31 => Ok(Op::ByteswapI64),
            32 => Ok(Op::ConstF64),
            33 => Ok(Op::F64Op),
            // KASM v1.0 mutation
            34 => Ok(Op::Adaptive),
            35 => Ok(Op::Comptime),
            36 => Ok(Op::Grad),
            37 => Ok(Op::Cond),
            38 => Ok(Op::Memoize),
            39 => Ok(Op::Pipeline),
            40 => Ok(Op::Vmap),
            41 => Ok(Op::Pmap),
            42 => Ok(Op::Fori),
            43 => Ok(Op::WhileLoop),
            44 => Ok(Op::Reduce),
            45 => Ok(Op::Scan),
            46 => Ok(Op::VLenI64),
            47 => Ok(Op::VSumI64),
            48 => Ok(Op::VAddI64),
            49 => Ok(Op::VMulI64),
            50 => Ok(Op::VSubI64),
            51 => Ok(Op::VMaxI64),
            52 => Ok(Op::VMinI64),
            53 => Ok(Op::VRangeI64),
            54 => Ok(Op::VConcatI64),
            55 => Ok(Op::VReverseI64),
            56 => Ok(Op::VBroadcastI64),
            57 => Ok(Op::VEqI64),
            58 => Ok(Op::VAndI64),
            59 => Ok(Op::VOrI64),
            60 => Ok(Op::VXorI64),
            61 => Ok(Op::VAbsI64),
            62 => Ok(Op::VNegI64),
            63 => Ok(Op::VBitFlipI64),
            64 => Ok(Op::Fractal),
            65 => Ok(Op::Eval),
            66 => Ok(Op::VGetI64),
            67 => Ok(Op::PopcntI64),
            68 => Ok(Op::LzcntI64),
            69 => Ok(Op::TzcntI64),
            70 => Ok(Op::PextI64),
            71 => Ok(Op::PdepI64),
            72 => Ok(Op::Lazy),
            73 => Ok(Op::Force),
            _ => Err(KasmError::BadOp(b)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    pub op: Op,
    pub ty: Ty,
    pub a: u16,
    pub b: u16,
    pub imm: i16,
}

impl Node {
    pub fn input(slot: u8) -> Self {
        Self { op: Op::Input, ty: Ty::I64, a: 0, b: 0, imm: slot as i16 }
    }

    pub fn const_i64(value: i16) -> Self {
        Self { op: Op::ConstI64, ty: Ty::I64, a: 0, b: 0, imm: value }
    }

    pub fn add(a: u16, b: u16) -> Self {
        Self { op: Op::AddI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn mul(a: u16, b: u16) -> Self {
        Self { op: Op::MulI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn sub(a: u16, b: u16) -> Self {
        Self { op: Op::SubI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn div_checked(a: u16, b: u16) -> Self {
        Self { op: Op::DivI64Checked, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn min(a: u16, b: u16) -> Self {
        Self { op: Op::MinI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn max(a: u16, b: u16) -> Self {
        Self { op: Op::MaxI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn eq(a: u16, b: u16) -> Self {
        Self { op: Op::EqI64, ty: Ty::Bool, a, b, imm: 0 }
    }

    pub fn select_i64(cond: u16, if_true: u16, if_false: u16) -> Self {
        Self { op: Op::SelectI64, ty: Ty::I64, a: cond, b: if_true, imm: if_false as i16 }
    }

    pub fn and(a: u16, b: u16) -> Self {
        Self { op: Op::AndBool, ty: Ty::Bool, a, b, imm: 0 }
    }

    pub fn or(a: u16, b: u16) -> Self {
        Self { op: Op::OrBool, ty: Ty::Bool, a, b, imm: 0 }
    }

    pub fn not(a: u16) -> Self {
        Self { op: Op::NotBool, ty: Ty::Bool, a, b: 0, imm: 0 }
    }

    pub fn hash64(a: u16) -> Self {
        Self { op: Op::Hash64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    pub fn output(a: u16, ty: Ty) -> Self {
        Self { op: Op::Output, ty, a, b: 0, imm: 0 }
    }

    // --- v0.2 builders ---

    pub fn lt(a: u16, b: u16) -> Self {
        Self { op: Op::LtI64, ty: Ty::Bool, a, b, imm: 0 }
    }

    pub fn le(a: u16, b: u16) -> Self {
        Self { op: Op::LeI64, ty: Ty::Bool, a, b, imm: 0 }
    }

    pub fn bit_and(a: u16, b: u16) -> Self {
        Self { op: Op::BitAndI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn bit_or(a: u16, b: u16) -> Self {
        Self { op: Op::BitOrI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn bit_xor(a: u16, b: u16) -> Self {
        Self { op: Op::BitXorI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn shl(a: u16, b: u16) -> Self {
        Self { op: Op::ShlI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn shr(a: u16, b: u16) -> Self {
        Self { op: Op::ShrI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn sat_add(a: u16, b: u16) -> Self {
        Self { op: Op::SatAddI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn sat_sub(a: u16, b: u16) -> Self {
        Self { op: Op::SatSubI64, ty: Ty::I64, a, b, imm: 0 }
    }

    pub fn mod_checked(a: u16, b: u16) -> Self {
        Self { op: Op::ModI64Checked, ty: Ty::I64, a, b, imm: 0 }
    }

    /// Clamp `a` between `lo` and `hi` — implemented as
    /// `min(max(a, lo), hi)` so the result stays defined when
    /// `lo > hi`.
    pub fn clamp(a: u16, lo: u16, hi: u16) -> Self {
        Self { op: Op::ClampI64, ty: Ty::I64, a, b: lo, imm: hi as i16 }
    }

    /// Reduce the I64 nodes at indices `[base, base + count)` by
    /// addition. `count >= 1`.
    pub fn reduce_add(base: u16, count: u16) -> Self {
        Self { op: Op::ReduceAddI64, ty: Ty::I64, a: base, b: 0, imm: count as i16 }
    }

    /// Reduce the I64 nodes at indices `[base, base + count)` by
    /// multiplication. `count >= 1`.
    pub fn reduce_mul(base: u16, count: u16) -> Self {
        Self { op: Op::ReduceMulI64, ty: Ty::I64, a: base, b: 0, imm: count as i16 }
    }

    /// Bitwise complement (Ω-6.1, bijective).
    pub fn bit_flip(a: u16) -> Self {
        Self { op: Op::BitFlipI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    /// Two's complement negation (Ω-6.1, bijective via wrapping).
    pub fn neg(a: u16) -> Self {
        Self { op: Op::NegI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    /// Reverse the 64 bits (Ω-6.1, bijective involution).
    pub fn reverse_bits(a: u16) -> Self {
        Self { op: Op::ReverseBitsI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    /// Swap the 8 bytes — endian flip (Ω-6.1, bijective involution).
    pub fn byteswap(a: u16) -> Self {
        Self { op: Op::ByteswapI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    pub fn popcnt(a: u16) -> Self {
        Self { op: Op::PopcntI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    pub fn lzcnt(a: u16) -> Self {
        Self { op: Op::LzcntI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    pub fn tzcnt(a: u16) -> Self {
        Self { op: Op::TzcntI64, ty: Ty::I64, a, b: 0, imm: 0 }
    }

    pub fn pext(a: u16, mask: u16) -> Self {
        Self { op: Op::PextI64, ty: Ty::I64, a, b: mask, imm: 0 }
    }

    pub fn pdep(a: u16, mask: u16) -> Self {
        Self { op: Op::PdepI64, ty: Ty::I64, a, b: mask, imm: 0 }
    }

    pub fn lazy(child: u16) -> Self {
        Self { op: Op::Lazy, ty: Ty::I64, a: child, b: 0, imm: 0 }
    }

    pub fn force(future: u16) -> Self {
        Self { op: Op::Force, ty: Ty::I64, a: future, b: 0, imm: 0 }
    }

    // --- KASM v1.0 mutation builders (Phase 11) ---

    /// `Op::Adaptive(slot)` — auto-tune wrapper, `imm` indexes the
    /// configuration family (block_size variations, vec_lanes, etc).
    pub fn adaptive(slot: u16, family: i16) -> Self {
        Self { op: Op::Adaptive, ty: Ty::I64, a: slot, b: 0, imm: family }
    }

    /// `Op::Comptime(slot)` — evaluate at load time, inline result.
    pub fn comptime(slot: u16) -> Self {
        Self { op: Op::Comptime, ty: Ty::I64, a: slot, b: 0, imm: 0 }
    }

    /// `Op::Grad(slot, var_idx)` — symbolic derivative w.r.t. Input(var_idx).
    pub fn grad(slot: u16, var_idx: u8) -> Self {
        Self { op: Op::Grad, ty: Ty::I64, a: slot, b: 0, imm: var_idx as i16 }
    }

    /// `Op::Cond(pred, then_slot, else_slot)` — functional if-then-else.
    pub fn cond(pred: u16, then_slot: u16, else_slot: u16) -> Self {
        Self { op: Op::Cond, ty: Ty::I64, a: pred, b: then_slot, imm: else_slot as i16 }
    }

    /// `Op::Memoize(slot)` — force memoization of slot's value.
    pub fn memoize(slot: u16) -> Self {
        Self { op: Op::Memoize, ty: Ty::I64, a: slot, b: 0, imm: 0 }
    }

    /// `Op::Pipeline(prog_a_slot, prog_b_slot)` — compose two programs.
    pub fn pipeline(prog_a: u16, prog_b: u16) -> Self {
        Self { op: Op::Pipeline, ty: Ty::I64, a: prog_a, b: prog_b, imm: 0 }
    }

    // --- Φ.0 — F64 builders ---

    /// `f64` constant from a small integer (`-32_768.0..=32_767.0`).
    /// Larger or non-integer constants must be built with
    /// `const_i64` + `f64_from_i64` + `f64_div`.
    pub fn const_f64(value: i16) -> Self {
        Self { op: Op::ConstF64, ty: Ty::F64, a: 0, b: 0, imm: value }
    }

    /// Internal: dispatch to F64Op with sub-op selector encoded in `imm`.
    fn f64_op(sub: F64SubOp, a: u16, b: u16) -> Self {
        Self {
            op: Op::F64Op,
            ty: sub.result_ty(),
            a,
            b,
            imm: sub.imm(),
        }
    }

    pub fn f64_add(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::Add, a, b)
    }

    pub fn f64_sub(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::Sub, a, b)
    }

    pub fn f64_mul(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::Mul, a, b)
    }

    pub fn f64_div(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::DivChecked, a, b)
    }

    pub fn f64_min(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::Min, a, b)
    }

    pub fn f64_max(a: u16, b: u16) -> Self {
        Self::f64_op(F64SubOp::Max, a, b)
    }

    pub fn f64_sqrt(a: u16) -> Self {
        Self::f64_op(F64SubOp::Sqrt, a, 0)
    }

    pub fn f64_abs(a: u16) -> Self {
        Self::f64_op(F64SubOp::Abs, a, 0)
    }

    pub fn f64_neg(a: u16) -> Self {
        Self::f64_op(F64SubOp::Neg, a, 0)
    }

    /// Cast an `i64` value to `f64` (lossy for magnitudes above 2^53).
    pub fn f64_from_i64(a: u16) -> Self {
        Self::f64_op(F64SubOp::FromI64, a, 0)
    }

    /// Truncate an `f64` value to `i64`. Returns `0` on `NaN` / `±Inf`
    /// (total function — synthesizer guarantee).
    pub fn f64_to_i64(a: u16) -> Self {
        Self::f64_op(F64SubOp::ToI64, a, 0)
    }

    /// Φ.7a — `e^a`. Total: NaN/Inf collapses to 0.0.
    pub fn f64_exp(a: u16) -> Self {
        Self::f64_op(F64SubOp::Exp, a, 0)
    }

    /// Φ.7a — `ln(|a|)`. Total: ln(0) → 0.0, ln(neg) → ln(|neg|).
    pub fn f64_ln(a: u16) -> Self {
        Self::f64_op(F64SubOp::Ln, a, 0)
    }

    /// Build an `Input` node typed as `Ty::F64`. The runtime reads the
    /// same 8 little-endian bytes as for an `Input` with `Ty::I64`,
    /// but downstream `F64Op` sub-ops will interpret those bytes via
    /// `f64::from_bits`.
    pub fn input_f64(slot: u8) -> Self {
        Self { op: Op::Input, ty: Ty::F64, a: 0, b: 0, imm: slot as i16 }
    }

    /// Wave 7b — build an `Input` node typed as `Ty::VecI64`. The
    /// runtime reads the wire format `[u32 LE count | count × 8 bytes
    /// i64 LE]` from the args at this slot's offset, computed by the
    /// per-slot parser in `kasm::execute()`.
    pub fn input_vec(slot: u8) -> Self {
        Self { op: Op::Input, ty: Ty::VecI64, a: 0, b: 0, imm: slot as i16 }
    }

    /// Wave 7d — build a `VLenI64` node : `Op::VLenI64(vec_slot) → i64`.
    /// First runtime arithmetic op on Vec values (just a length query,
    /// no transformation).
    pub fn v_len(vec_slot: u16) -> Self {
        Self { op: Op::VLenI64, ty: Ty::I64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7d-bis — Vec sum reduction (Vec → i64).
    pub fn v_sum(vec_slot: u16) -> Self {
        Self { op: Op::VSumI64, ty: Ty::I64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7d-bis — Vec element-wise add (Vec, Vec → Vec).
    pub fn v_add(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VAddI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7d-bis — Vec element-wise mul (Vec, Vec → Vec).
    pub fn v_mul(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VMulI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7e — Vec element-wise sub (Vec, Vec → Vec).
    pub fn v_sub(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VSubI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7e — Vec element-wise max (Vec, Vec → Vec).
    pub fn v_max(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VMaxI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7e — Vec element-wise min (Vec, Vec → Vec).
    pub fn v_min(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VMinI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7e — Vec range/iota (i64 length → Vec [0..len)).
    pub fn v_range(len_slot: u16) -> Self {
        Self { op: Op::VRangeI64, ty: Ty::VecI64, a: len_slot, b: 0, imm: 0 }
    }

    /// Wave 7f — Vec concatenation (Vec, Vec → Vec).
    pub fn v_concat(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VConcatI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7f — Vec reverse (Vec → Vec).
    pub fn v_reverse(vec_slot: u16) -> Self {
        Self { op: Op::VReverseI64, ty: Ty::VecI64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7f — Vec broadcast/fill (i64 value, i64 length → Vec).
    pub fn v_broadcast(value_slot: u16, len_slot: u16) -> Self {
        Self { op: Op::VBroadcastI64, ty: Ty::VecI64, a: value_slot, b: len_slot, imm: 0 }
    }

    /// Wave 7g — Vec element-wise equality (Vec, Vec → Vec of 0/1).
    pub fn v_eq(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VEqI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7g — Vec element-wise bitwise AND.
    pub fn v_and(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VAndI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7g — Vec element-wise bitwise OR.
    pub fn v_or(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VOrI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7g — Vec element-wise bitwise XOR.
    pub fn v_xor(vec_a: u16, vec_b: u16) -> Self {
        Self { op: Op::VXorI64, ty: Ty::VecI64, a: vec_a, b: vec_b, imm: 0 }
    }

    /// Wave 7h — Vec element-wise abs.
    pub fn v_abs(vec_slot: u16) -> Self {
        Self { op: Op::VAbsI64, ty: Ty::VecI64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7h — Vec element-wise negate.
    pub fn v_neg(vec_slot: u16) -> Self {
        Self { op: Op::VNegI64, ty: Ty::VecI64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7h — Vec element-wise bitwise NOT.
    pub fn v_bit_flip(vec_slot: u16) -> Self {
        Self { op: Op::VBitFlipI64, ty: Ty::VecI64, a: vec_slot, b: 0, imm: 0 }
    }

    /// Wave 7i — Vec random-access read : `vec[idx % len]` → i64.
    /// Empty vec → 0. Total function (no panic). Unlocks self-hosted
    /// KASM interpreters by making program bytecode (passed as VecI64)
    /// addressable field-by-field.
    pub fn v_get(vec_slot: u16, idx_slot: u16) -> Self {
        Self { op: Op::VGetI64, ty: Ty::I64, a: vec_slot, b: idx_slot, imm: 0 }
    }

    pub(super) fn encode(self, out: &mut Vec<u8>) {
        out.push(self.op as u8);
        out.push(self.ty as u8);
        out.extend_from_slice(&self.a.to_le_bytes());
        out.extend_from_slice(&self.b.to_le_bytes());
        out.extend_from_slice(&self.imm.to_le_bytes());
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, KasmError> {
        if bytes.len() != NODE_LEN {
            return Err(KasmError::Truncated);
        }
        Ok(Self {
            op: Op::from_byte(bytes[0])?,
            ty: Ty::from_byte(bytes[1])?,
            a: u16::from_le_bytes(bytes[2..4].try_into().unwrap()),
            b: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            imm: i16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialEvalReport {
    pub original_nodes: usize,
    pub residual_nodes: usize,
    pub eliminated_nodes: usize,
    pub residual_ratio: f64,
    pub is_static: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteReport {
    pub passes: usize,
    pub residual_nodes: usize,
    pub reduced_to_constant: bool,
}

impl PartialEvalReport {
    pub(super) fn from_programs(
        original_nodes: usize,
        residual_nodes: usize,
        is_static: bool,
    ) -> Self {
        let eliminated_nodes = original_nodes.saturating_sub(residual_nodes);
        let residual_ratio = if original_nodes == 0 {
            0.0
        } else {
            residual_nodes as f64 / original_nodes as f64
        };
        Self {
            original_nodes,
            residual_nodes,
            eliminated_nodes,
            residual_ratio,
            is_static,
        }
    }
}

#[derive(Debug)]
pub enum KasmError {
    BadMagic,
    BadVersion(u8),
    BadTarget(u8),
    BadType(u8),
    BadOp(u8),
    BadLength,
    BadFooter,
    BadNodeCount(usize),
    TooManySlots,
    FuelTooSmall,
    Truncated,
    BadInputLength { expected: usize, got: usize },
    BadInputSlot { node: usize, slot: i16 },
    BadRef { node: usize, reference: u16 },
    TypeMismatch { node: usize },
    OutputCount { expected: u8, got: u8 },
    ValueTypeMismatch { node: usize },
    ComposeArity { left_outputs: u8, right_inputs: u8 },
    ComposeType { slot: usize, left: Ty, right: Ty },
    ExternalTarget(Target),
    /// `ReduceAddI64` / `ReduceMulI64` saw `count == 0` or
    /// `base + count > current_node_index`.
    BadReduceCount { node: usize, count: i16 },
    /// Φ.0 — `F64Op::imm` carries a sub-op selector that is either
    /// out of range (`> F64_OP_MAX`) or has non-zero bits in its
    /// reserved high byte.
    BadF64SubOp(i16),
    /// KASM v1.0 — a meta-op (Vmap/Pmap/Grad/Fori/WhileLoop/Reduce/Scan)
    /// reached the scalar interpreter. These ops require runtime support
    /// only available in the Forge brain (atlas lookup, vector storage,
    /// parallel execution). Calling them in the bare KASM interpreter is
    /// a programming error — the dispatch layer should intercept first.
    UnsupportedV1OpInScalarInterpreter { node: usize, op_byte: u8 },
    /// Wave 4 (Phase Ω.10) — `MultiMethod` blob is malformed: bad magic,
    /// truncated method table, unsupported version, etc. The String
    /// detail is appended to the error message for forensic diagnostics.
    BadMultiMethod(String),
}

impl fmt::Display for KasmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KasmError::BadMagic => write!(f, "bad KASM magic"),
            KasmError::BadVersion(v) => write!(f, "unsupported KASM version {v}"),
            KasmError::BadTarget(t) => write!(f, "bad KASM target {t}"),
            KasmError::BadType(t) => write!(f, "bad KASM type {t}"),
            KasmError::BadOp(op) => write!(f, "bad KASM op {op}"),
            KasmError::BadLength => write!(f, "bad KASM byte length"),
            KasmError::BadFooter => write!(f, "KASM footer hash mismatch"),
            KasmError::BadNodeCount(n) => write!(f, "bad KASM node count {n}"),
            KasmError::TooManySlots => write!(f, "too many KASM input/output slots"),
            KasmError::FuelTooSmall => write!(f, "KASM fuel is smaller than node count"),
            KasmError::Truncated => write!(f, "truncated KASM node"),
            KasmError::BadInputLength { expected, got } => {
                write!(f, "bad KASM input length: expected {expected} bytes, got {got}")
            }
            KasmError::BadInputSlot { node, slot } => {
                write!(f, "node {node} reads invalid input slot {slot}")
            }
            KasmError::BadRef { node, reference } => {
                write!(f, "node {node} references future/missing node {reference}")
            }
            KasmError::TypeMismatch { node } => write!(f, "type mismatch at node {node}"),
            KasmError::OutputCount { expected, got } => {
                write!(f, "expected {expected} output nodes, got {got}")
            }
            KasmError::ValueTypeMismatch { node } => write!(f, "value type mismatch at node {node}"),
            KasmError::ComposeArity { left_outputs, right_inputs } => {
                write!(f, "cannot compose: left has {left_outputs} outputs, right has {right_inputs} inputs")
            }
            KasmError::ComposeType { slot, left, right } => {
                write!(f, "cannot compose slot {slot}: left {left:?} != right {right:?}")
            }
            KasmError::ExternalTarget(target) => write!(f, "{target:?} requires an external backend"),
            KasmError::BadReduceCount { node, count } => {
                write!(f, "node {node}: reduce count {count} out of range")
            }
            KasmError::BadF64SubOp(imm) => {
                write!(f, "F64Op sub-op selector {imm:#06x} out of range")
            }
            KasmError::UnsupportedV1OpInScalarInterpreter { node, op_byte } => {
                write!(
                    f,
                    "KASM v1.0 op {op_byte} at node {node} requires Forge brain dispatch \
                     (atlas/vector support); not executable in the scalar interpreter"
                )
            }
            KasmError::BadMultiMethod(detail) => {
                write!(f, "bad MultiMethod blob: {detail}")
            }
        }
    }
}

impl std::error::Error for KasmError {}
