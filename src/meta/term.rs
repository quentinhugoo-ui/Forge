//! Term : AST du calcul des constructions minimal, content-addressed.

use sha2::{Digest, Sha256};

const TERM_HASH_DOMAIN: &[u8] = b"SCAN-OMEGA-META-TERM-V1";

const TAG_VAR: u8 = 1;
const TAG_SORT: u8 = 2;
const TAG_LAM: u8 = 3;
const TAG_PI: u8 = 4;
const TAG_APP: u8 = 5;

/// Term du Calculus of Constructions minimal.
///
/// **Représentation** : indices de de Bruijn — `Var(0)` désigne le binder
/// le plus interne, `Var(1)` le binder qui le contient, etc. Cette
/// représentation rend l'α-équivalence **structurelle** : deux termes
/// α-équivalents ont la même structure et donc le même hash.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Term {
    /// Variable de de Bruijn. `Var(0)` = binder le plus interne.
    Var(u32),
    /// Universe : `Sort(0)` = `Type`, `Sort(1)` = `Type1`, etc.
    Sort(u32),
    /// Lambda abstraction : `λ (_: ty). body`. Le body référence
    /// le paramètre via `Var(0)`.
    Lam { ty: Box<Term>, body: Box<Term> },
    /// Pi (type fonction dépendant) : `Π (_: ty). body`. Si `body`
    /// n'utilise pas `Var(0)`, c'est un type `ty → body` ordinaire.
    Pi { ty: Box<Term>, body: Box<Term> },
    /// Application : `f x`.
    App(Box<Term>, Box<Term>),
}

impl Term {
    pub fn var(i: u32) -> Self {
        Self::Var(i)
    }
    pub fn sort(level: u32) -> Self {
        Self::Sort(level)
    }
    pub fn ty() -> Self {
        Self::Sort(0)
    }
    pub fn lam(ty: Term, body: Term) -> Self {
        Self::Lam { ty: Box::new(ty), body: Box::new(body) }
    }
    pub fn pi(ty: Term, body: Term) -> Self {
        Self::Pi { ty: Box::new(ty), body: Box::new(body) }
    }
    pub fn app(f: Term, x: Term) -> Self {
        Self::App(Box::new(f), Box::new(x))
    }

    /// Type fonction non-dépendant : `a → b`. Le body est lifté de 1 pour
    /// que ses variables libres ignorent le binder Pi.
    pub fn arrow(a: Term, b: Term) -> Self {
        let b_lifted = b.lift(0, 1);
        Self::pi(a, b_lifted)
    }

    /// Lift toutes les variables libres ≥ `cutoff` par `delta`. Utilisé
    /// pour faire passer un terme sous un nouveau binder.
    pub fn lift(&self, cutoff: u32, delta: u32) -> Term {
        match self {
            Term::Var(i) => {
                if *i >= cutoff {
                    Term::Var(i + delta)
                } else {
                    Term::Var(*i)
                }
            }
            Term::Sort(n) => Term::Sort(*n),
            Term::Lam { ty, body } => Term::Lam {
                ty: Box::new(ty.lift(cutoff, delta)),
                body: Box::new(body.lift(cutoff + 1, delta)),
            },
            Term::Pi { ty, body } => Term::Pi {
                ty: Box::new(ty.lift(cutoff, delta)),
                body: Box::new(body.lift(cutoff + 1, delta)),
            },
            Term::App(f, x) => Term::App(
                Box::new(f.lift(cutoff, delta)),
                Box::new(x.lift(cutoff, delta)),
            ),
        }
    }

    /// Substitue `Var(target)` par `replacement`. Décrémente les variables
    /// supérieures (un binder a été éliminé). Lifte automatiquement le
    /// `replacement` selon la profondeur courante.
    pub fn subst(&self, target: u32, replacement: &Term) -> Term {
        match self {
            Term::Var(i) => {
                if *i == target {
                    replacement.lift(0, target)
                } else if *i > target {
                    Term::Var(i - 1)
                } else {
                    Term::Var(*i)
                }
            }
            Term::Sort(n) => Term::Sort(*n),
            Term::Lam { ty, body } => Term::Lam {
                ty: Box::new(ty.subst(target, replacement)),
                body: Box::new(body.subst(target + 1, replacement)),
            },
            Term::Pi { ty, body } => Term::Pi {
                ty: Box::new(ty.subst(target, replacement)),
                body: Box::new(body.subst(target + 1, replacement)),
            },
            Term::App(f, x) => Term::App(
                Box::new(f.subst(target, replacement)),
                Box::new(x.subst(target, replacement)),
            ),
        }
    }

    /// Hash content-addressed (sha256 domain-separated).
    ///
    /// Propriété : deux termes α-équivalents (i.e. structurellement
    /// identiques en de Bruijn) ont le même hash.
    pub fn hash(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(TERM_HASH_DOMAIN);
        self.write_canonical(&mut h);
        let result = h.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    fn write_canonical(&self, h: &mut Sha256) {
        match self {
            Term::Var(i) => {
                h.update([TAG_VAR]);
                h.update(i.to_le_bytes());
            }
            Term::Sort(n) => {
                h.update([TAG_SORT]);
                h.update(n.to_le_bytes());
            }
            Term::Lam { ty, body } => {
                h.update([TAG_LAM]);
                ty.write_canonical(h);
                body.write_canonical(h);
            }
            Term::Pi { ty, body } => {
                h.update([TAG_PI]);
                ty.write_canonical(h);
                body.write_canonical(h);
            }
            Term::App(f, x) => {
                h.update([TAG_APP]);
                f.write_canonical(h);
                x.write_canonical(h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_same_term_same() {
        let a = Term::lam(Term::ty(), Term::var(0));
        let b = Term::lam(Term::ty(), Term::var(0));
        assert_eq!(a.hash(), b.hash());
    }

    #[test]
    fn hash_distinguishes_constructors() {
        let v = Term::var(0).hash();
        let s = Term::sort(0).hash();
        let l = Term::lam(Term::ty(), Term::var(0)).hash();
        let p = Term::pi(Term::ty(), Term::var(0)).hash();
        let app = Term::app(Term::var(0), Term::var(0)).hash();
        let unique: std::collections::BTreeSet<_> = [v, s, l, p, app].into_iter().collect();
        assert_eq!(unique.len(), 5, "tags doivent distinguer les constructeurs");
    }

    #[test]
    fn hash_distinguishes_indices() {
        assert_ne!(Term::var(0).hash(), Term::var(1).hash());
        assert_ne!(Term::sort(0).hash(), Term::sort(1).hash());
    }

    #[test]
    fn hash_uses_domain_separation() {
        // Sha256 brut sur les bytes des tags ne doit PAS coïncider.
        let mut raw = Sha256::new();
        raw.update([TAG_VAR]);
        raw.update(0u32.to_le_bytes());
        let raw_result: [u8; 32] = raw.finalize().into();
        assert_ne!(Term::var(0).hash(), raw_result);
    }

    #[test]
    fn lift_var_above_cutoff_increases() {
        let t = Term::var(2);
        assert_eq!(t.lift(1, 3), Term::var(5));
    }

    #[test]
    fn lift_var_below_cutoff_unchanged() {
        let t = Term::var(0);
        assert_eq!(t.lift(1, 3), Term::var(0));
    }

    #[test]
    fn lift_traverses_binders() {
        // λ. Var(1) — le 1 référence un binder externe. Sous le λ on est à
        // profondeur 1, donc cutoff=0+1=1 dans le body. Var(1) ≥ 1 → lift.
        let t = Term::lam(Term::ty(), Term::var(1));
        let lifted = t.lift(0, 1);
        assert_eq!(lifted, Term::lam(Term::ty(), Term::var(2)));
    }

    #[test]
    fn subst_replaces_target_var() {
        // Var(0)[0 := y] = y (lifté de 0)
        let t = Term::var(0);
        let r = Term::sort(7);
        assert_eq!(t.subst(0, &r), Term::sort(7));
    }

    #[test]
    fn subst_decrements_higher_vars() {
        // Var(2)[0 := y] = Var(1) (le binder à 0 est consommé, les supérieurs descendent)
        let t = Term::var(2);
        let r = Term::sort(0);
        assert_eq!(t.subst(0, &r), Term::var(1));
    }

    #[test]
    fn subst_lifts_replacement_under_binders() {
        // (λ. Var(1))[0 := Var(0)] :
        //   sous le λ, target=1. On hit Var(1), on remplace par Var(0).lift(0, 1) = Var(1).
        let body = Term::lam(Term::ty(), Term::var(1));
        let r = Term::var(0);
        let result = body.subst(0, &r);
        assert_eq!(result, Term::lam(Term::ty(), Term::var(1)));
    }
}
