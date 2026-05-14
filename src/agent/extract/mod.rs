//! Ω-2 — L'Extraction Universelle.
//!
//! Cap actuel : **Ω-2.0** — Tracer-based extraction de fonctions Rust pures
//! et bornées vers des `kasm::Program` content-addressed.
//!
//! Φ.μ.7 : `macros.rs` et `term_extract.rs` repliés ici (3 fichiers → 1).
//! Le sous-dossier conserve `tracer.rs` + `tensor_tracer.rs` (les vrais
//! moteurs d'extraction).
//!
//! ## Le détournement (via negativa)
//!
//! La promesse Ω-2 originale visait Mojo. On a abandonné Mojo (toolchain
//! lourde, doctrine indépendance). À la place, **Rust** devient le langage
//! hôte d'où on extrait. Cohérent : tout SCAN est en Rust.
//!
//! Pas de proc-macro (qui demanderait un crate séparé + frontend lourd).
//! Pas de parser de strings (pas de `syn` en deps). À la place, on utilise
//! le **type system Rust pour CONTRAINDRE l'extraction** :
//!
//! 1. On expose un type `Tracer` qui implémente `std::ops::Add/Sub/Mul/...`.
//! 2. Toute fonction `Fn(Vec<Tracer>) -> Tracer` est automatiquement
//!    extractable.
//! 3. Toute fonction qui essaie d'appeler une op non-supportée (I/O,
//!    allocation, boucle dynamique, etc.) ne **compile pas** — Rust refuse
//!    avant même qu'on tente l'extraction.
//!
//! C'est l'inverse du pattern usuel "détecter la pureté à l'inférence" :
//! on **rend la pureté inférable au compilateur Rust** en restreignant le
//! type. La sécurité émerge du type-system, pas d'une analyse statique.

mod tracer;

pub use tracer::{extract, BoolTracer, ExtractError, Tracer};

pub mod tensor_tracer;

// ----- Ω-2.0.5 — Macros ergonomiques -----
//
// Macros `macro_rules!` (zéro proc-macro, zéro dépendance) qui enveloppent
// les patterns courants pour réduire le boilerplate sans introduire de
// sémantique nouvelle.

/// Sucre syntaxique pour `crate::extract::extract(n, |inputs| { ... })`.
///
/// # Exemple
/// ```ignore
/// let prog = tracer_extract!(2, |inputs| inputs[0] + inputs[1]).unwrap();
/// ```
#[macro_export]
macro_rules! tracer_extract {
    ($n:expr, |$inputs:ident| $body:expr) => {
        $crate::extract::extract($n, |$inputs: ::std::vec::Vec<$crate::extract::Tracer>| $body)
    };
}

/// Construit un `Tracer` constant pour n'importe quelle valeur i64.
/// Délègue à `Tracer::const_i64_wide` (1 nœud pour i16, jusqu'à ~15 nœuds
/// hors i16).
#[macro_export]
macro_rules! tracer_const {
    ($v:expr) => {
        $crate::extract::Tracer::const_i64_wide($v as i64)
    };
}

// ----- Ω-2.0.8 — Extraction directe vers `meta::Term` -----

/// Extrait une closure Rust en `meta::Term` en un seul pas.
/// Équivaut à `embed_program(extract(n_inputs, f)?)`.
pub fn extract_to_term<F>(n_inputs: u8, f: F) -> Result<crate::meta::Term, ExtractError>
where
    F: FnOnce(Vec<Tracer>) -> Tracer,
{
    let prog = extract(n_inputs, f)?;
    Ok(crate::meta::embed_program(&prog))
}

#[cfg(test)]
mod fold_tests {
    use super::*;
    use crate::kasm::{execute, Program};

    fn exec_one(p: &Program, args: &[i64]) -> Vec<u8> {
        let bytes: Vec<u8> = args.iter().flat_map(|v| v.to_le_bytes()).collect();
        execute(p, &bytes).expect("kasm execute")
    }

    fn read_one_i64(out: &[u8]) -> i64 {
        i64::from_le_bytes(out[..8].try_into().unwrap())
    }

    #[test]
    fn macro_extract_simple() {
        let prog = tracer_extract!(2, |inputs| inputs[0] + inputs[1]).unwrap();
        for (a, b) in [(0i64, 0), (1, 2), (-3, 7), (1_000, 2_000)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    #[test]
    fn macro_const_wide_works_with_large_value() {
        let target: i64 = 0x1234_5678_9ABC_DEF0_u64 as i64;
        let prog = extract(1, |inputs| inputs[0] + tracer_const!(target)).unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), target);
    }

    #[test]
    fn macro_extract_matches_direct_extract_hash() {
        let p_macro = tracer_extract!(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let p_direct = extract(1, |inputs: Vec<Tracer>| inputs[0] * 7 + 3).unwrap();
        assert_eq!(
            p_macro.canonical_hash_hex().unwrap(),
            p_direct.canonical_hash_hex().unwrap(),
        );
    }

    #[test]
    fn extract_to_term_identity_function() {
        let term = extract_to_term(1, |inputs| inputs[0]).expect("extract_to_term");
        assert_ne!(term.hash(), [0u8; 32]);
    }

    #[test]
    fn extract_to_term_arithmetic() {
        let term = extract_to_term(2, |inputs| inputs[0] * inputs[1] + 7).expect("extract_to_term");
        assert_ne!(term.hash(), [0u8; 32]);
    }

    #[test]
    fn extract_to_term_distinct_closures() {
        let t_add = extract_to_term(2, |inputs| inputs[0] + inputs[1]).unwrap();
        let t_mul = extract_to_term(2, |inputs| inputs[0] * inputs[1]).unwrap();
        assert_ne!(t_add.hash(), t_mul.hash());
    }

    #[test]
    fn extract_to_term_deterministic() {
        let t1 = extract_to_term(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let t2 = extract_to_term(1, |inputs| inputs[0] * 7 + 3).unwrap();
        assert_eq!(t1.hash(), t2.hash());
    }
}
