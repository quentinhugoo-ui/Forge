//! Ω-7.0.2 — Pattern-match sur meta::Term.
//!
//! Permet d'exprimer des règles de réécriture sous forme de motifs
//! structurels sur l'AST CoC (`meta::Term`) plutôt que sur `kasm::Node`.
//!
//! Avantages :
//!  - Variables anonymes (`Hole`) qui matchent n'importe quel sous-terme.
//!  - Composition naturelle : un `Hole` peut apparaître plusieurs fois
//!    et doit alors lier la même valeur (consistance d'unification).
//!  - Pont vers les preuves Ω-4 : chaque pattern peut, à terme, être
//!    annoté d'une preuve d'équivalence sémantique.
//!
//! ## Périmètre du first mile
//!
//! `meta::Term` a **5 variantes** : `Var`, `Sort`, `Lam`, `Pi`, `App`.
//! `TermPattern` couvre les 5, plus la variante `Hole` (variable
//! d'unification). Les variantes liantes (`Lam`, `Pi`) sont matchées
//! structurellement (sans α-conversion supplémentaire) ce qui est
//! suffisant car `Term` utilise déjà des indices de de Bruijn — donc
//! l'α-équivalence est *structurelle*.
//!
//! ## Reportés
//!
//! * Ω-7.0.2.1 — pattern de second ordre (un `Hole` qui matche un
//!   contexte avec trous internes, pour exprimer des règles type
//!   `f x → g x` indépendamment de `x`).
//! * Ω-7.0.2.2 — conversion `Term → Program` (inverse de
//!   `embed_program`) pour transformer un match en réécriture KASM.

use std::collections::BTreeMap;

use crate::meta::Term;

/// Identifiant numérique d'un trou dans un pattern (variable d'unification).
pub type HoleId = u32;

/// Pattern structurel sur `Term`. Mirroir des variantes `Term` plus la
/// variante `Hole` qui matche n'importe quoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermPattern {
    /// Trou : matche n'importe quel sous-terme. Si le même `HoleId`
    /// apparaît plusieurs fois dans le pattern, le matcher exige que
    /// les sous-termes matchés soient égaux (consistance d'unification).
    Hole(HoleId),
    /// Matche `Term::Var(i)` exactement.
    Var(u32),
    /// Matche `Term::Sort(level)` exactement.
    Sort(u32),
    /// Matche `Term::Lam { ty, body }`.
    Lam { ty: Box<TermPattern>, body: Box<TermPattern> },
    /// Matche `Term::Pi { ty, body }`.
    Pi { ty: Box<TermPattern>, body: Box<TermPattern> },
    /// Matche `Term::App(f, x)`.
    App(Box<TermPattern>, Box<TermPattern>),
}

impl TermPattern {
    /// Constructeur ergonomique pour `Hole`.
    pub fn hole(id: HoleId) -> Self {
        TermPattern::Hole(id)
    }
    /// Constructeur ergonomique pour `Var`.
    pub fn var(i: u32) -> Self {
        TermPattern::Var(i)
    }
    /// Constructeur ergonomique pour `Sort`.
    pub fn sort(level: u32) -> Self {
        TermPattern::Sort(level)
    }
    /// Constructeur ergonomique pour `Lam`.
    pub fn lam(ty: TermPattern, body: TermPattern) -> Self {
        TermPattern::Lam { ty: Box::new(ty), body: Box::new(body) }
    }
    /// Constructeur ergonomique pour `Pi`.
    pub fn pi(ty: TermPattern, body: TermPattern) -> Self {
        TermPattern::Pi { ty: Box::new(ty), body: Box::new(body) }
    }
    /// Constructeur ergonomique pour `App`.
    pub fn app(f: TermPattern, x: TermPattern) -> Self {
        TermPattern::App(Box::new(f), Box::new(x))
    }
}

/// Bindings : map d'un `HoleId` vers le `Term` qu'il a unifié.
pub type Bindings = BTreeMap<HoleId, Term>;

/// Tente de matcher `pattern` contre `term`. Retourne `Some(bindings)`
/// si le pattern matche, `None` sinon. La cohérence d'unification est
/// vérifiée : si le même `HoleId` est utilisé plusieurs fois, les
/// bindings doivent concorder.
pub fn match_pattern(pattern: &TermPattern, term: &Term) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if match_recursive(pattern, term, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_recursive(
    pattern: &TermPattern,
    term: &Term,
    bindings: &mut Bindings,
) -> bool {
    match pattern {
        TermPattern::Hole(id) => {
            if let Some(existing) = bindings.get(id) {
                existing == term
            } else {
                bindings.insert(*id, term.clone());
                true
            }
        }
        TermPattern::Var(i) => matches!(term, Term::Var(j) if i == j),
        TermPattern::Sort(level) => matches!(term, Term::Sort(l) if level == l),
        TermPattern::Lam { ty: p_ty, body: p_body } => match term {
            Term::Lam { ty: t_ty, body: t_body } => {
                match_recursive(p_ty, t_ty, bindings)
                    && match_recursive(p_body, t_body, bindings)
            }
            _ => false,
        },
        TermPattern::Pi { ty: p_ty, body: p_body } => match term {
            Term::Pi { ty: t_ty, body: t_body } => {
                match_recursive(p_ty, t_ty, bindings)
                    && match_recursive(p_body, t_body, bindings)
            }
            _ => false,
        },
        TermPattern::App(p_f, p_x) => match term {
            Term::App(t_f, t_x) => {
                match_recursive(p_f, t_f, bindings)
                    && match_recursive(p_x, t_x, bindings)
            }
            _ => false,
        },
    }
}

// ---------------------------------------------------------------------------
// Ω-7.0.2.1 — Patterns de second ordre
// ---------------------------------------------------------------------------

/// Construit une variante "HoleApp" qui matche `Hole(id)` appliqué à des
/// arguments. Exemple : pour matcher `f(x, y)` quel que soit `f`, on utilise
/// `TermPattern::hole_app(0, vec![pat_x, pat_y])`.
///
/// Implémenté en réécrivant en chaîne d'`App` : `App(App(f, x), y)` etc.
impl TermPattern {
    pub fn hole_app(id: HoleId, args: Vec<TermPattern>) -> TermPattern {
        let mut acc = TermPattern::Hole(id);
        for arg in args {
            acc = TermPattern::App(Box::new(acc), Box::new(arg));
        }
        acc
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};
    use crate::meta::embed_program;

    fn affine_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            8,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),
                Node::const_i64(3),
                Node::add(2, 3),
                Node::output(4, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn hole_matches_any_term() {
        let p = affine_program();
        let term = embed_program(&p);
        let pattern = TermPattern::hole(0);
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "Hole doit matcher n'importe quel terme");
        let bindings = m.unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings.get(&0), Some(&term));
    }

    #[test]
    fn var_pattern_matches_var_exact() {
        let term = Term::var(3);
        assert!(match_pattern(&TermPattern::var(3), &term).is_some());
        assert!(match_pattern(&TermPattern::var(4), &term).is_none());
        // Var pattern ne matche pas une autre variante.
        assert!(match_pattern(&TermPattern::var(3), &Term::sort(3)).is_none());
    }

    #[test]
    fn sort_pattern_matches_sort_exact() {
        let term = Term::sort(0);
        assert!(match_pattern(&TermPattern::sort(0), &term).is_some());
        assert!(match_pattern(&TermPattern::sort(1), &term).is_none());
    }

    #[test]
    fn hole_with_same_id_must_bind_consistently() {
        // Pattern : App(Hole(0), Hole(0)).
        // Sur App(Var(1), Var(1)) → match (les deux holes lient Var(1)).
        // Sur App(Var(1), Var(2)) → fail (incohérence).
        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(0));

        let consistent = Term::app(Term::var(1), Term::var(1));
        let m = match_pattern(&pattern, &consistent);
        assert!(m.is_some(), "Hole(0) lie deux fois Var(1) — doit matcher");
        let b = m.unwrap();
        assert_eq!(b.len(), 1);
        assert_eq!(b.get(&0), Some(&Term::var(1)));

        let inconsistent = Term::app(Term::var(1), Term::var(2));
        assert!(
            match_pattern(&pattern, &inconsistent).is_none(),
            "Hole(0) lie Var(1) puis Var(2) — incohérence, doit fail"
        );
    }

    #[test]
    fn distinct_hole_ids_bind_independently() {
        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(1));
        let term = Term::app(Term::var(7), Term::sort(2));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.len(), 2);
        assert_eq!(b.get(&0), Some(&Term::var(7)));
        assert_eq!(b.get(&1), Some(&Term::sort(2)));
    }

    #[test]
    fn lam_pattern_matches_lam_recursively() {
        // λ(_: Type). Var(0) — pattern avec un trou pour le body.
        let pattern = TermPattern::lam(TermPattern::sort(0), TermPattern::hole(9));
        let term = Term::lam(Term::sort(0), Term::var(0));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.get(&9), Some(&Term::var(0)));

        // Un Pi ne doit pas matcher un Lam pattern.
        let pi_term = Term::pi(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &pi_term).is_none());
    }

    #[test]
    fn pi_pattern_distinct_from_lam() {
        let pattern = TermPattern::pi(TermPattern::hole(0), TermPattern::hole(1));
        let pi_term = Term::pi(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &pi_term).is_some());

        let lam_term = Term::lam(Term::sort(0), Term::var(0));
        assert!(match_pattern(&pattern, &lam_term).is_none());
    }

    #[test]
    fn deep_nested_pattern_matches() {
        // Pattern : App(App(Hole(0), Hole(1)), Hole(2)) — un App à 3 niveaux.
        let pattern = TermPattern::app(
            TermPattern::app(TermPattern::hole(0), TermPattern::hole(1)),
            TermPattern::hole(2),
        );
        let inner = Term::app(Term::sort(100), Term::var(0));
        let term = Term::app(inner.clone(), Term::sort(200));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some());
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&Term::sort(100)));
        assert_eq!(b.get(&1), Some(&Term::var(0)));
        assert_eq!(b.get(&2), Some(&Term::sort(200)));
    }

    #[test]
    fn pattern_match_on_embedded_program_finds_substructure() {
        // L'embedding d'un programme produit un Term en App-chain :
        //   App(App(...App(Sort(STRUCT_PROGRAM), tgt)...), node_n).
        // La racine est donc un App. On vérifie qu'un pattern App générique
        // matche, et qu'on peut extraire le LHS et le RHS via deux Holes.
        let p = affine_program();
        let term = embed_program(&p);

        let pattern = TermPattern::app(TermPattern::hole(0), TermPattern::hole(1));
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "embedded program a forme App(_,_) à la racine");
        let b = m.unwrap();
        assert_eq!(b.len(), 2);

        // Le RHS de la racine est l'embedding du dernier node (Output).
        // On vérifie au moins que le Hole(1) est bien lié à un sous-terme
        // de l'embedding.
        let rhs = b.get(&1).expect("Hole(1) doit être lié");
        // Sanity : le hash du Hole(1) doit différer de celui du term complet.
        assert_ne!(rhs.hash(), term.hash());
    }

    #[test]
    fn no_match_fails_cleanly() {
        // Pattern Sort(5) sur un App → None, pas de panic.
        let pattern = TermPattern::sort(5);
        let term = Term::app(Term::var(0), Term::var(1));
        assert!(match_pattern(&pattern, &term).is_none());
    }

    // -----------------------------------------------------------------
    // Ω-7.0.2.1 — patterns de second ordre (hole_app)
    // -----------------------------------------------------------------

    #[test]
    fn hole_app_matches_function_application() {
        // hole_app(0, [Hole(1)]) matche App(f, x) et lie 0 → f, 1 → x.
        let pattern = TermPattern::hole_app(0, vec![TermPattern::hole(1)]);
        let f = Term::var(42);
        let x = Term::sort(7);
        let term = Term::app(f.clone(), x.clone());
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "hole_app à un argument doit matcher App(f, x)");
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&f));
        assert_eq!(b.get(&1), Some(&x));
    }

    #[test]
    fn hole_app_two_args_matches_curried_app() {
        // hole_app(0, [Hole(1), Hole(2)]) matche App(App(f, x), y).
        let pattern = TermPattern::hole_app(
            0,
            vec![TermPattern::hole(1), TermPattern::hole(2)],
        );
        let f = Term::var(1);
        let x = Term::var(2);
        let y = Term::var(3);
        let term = Term::app(Term::app(f.clone(), x.clone()), y.clone());
        let m = match_pattern(&pattern, &term);
        assert!(m.is_some(), "hole_app à deux arguments doit matcher App-curried");
        let b = m.unwrap();
        assert_eq!(b.get(&0), Some(&f));
        assert_eq!(b.get(&1), Some(&x));
        assert_eq!(b.get(&2), Some(&y));
    }

    #[test]
    fn hole_app_fails_on_non_app() {
        // hole_app(0, [Hole(1)]) ne doit pas matcher Var(_) ou Sort(_).
        let pattern = TermPattern::hole_app(0, vec![TermPattern::hole(1)]);
        assert!(match_pattern(&pattern, &Term::var(5)).is_none());
        assert!(match_pattern(&pattern, &Term::sort(0)).is_none());
        // Cas zéro arg : hole_app(0, []) = Hole(0), matche tout.
        let zero_args = TermPattern::hole_app(0, vec![]);
        assert!(match_pattern(&zero_args, &Term::var(5)).is_some());
    }
}
