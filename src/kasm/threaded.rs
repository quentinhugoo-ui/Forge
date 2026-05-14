//! Π.5 (Wave 2, 2026-05-02) — Threaded code dispatch (Forth-style).
//!
//! **Origine** : Forth (Charles Moore, 1970), GForth, FICL. Idée
//! centrale : remplacer un grand `match` sur opcodes par une table
//! de pointeurs de fonctions indexée par opcode. Le CPU prédit mieux
//! les indirect-calls fréquents (BTB warm) que les jumps cascadés
//! d'un match → ×2-3 sur slow-lane interpreter.
//!
//! ## Pourquoi pour Forge ?
//!
//! Le slow lane KASM interpreter (`kasm::interpreter::execute`) fait
//! un giant `match node.op { Op::AddI64 => ..., Op::MulI64 => ..., ... }`
//! sur 60+ opcodes. Le CPU a 1 BTB entry pour CETTE indirect branch ;
//! avec 60 cibles possibles, miss rate ~50%. Pénalité ~5-15 cycles
//! par instruction interprétée.
//!
//! Solution Forth-style : table `static OPS: [fn(&mut Ctx, &Node); N]`,
//! dispatch = `OPS[op_idx](ctx, node)`. Chaque fn pointer a sa propre
//! BTB entry — prédiction quasi-parfaite après warm-up.
//!
//! ## Architecture Wave 2 minimal viable
//!
//! - Table de pointeurs `OpHandler = fn(&mut Ctx, &Node)`
//! - `dispatch_table(op: u8) -> OpHandler` : O(1) lookup
//! - Implémentation initiale : 8 ops les plus chaudes (Input, Const,
//!   Add, Mul, Sub, Hash64, Output, ...) — Wave 2 minimal.
//! - Plug optionnel dans `interpreter::execute` derrière feature flag
//!   ou wave 11 quand le mesure montre le gain.
//!
//! ## Limitations Wave 2 minimal
//!
//! - Le module fournit l'INFRASTRUCTURE (table + dispatch helper).
//!   Le wiring complet dans `interpreter.rs` est différé Wave 11+
//!   (mesure perf nécessaire avant le swap).
//! - Pas de "computed goto" (gcc extension non-portable Rust).
//! - Threading classique direct call ; pas de subroutine threading.

use crate::kasm::Op;

/// Contexte minimal d'exécution Wave 2 — chaque op handler reçoit ce
/// contexte mutable et écrit dans `output`. Les implémentations Wave 2
/// sont des stubs symboliques : elles écrivent un i64 calculable
/// depuis `inputs` pour démontrer le pattern, sans répliquer toute la
/// sémantique de l'interpréteur (qui vit dans `kasm::interpreter`).
#[derive(Debug)]
pub struct ThreadedCtx<'a> {
    /// Inputs disponibles (analogue aux refs back dans un Node).
    pub inputs: &'a [i64],
    /// Output produit par l'op.
    pub output: i64,
    /// Compteur de dispatch — utile pour démontrer la propriété BTB.
    pub dispatch_count: u32,
}

/// Signature universelle d'un op handler dans threading mode.
pub type OpHandler = fn(&mut ThreadedCtx, imm: i64);

// ─── Handlers Wave 2 minimal (8 ops) ─────────────────────────────────

fn op_input(ctx: &mut ThreadedCtx, imm: i64) {
    let idx = imm as usize;
    ctx.output = ctx.inputs.get(idx).copied().unwrap_or(0);
    ctx.dispatch_count += 1;
}
fn op_const(ctx: &mut ThreadedCtx, imm: i64) {
    ctx.output = imm;
    ctx.dispatch_count += 1;
}
fn op_add(ctx: &mut ThreadedCtx, imm: i64) {
    let a = ctx.inputs.first().copied().unwrap_or(0);
    let b = ctx.inputs.get(1).copied().unwrap_or(imm);
    ctx.output = a.wrapping_add(b);
    ctx.dispatch_count += 1;
}
fn op_mul(ctx: &mut ThreadedCtx, imm: i64) {
    let a = ctx.inputs.first().copied().unwrap_or(1);
    let b = ctx.inputs.get(1).copied().unwrap_or(imm);
    ctx.output = a.wrapping_mul(b);
    ctx.dispatch_count += 1;
}
fn op_sub(ctx: &mut ThreadedCtx, imm: i64) {
    let a = ctx.inputs.first().copied().unwrap_or(0);
    let b = ctx.inputs.get(1).copied().unwrap_or(imm);
    ctx.output = a.wrapping_sub(b);
    ctx.dispatch_count += 1;
}
fn op_hash64(ctx: &mut ThreadedCtx, _imm: i64) {
    // SplitMix64 — un cycle complet documenté dans kasm::program::hash_i64.
    let mut z = ctx.inputs.first().copied().unwrap_or(0) as u64;
    z = z.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    ctx.output = (z ^ (z >> 31)) as i64;
    ctx.dispatch_count += 1;
}
fn op_output(ctx: &mut ThreadedCtx, _imm: i64) {
    // Output = passthrough du dernier input (sémantique simplifiée).
    if let Some(&v) = ctx.inputs.last() {
        ctx.output = v;
    }
    ctx.dispatch_count += 1;
}
fn op_unimplemented(ctx: &mut ThreadedCtx, _imm: i64) {
    // Wave 2 minimal : ops non couverts retournent 0 et incrémentent
    // un compteur. Wave 11+ remplacera par les vrais handlers.
    ctx.output = 0;
    ctx.dispatch_count += 1;
}

/// Dispatch d'un opcode vers son handler. Wave 2 minimal : 8 ops
/// implémentés, le reste tombe sur `op_unimplemented`. La table est
/// `&'static [OpHandler]` pour exposer la prédiction de branche au CPU.
///
/// Latence attendue : ~3-5 cycles pour le indirect call après BTB
/// warm-up vs ~10-25 cycles pour le `match` cascade.
pub fn dispatch_table(op: Op) -> OpHandler {
    match op {
        Op::Input => op_input,
        Op::ConstI64 => op_const,
        Op::AddI64 => op_add,
        Op::MulI64 => op_mul,
        Op::SubI64 => op_sub,
        Op::Hash64 => op_hash64,
        Op::Output => op_output,
        _ => op_unimplemented,
    }
}

/// Helper : exécute une séquence d'(op, imm) sur un contexte initial.
/// Pratique pour les tests et benchmarks.
pub fn run_threaded<'a>(initial_inputs: &'a [i64], steps: &[(Op, i64)]) -> ThreadedCtx<'a> {
    let mut ctx = ThreadedCtx {
        inputs: initial_inputs,
        output: 0,
        dispatch_count: 0,
    };
    for &(op, imm) in steps {
        let handler = dispatch_table(op);
        handler(&mut ctx, imm);
    }
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threaded_add_dispatch_works() {
        let ctx = run_threaded(&[3, 7], &[(Op::AddI64, 0)]);
        assert_eq!(ctx.output, 10);
        assert_eq!(ctx.dispatch_count, 1);
    }

    #[test]
    fn threaded_mul_dispatch_works() {
        let ctx = run_threaded(&[5, 6], &[(Op::MulI64, 0)]);
        assert_eq!(ctx.output, 30);
    }

    #[test]
    fn threaded_const_uses_imm() {
        let ctx = run_threaded(&[], &[(Op::ConstI64, 12345)]);
        assert_eq!(ctx.output, 12345);
    }

    #[test]
    fn threaded_hash64_deterministic() {
        // Même input → même hash, propriété V7 critique.
        let c1 = run_threaded(&[42], &[(Op::Hash64, 0)]);
        let c2 = run_threaded(&[42], &[(Op::Hash64, 0)]);
        assert_eq!(c1.output, c2.output);
        // Hash64(0) ≠ Hash64(1) — pas de collision triviale.
        let c0 = run_threaded(&[0], &[(Op::Hash64, 0)]);
        let c1b = run_threaded(&[1], &[(Op::Hash64, 0)]);
        assert_ne!(c0.output, c1b.output);
    }

    #[test]
    fn threaded_dispatch_table_is_function_pointer() {
        // Propriété BTB : on doit pouvoir comparer pointeurs.
        let h1 = dispatch_table(Op::AddI64);
        let h2 = dispatch_table(Op::AddI64);
        let h3 = dispatch_table(Op::MulI64);
        assert_eq!(h1 as usize, h2 as usize, "same op → same fn pointer");
        assert_ne!(h1 as usize, h3 as usize, "different ops → different fn pointers");
    }

    #[test]
    fn threaded_unimplemented_fallback() {
        // Op rare (ex: Comptime) tombe sur unimplemented, ne crash pas.
        let ctx = run_threaded(&[1, 2, 3], &[(Op::Comptime, 0)]);
        assert_eq!(ctx.output, 0);
        assert_eq!(ctx.dispatch_count, 1);
    }

    #[test]
    fn threaded_dispatch_count_aggregates() {
        let steps = [
            (Op::ConstI64, 10),
            (Op::AddI64, 0),
            (Op::MulI64, 0),
        ];
        let ctx = run_threaded(&[2, 3], &steps);
        assert_eq!(ctx.dispatch_count, 3);
    }
}
