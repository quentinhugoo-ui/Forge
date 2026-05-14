//! Ω-7 — La Dissolution du Tokenizer.
//!
//! Cap actuel : **Ω-7.0** — agent symbolique non-LLM qui propose des
//! réécritures KASM **sans aucun token de langue naturelle dans son IO**.
//!
//! ## Doctrine
//!
//! L'agent perçoit un `kasm::Program` (bytes content-addressed) et
//! produit des candidats `Program` (bytes content-addressed). Sa
//! représentation interne utilise :
//!
//!  * Les opcodes KASM (`Op` enum, 28 variants).
//!  * Les indices de nœuds (`u16`).
//!  * Optionnellement, les termes du calcul des constructions Ω-4 (`Term`).
//!
//! Aucun `&str`, `String`, ou autre encodage texte humain n'entre dans
//! le raisonnement. Les seules `&str` sont les noms internes des règles
//! pour debug/logging — jamais consommés par la logique de transformation.
//!
//! ## Critère Ω-7
//!
//! > L'agent produit une réécriture KASM valide sans avoir vu un token
//! > de langue naturelle.
//!
//! Implémenté par : `SymbolicAgent::propose_rewrites(&Program) -> Vec<Program>`.
//! Chaque candidat retourné est :
//!  * Un `kasm::Program` valide (passe `verify`).
//!  * Sémantiquement équivalent à l'input sur les ops déterministes.
//!  * Strictement préférable (plus petit OU coût Landauer plus bas).
//!
//! ## Limites assumées
//!
//! - 3 règles algébriques minimales (add_zero, mul_one, const_fold).
//!   Étendre = Ω-7.0.x.
//! - Pas d'apprentissage, pas de neural net. C'est un agent *symbolique*,
//!   première brique avant l'agent **appris** Ω-7.1+.
//! - Pas de connexion au corpus Linux/Lean mathlib (Ω-7.x).

pub mod symbolic;
pub mod term_pattern;
pub mod term_to_program;
pub mod corpus;
pub mod extract;

pub use symbolic::{RankedCandidate, SymbolicAgent};
pub use term_pattern::{match_pattern, Bindings, HoleId, TermPattern};
