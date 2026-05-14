//! Ω-4 — L4 Méta + Preuves.
//!
//! Cap actuel : **Ω-4.0** — Calculus of Constructions minimal, content-addressed.
//!
//! Sous-ensemble Lean-like / CoC :
//!  * `Term` : Var (de Bruijn), Sort (universes), Lam, Pi, App.
//!  * Hash content-addressed sha256 domain-separated → identité unique
//!    par α-équivalence (les indices de de Bruijn rendent l'α-équivalence
//!    structurelle).
//!  * Beta reduction + normalisation avec fuel.
//!  * Type checker `infer(ctx, t) -> Type` et `check(ctx, t, expected)`.
//!  * Curry-Howard : un programme de type `Π (_: A). B` est une preuve de
//!    la proposition `A → B`.
//!
//! Reporté (sortis du scope V7 sous doctrine pure-Rust + std + sha2) :
//!  * Beta-réduction / type-checking dépendant / inductive / universe / tactic / bootstrap.
//!  * Réintroduits si un consommateur en prod les rappelle.

pub mod kasm_embed;
pub mod mlir_bridge;
pub mod term;

pub use kasm_embed::{embed_node, embed_program, meta_canonical_hash, meta_content_hash};
pub use term::Term;
