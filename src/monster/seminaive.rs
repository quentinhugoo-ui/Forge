//! Π.8 (Wave 1, 2026-05-02) — Datalog seminaive incremental fixpoint.
//!
//! **Origine** : Datalog (Souffle, DDLog, Differential Dataflow). Idée
//! centrale : un fixpoint déductif où chaque itération ne dérive QUE
//! les nouveaux faits via les changements (`Δ`) du tour précédent —
//! pas l'EDB stable. Donné R(x,y) :- A(x,z), B(z,y), la version naïve
//! refait `A ⋈ B` à chaque pas ; la seminaive ne calcule que
//! `ΔA ⋈ B ∪ A ⋈ ΔB`. Gain : O(K·|Δ|) au lieu de O(K·|R|).
//!
//! ## Architecture minimal viable Wave 1
//!
//! ```text
//!   EDB (Extensional DB) ─┐
//!   IDB rules ────────────┴→ initial Δ = EDB
//!                              │
//!                              ↓ apply rules to Δ → new derived Δ'
//!                              ↓ Δ' \ IDB_old = truly new
//!                              ↓ until Δ = ∅ → fixpoint
//! ```
//!
//! Ici on garde la version Datalog la plus simple :
//! - faits = `Fact { relation: u32, args: Vec<i64> }`
//! - règles = `Rule { head, body: Vec<Atom> }` avec atoms unifiables
//! - moteur = `SeminaiveEngine::run()` retourne IDB stable
//!
//! ## Pourquoi pour Forge ?
//!
//! Le synthétiseur du lab dérive incrementalement des faits :
//! "programme P sur target T donne loss L", "atome A apparaît dans
//! famille F", "ultra-glyph U généralise programme P". Quand un
//! nouveau programme entre, on veut **propager** ses conséquences sans
//! recomputer toute la dérivation. C'est exactement ce que la
//! seminaive donne — les nouveaux faits sont une `Δ` qui se propage
//! dans les règles existantes.
//!
//! ## Limitations Wave 1 minimal
//!
//! - Pas de négation (Datalog stratifié non-implémenté Wave 1).
//! - Pas d'agrégation (count/sum/min — Wave 1c ou plus tard).
//! - Pas de récursion mutuelle complexe — on supporte la récursion
//!   simple (R :- R, ...) mais pas les cycles cross-relations subtils.
//! - Pattern matching basique : `Term::Const(i64)` ou `Term::Var(name)`,
//!   pas de prédicats arithmétiques inline.

use std::collections::{HashMap, HashSet};

/// Identifiant numérique d'une relation. Les relations sont indexées
/// par `u32` pour rester compactes ; le mapping nom→id est externe.
pub type RelationId = u32;

/// Un fait concret stocké dans EDB ou dérivé en IDB.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fact {
    pub relation: RelationId,
    pub args: Vec<i64>,
}

impl Fact {
    pub fn new(relation: RelationId, args: Vec<i64>) -> Self {
        Self { relation, args }
    }
}

/// Un terme dans une règle : soit une constante (binding fermé) soit
/// une variable nommée (binding ouvert qui doit s'unifier).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Const non utilisé en validate-features mais exporté pour API
pub enum Term {
    /// Constante littérale i64.
    Const(i64),
    /// Variable nommée — doit s'unifier avec les autres occurrences.
    Var(String),
}

/// Un atome dans une règle : `relation(t1, t2, ..., tn)`.
#[derive(Debug, Clone)]
pub struct Atom {
    pub relation: RelationId,
    pub terms: Vec<Term>,
}

impl Atom {
    pub fn new(relation: RelationId, terms: Vec<Term>) -> Self {
        Self { relation, terms }
    }
}

/// Une règle Datalog : `head :- body[0], body[1], ...`.
#[derive(Debug, Clone)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Atom>,
}

impl Rule {
    pub fn new(head: Atom, body: Vec<Atom>) -> Self {
        Self { head, body }
    }
}

/// Substitution variable → valeur i64. Construite incrémentalement
/// pendant l'unification d'un body atom contre un fact concret.
type Subst = HashMap<String, i64>;

fn unify_term(term: &Term, value: i64, subst: &mut Subst) -> bool {
    match term {
        Term::Const(c) => *c == value,
        Term::Var(name) => match subst.get(name) {
            Some(existing) => *existing == value,
            None => {
                subst.insert(name.clone(), value);
                true
            }
        },
    }
}

fn unify_atom(atom: &Atom, fact: &Fact, subst: &mut Subst) -> bool {
    if atom.relation != fact.relation || atom.terms.len() != fact.args.len() {
        return false;
    }
    for (term, value) in atom.terms.iter().zip(fact.args.iter()) {
        if !unify_term(term, *value, subst) {
            return false;
        }
    }
    true
}

fn instantiate(atom: &Atom, subst: &Subst) -> Option<Fact> {
    let mut args = Vec::with_capacity(atom.terms.len());
    for term in &atom.terms {
        match term {
            Term::Const(c) => args.push(*c),
            Term::Var(name) => match subst.get(name) {
                Some(v) => args.push(*v),
                None => return None, // Variable libre dans la tête → invalide.
            },
        }
    }
    Some(Fact::new(atom.relation, args))
}

/// Comptage d'opérations pour observabilité.
#[derive(Debug, Default, Clone)]
pub struct SeminaiveStats {
    /// Nombre d'itérations du fixpoint (jusqu'à `Δ = ∅`).
    pub iterations: u32,
    /// Faits totaux dérivés (incluant duplicates avant dedup).
    pub derivations_attempted: u64,
    /// Faits réellement nouveaux ajoutés à l'IDB.
    pub new_facts_added: u64,
    /// Comparaisons d'atomes effectuées (proxy du coût total).
    pub atom_unifications: u64,
}

/// Moteur seminaive : prend (EDB, rules) et retourne IDB au fixpoint.
///
/// L'IDB final contient EDB ∪ tous les faits dérivables. Pour un
/// programme correct (rules safe, pas de négation), le fixpoint
/// termine en au plus |Hilbert universe|^arité tours — en pratique
/// bien moins.
pub struct SeminaiveEngine {
    rules: Vec<Rule>,
}

impl SeminaiveEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Exécute la seminaive jusqu'au fixpoint et retourne (IDB, stats).
    /// L'EDB est inclus dans l'IDB retourné.
    pub fn run(&self, edb: Vec<Fact>) -> (Vec<Fact>, SeminaiveStats) {
        let mut stats = SeminaiveStats::default();
        let mut idb: HashSet<Fact> = edb.into_iter().collect();
        // Δ_0 = EDB initial. À chaque tour, Δ_{k+1} = (nouveaux faits
        // dérivés depuis Δ_k) \ IDB.
        let mut delta: HashSet<Fact> = idb.clone();

        // Cap anti-runaway : si on dépasse 256 itérations, on s'arrête
        // (rules non-safe ou explosion combinatoire).
        for iter in 0..256 {
            stats.iterations = iter + 1;
            if delta.is_empty() {
                stats.iterations = iter; // Le tour vide ne compte pas.
                break;
            }
            let mut next_delta: HashSet<Fact> = HashSet::new();

            // Pour chaque rule, on essaie chaque body_atom comme
            // "atom-pivot" qui matche un fait du delta. Les autres
            // body_atoms matchent l'IDB stable. Ainsi tout match a au
            // moins UN fait nouveau → optimisation seminaive standard.
            for (rule_idx, rule) in self.rules.iter().enumerate() {
                for pivot_idx in 0..rule.body.len() {
                    self.derive_with_pivot(
                        rule_idx, pivot_idx, rule, &delta, &idb,
                        &mut next_delta, &mut stats,
                    );
                }
            }

            // Δ_{k+1} = next_delta \ IDB. On retire les faits déjà
            // connus pour que la prochaine itération ne reprenne pas
            // les mêmes dérivations.
            let mut truly_new = HashSet::new();
            for fact in next_delta {
                if !idb.contains(&fact) {
                    stats.new_facts_added += 1;
                    idb.insert(fact.clone());
                    truly_new.insert(fact);
                }
            }
            delta = truly_new;
        }

        let mut out: Vec<Fact> = idb.into_iter().collect();
        // Tri déterministe pour reproductibilité cross-machine.
        out.sort_by(|a, b| {
            a.relation.cmp(&b.relation).then_with(|| a.args.cmp(&b.args))
        });
        (out, stats)
    }

    fn derive_with_pivot(
        &self,
        _rule_idx: usize,
        pivot_idx: usize,
        rule: &Rule,
        delta: &HashSet<Fact>,
        idb: &HashSet<Fact>,
        next_delta: &mut HashSet<Fact>,
        stats: &mut SeminaiveStats,
    ) {
        let pivot_atom = &rule.body[pivot_idx];
        // Pour chaque fact dans delta de la même relation que pivot_atom :
        let candidates: Vec<&Fact> = delta
            .iter()
            .filter(|f| f.relation == pivot_atom.relation)
            .collect();
        for pivot_fact in candidates {
            let mut base_subst: Subst = HashMap::new();
            stats.atom_unifications += 1;
            if !unify_atom(pivot_atom, pivot_fact, &mut base_subst) {
                continue;
            }
            // Maintenant on join avec les autres body atoms (sur IDB).
            self.join_remaining(rule, pivot_idx, &base_subst, idb, next_delta, stats);
        }
    }

    fn join_remaining(
        &self,
        rule: &Rule,
        pivot_idx: usize,
        base_subst: &Subst,
        idb: &HashSet<Fact>,
        next_delta: &mut HashSet<Fact>,
        stats: &mut SeminaiveStats,
    ) {
        // Liste des indices restants (≠ pivot_idx) à joindre.
        let remaining: Vec<usize> = (0..rule.body.len())
            .filter(|&i| i != pivot_idx)
            .collect();
        self.join_recursive(
            rule, &remaining, 0, base_subst.clone(),
            idb, next_delta, stats,
        );
    }

    fn join_recursive(
        &self,
        rule: &Rule,
        remaining: &[usize],
        depth: usize,
        subst: Subst,
        idb: &HashSet<Fact>,
        next_delta: &mut HashSet<Fact>,
        stats: &mut SeminaiveStats,
    ) {
        if depth == remaining.len() {
            // Tous les body atoms unifiés → on instancie la tête.
            stats.derivations_attempted += 1;
            if let Some(head_fact) = instantiate(&rule.head, &subst) {
                next_delta.insert(head_fact);
            }
            return;
        }
        let atom_idx = remaining[depth];
        let atom = &rule.body[atom_idx];
        // Pour chaque fait dans IDB de la même relation, tenter d'unifier.
        for fact in idb.iter().filter(|f| f.relation == atom.relation) {
            stats.atom_unifications += 1;
            let mut new_subst = subst.clone();
            if unify_atom(atom, fact, &mut new_subst) {
                self.join_recursive(
                    rule, remaining, depth + 1, new_subst,
                    idb, next_delta, stats,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper : crée un atome `R(args...)` avec args = constantes/vars.
    fn atom(relation: RelationId, terms: Vec<Term>) -> Atom {
        Atom::new(relation, terms)
    }

    #[test]
    fn seminaive_transitive_closure() {
        // edge(1,2), edge(2,3), edge(3,4)
        // path(x,y) :- edge(x,y)
        // path(x,z) :- edge(x,y), path(y,z)
        // Expected paths : (1,2), (2,3), (3,4), (1,3), (2,4), (1,4)
        const EDGE: RelationId = 1;
        const PATH: RelationId = 2;
        let edb = vec![
            Fact::new(EDGE, vec![1, 2]),
            Fact::new(EDGE, vec![2, 3]),
            Fact::new(EDGE, vec![3, 4]),
        ];
        let rules = vec![
            // path(X,Y) :- edge(X,Y)
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                vec![atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())])],
            ),
            // path(X,Z) :- edge(X,Y), path(Y,Z)
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Z".into())]),
                vec![
                    atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                    atom(PATH, vec![Term::Var("Y".into()), Term::Var("Z".into())]),
                ],
            ),
        ];
        let engine = SeminaiveEngine::new(rules);
        let (idb, stats) = engine.run(edb);

        let paths: Vec<&Fact> = idb.iter().filter(|f| f.relation == PATH).collect();
        let path_pairs: HashSet<(i64, i64)> =
            paths.iter().map(|f| (f.args[0], f.args[1])).collect();

        let expected: HashSet<(i64, i64)> = vec![
            (1, 2), (2, 3), (3, 4), (1, 3), (2, 4), (1, 4),
        ].into_iter().collect();

        assert_eq!(path_pairs, expected, "transitive closure must derive all 6 paths");
        assert!(stats.iterations < 10, "fixpoint should reach quickly");
        assert!(stats.new_facts_added >= 6, "must add at least 6 paths");
    }

    #[test]
    fn seminaive_terminates_on_cycle() {
        // Avec un cycle edge(1,2), edge(2,1), la transitive closure
        // produit (1,1), (1,2), (2,1), (2,2) puis se stabilise.
        const EDGE: RelationId = 1;
        const PATH: RelationId = 2;
        let edb = vec![
            Fact::new(EDGE, vec![1, 2]),
            Fact::new(EDGE, vec![2, 1]),
        ];
        let rules = vec![
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                vec![atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())])],
            ),
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Z".into())]),
                vec![
                    atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                    atom(PATH, vec![Term::Var("Y".into()), Term::Var("Z".into())]),
                ],
            ),
        ];
        let engine = SeminaiveEngine::new(rules);
        let (idb, stats) = engine.run(edb);

        let path_pairs: HashSet<(i64, i64)> = idb
            .iter()
            .filter(|f| f.relation == PATH)
            .map(|f| (f.args[0], f.args[1]))
            .collect();

        let expected: HashSet<(i64, i64)> = vec![
            (1, 2), (2, 1), (1, 1), (2, 2),
        ].into_iter().collect();

        assert_eq!(path_pairs, expected);
        assert!(stats.iterations < 256, "must not hit anti-runaway cap");
    }

    #[test]
    fn seminaive_const_filter() {
        // parent(alice, bob), parent(bob, carol)
        // ancestor_of_alice(Y) :- parent(alice, Y)
        // → un seul fait : ancestor_of_alice(bob)
        const PARENT: RelationId = 1;
        const ANCESTOR: RelationId = 2;
        const ALICE: i64 = 100;
        const BOB: i64 = 200;
        const CAROL: i64 = 300;
        let edb = vec![
            Fact::new(PARENT, vec![ALICE, BOB]),
            Fact::new(PARENT, vec![BOB, CAROL]),
        ];
        let rules = vec![
            Rule::new(
                atom(ANCESTOR, vec![Term::Var("Y".into())]),
                vec![atom(PARENT, vec![Term::Const(ALICE), Term::Var("Y".into())])],
            ),
        ];
        let engine = SeminaiveEngine::new(rules);
        let (idb, _) = engine.run(edb);
        let ancestors: Vec<i64> = idb
            .iter()
            .filter(|f| f.relation == ANCESTOR)
            .map(|f| f.args[0])
            .collect();
        assert_eq!(ancestors, vec![BOB], "only bob is direct child of alice");
    }

    #[test]
    fn seminaive_empty_edb_yields_empty_idb() {
        let rules = vec![
            Rule::new(
                atom(2, vec![Term::Var("X".into())]),
                vec![atom(1, vec![Term::Var("X".into())])],
            ),
        ];
        let engine = SeminaiveEngine::new(rules);
        let (idb, stats) = engine.run(vec![]);
        assert!(idb.is_empty());
        assert_eq!(stats.new_facts_added, 0);
    }

    #[test]
    fn seminaive_deterministic_output_order() {
        // Le sort déterministe en sortie permet le hash content-addressed
        // sur l'IDB, propriété V7 critique.
        const E: RelationId = 1;
        let edb = vec![
            Fact::new(E, vec![3]),
            Fact::new(E, vec![1]),
            Fact::new(E, vec![2]),
        ];
        let engine = SeminaiveEngine::new(vec![]);
        let (idb1, _) = engine.run(edb.clone());
        let (idb2, _) = engine.run(edb);
        assert_eq!(idb1, idb2, "même EDB → même IDB ordre");
        // Doit être trié par args.
        assert_eq!(idb1[0].args, vec![1]);
        assert_eq!(idb1[1].args, vec![2]);
        assert_eq!(idb1[2].args, vec![3]);
    }

    #[test]
    fn seminaive_stats_track_iterations() {
        // path(x,y) :- edge(x,y) ; path(x,z) :- edge(x,y), path(y,z)
        // Sur une chaîne de longueur 4, il faut au plus 4 itérations
        // pour atteindre le fixpoint.
        const EDGE: RelationId = 1;
        const PATH: RelationId = 2;
        let edb = vec![
            Fact::new(EDGE, vec![1, 2]),
            Fact::new(EDGE, vec![2, 3]),
            Fact::new(EDGE, vec![3, 4]),
            Fact::new(EDGE, vec![4, 5]),
        ];
        let rules = vec![
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                vec![atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())])],
            ),
            Rule::new(
                atom(PATH, vec![Term::Var("X".into()), Term::Var("Z".into())]),
                vec![
                    atom(EDGE, vec![Term::Var("X".into()), Term::Var("Y".into())]),
                    atom(PATH, vec![Term::Var("Y".into()), Term::Var("Z".into())]),
                ],
            ),
        ];
        let engine = SeminaiveEngine::new(rules);
        let (_, stats) = engine.run(edb);
        // Doit avoir terminé proprement, non capé.
        assert!(stats.iterations >= 1 && stats.iterations < 10);
        assert!(stats.atom_unifications > 0);
    }
}
