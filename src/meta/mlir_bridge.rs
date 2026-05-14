//! Ω-4.6 — Bridge `meta::Term` ↔ MLIR text dialect `meta`.
//!
//! Le pendant strict, pour le L4 méta (Calculus of Constructions), du codec
//! `kasm` ⊂ MLIR (cf. [`crate::kasm::mlir`]). Définit un format texte
//! déterministe pour le dialecte custom `meta.*` et fournit :
//!
//!   * [`emit_meta_mlir`] — sérialise un `Term`.
//!   * [`parse_meta_mlir`] — désérialise vers un `Term`.
//!
//! La propriété centrale (testée) :
//! ```text
//!     parse_meta_mlir(emit_meta_mlir(t)) == t   (byte-for-byte sur l'AST)
//!     hash invariant                            (Term::hash() préservé)
//! ```
//!
//! Format (pseudo-grammar) :
//! ```text
//! module   ::= "meta.term" "{" "\n" body "\n" "}" "\n"
//! body     ::= line ("\n" line)* "\n" root
//! line     ::= "  %" IDX " = meta." OP_TAIL
//! OP_TAIL  ::= "var {index = " N "}"
//!            | "sort {level = " N "}"
//!            | "lam %" T_IDX ", %" B_IDX
//!            | "pi %" T_IDX ", %" B_IDX
//!            | "app %" F_IDX ", %" X_IDX
//! root     ::= "  meta.root %" IDX
//! ```
//!
//! L'ordre des lignes est un **tri topologique post-ordre** déterministe :
//! les sous-termes apparaissent toujours avant leurs parents. Cela donne
//! au format la même propriété que la forme SSA de MLIR, sans ambiguïté
//! ni espace flottant.
//!
//! ## Pourquoi un dialecte distinct du `kasm.dialect` (via negativa)
//!
//! `kasm` opère sur i64/i1, opcodes pré-définis, DAG borné. `meta` opère
//! sur termes du Calculus of Constructions : universes, binders, types
//! dépendants. Réutiliser `kasm` reviendrait à confondre les domaines
//! sémantiques — interdit par doctrine.
//!
//! ## Pourquoi pas de raccourci syntaxique
//!
//! L'AST `Term` est minimal (5 constructeurs). Sa forme MLIR doit lui
//! coller 1:1. Pas de Pi/Lam shorthand "→", pas d'inférence d'index :
//! tout est explicite. C'est ce qui garantit le roundtrip byte-exact.

use std::collections::HashMap;
use std::fmt::Write as _;

use super::term::Term;

const SSA_PREFIX: &str = "%";

#[derive(Debug, PartialEq, Eq)]
pub enum MlirBridgeError {
    Syntax(String),
    UnknownOp(String),
    BadIndex(String),
    BadInteger(String),
    BadHeader,
    BadFooter,
    MissingRoot,
    DuplicateRoot,
    UnresolvedRef(usize),
    EmptyModule,
}

impl std::fmt::Display for MlirBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MlirBridgeError::Syntax(s) => write!(f, "syntax error: {s}"),
            MlirBridgeError::UnknownOp(s) => write!(f, "unknown meta op: {s}"),
            MlirBridgeError::BadIndex(s) => write!(f, "bad SSA index: {s}"),
            MlirBridgeError::BadInteger(s) => write!(f, "bad integer literal: {s}"),
            MlirBridgeError::BadHeader => write!(f, "bad meta.term header"),
            MlirBridgeError::BadFooter => write!(f, "bad meta.term footer"),
            MlirBridgeError::MissingRoot => write!(f, "missing meta.root line"),
            MlirBridgeError::DuplicateRoot => write!(f, "duplicate meta.root line"),
            MlirBridgeError::UnresolvedRef(i) => write!(f, "unresolved SSA reference %{i}"),
            MlirBridgeError::EmptyModule => write!(f, "empty meta.term module (no root)"),
        }
    }
}

impl std::error::Error for MlirBridgeError {}

// ---------------------------------------------------------------------------
// Emit
// ---------------------------------------------------------------------------

/// Émet un `Term` dans la forme MLIR text du dialecte `meta`.
///
/// Format strictement déterministe : indentation 2 espaces, tri topologique
/// post-ordre (les feuilles d'abord). Aucune ambiguïté, aucun espace
/// flottant.
pub fn emit_meta_mlir(t: &Term) -> String {
    let mut out = String::new();
    out.push_str("meta.term {\n");

    let mut emitter = Emitter { out: &mut out, next_idx: 0 };
    let root_idx = emitter.emit_node(t);

    let _ = writeln!(out, "  meta.root {SSA_PREFIX}{root_idx}");
    out.push_str("}\n");
    out
}

struct Emitter<'a> {
    out: &'a mut String,
    next_idx: usize,
}

impl<'a> Emitter<'a> {
    fn emit_node(&mut self, t: &Term) -> usize {
        match t {
            Term::Var(i) => {
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.var {{index = {i}}}",
                );
                idx
            }
            Term::Sort(n) => {
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.sort {{level = {n}}}",
                );
                idx
            }
            Term::Lam { ty, body } => {
                let ty_idx = self.emit_node(ty);
                let body_idx = self.emit_node(body);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.lam {SSA_PREFIX}{ty_idx}, {SSA_PREFIX}{body_idx}",
                );
                idx
            }
            Term::Pi { ty, body } => {
                let ty_idx = self.emit_node(ty);
                let body_idx = self.emit_node(body);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.pi {SSA_PREFIX}{ty_idx}, {SSA_PREFIX}{body_idx}",
                );
                idx
            }
            Term::App(f, x) => {
                let f_idx = self.emit_node(f);
                let x_idx = self.emit_node(x);
                let idx = self.alloc_idx();
                let _ = writeln!(
                    self.out,
                    "  {SSA_PREFIX}{idx} = meta.app {SSA_PREFIX}{f_idx}, {SSA_PREFIX}{x_idx}",
                );
                idx
            }
        }
    }

    fn alloc_idx(&mut self) -> usize {
        let i = self.next_idx;
        self.next_idx += 1;
        i
    }
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

/// Parse une forme MLIR text émise par [`emit_meta_mlir`] et reconstruit
/// le `Term` d'origine.
pub fn parse_meta_mlir(text: &str) -> Result<Term, MlirBridgeError> {
    let mut lines = text.lines();

    // Header : doit être "meta.term {".
    let header = lines
        .find(|l| !l.trim().is_empty())
        .ok_or(MlirBridgeError::EmptyModule)?;
    if header.trim() != "meta.term {" {
        return Err(MlirBridgeError::BadHeader);
    }

    let mut bindings: HashMap<usize, Term> = HashMap::new();
    let mut next_expected_idx: usize = 0;
    let mut root: Option<Term> = None;
    let mut footer_seen = false;

    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "}" {
            footer_seen = true;
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("meta.root ") {
            if root.is_some() {
                return Err(MlirBridgeError::DuplicateRoot);
            }
            let idx = parse_ssa(rest)?;
            let term = bindings
                .get(&idx)
                .ok_or(MlirBridgeError::UnresolvedRef(idx))?
                .clone();
            root = Some(term);
            continue;
        }

        // Forme : "%N = meta.OP <body>"
        let (lhs, rhs) = trimmed
            .split_once('=')
            .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `=` in `{trimmed}`")))?;
        let idx = parse_ssa(lhs.trim())?;
        if idx != next_expected_idx {
            return Err(MlirBridgeError::Syntax(format!(
                "expected SSA index {next_expected_idx}, got {idx}",
            )));
        }
        next_expected_idx += 1;

        let rhs = rhs.trim();
        let (op_token, body) = match rhs.split_once(' ') {
            Some((op, body)) => (op, body.trim()),
            None => (rhs, ""),
        };
        let op_mnem = op_token
            .strip_prefix("meta.")
            .ok_or_else(|| MlirBridgeError::UnknownOp(op_token.to_string()))?;

        let term = match op_mnem {
            "var" => {
                let n = parse_attr_u32(body, "index")?;
                Term::var(n)
            }
            "sort" => {
                let n = parse_attr_u32(body, "level")?;
                Term::sort(n)
            }
            "lam" | "pi" | "app" => {
                let (a, b) = parse_two_ssa(body)?;
                let lhs_term = bindings
                    .get(&a)
                    .ok_or(MlirBridgeError::UnresolvedRef(a))?
                    .clone();
                let rhs_term = bindings
                    .get(&b)
                    .ok_or(MlirBridgeError::UnresolvedRef(b))?
                    .clone();
                match op_mnem {
                    "lam" => Term::lam(lhs_term, rhs_term),
                    "pi" => Term::pi(lhs_term, rhs_term),
                    "app" => Term::app(lhs_term, rhs_term),
                    _ => unreachable!(),
                }
            }
            _ => return Err(MlirBridgeError::UnknownOp(op_mnem.to_string())),
        };

        bindings.insert(idx, term);
    }

    if !footer_seen {
        return Err(MlirBridgeError::BadFooter);
    }

    root.ok_or(MlirBridgeError::MissingRoot)
}

fn parse_ssa(s: &str) -> Result<usize, MlirBridgeError> {
    let s = s.trim();
    let n = s
        .strip_prefix(SSA_PREFIX)
        .ok_or_else(|| MlirBridgeError::BadIndex(s.to_string()))?;
    n.parse::<usize>()
        .map_err(|_| MlirBridgeError::BadIndex(s.to_string()))
}

fn parse_two_ssa(s: &str) -> Result<(usize, usize), MlirBridgeError> {
    let mut parts = s.split(',');
    let a = parts
        .next()
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected two SSA in `{s}`")))?;
    let b = parts
        .next()
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected two SSA in `{s}`")))?;
    if parts.next().is_some() {
        return Err(MlirBridgeError::Syntax(format!(
            "too many operands in `{s}`",
        )));
    }
    Ok((parse_ssa(a)?, parse_ssa(b)?))
}

fn parse_attr_u32(body: &str, key: &str) -> Result<u32, MlirBridgeError> {
    let body = body.trim();
    let lbrace = body
        .find('{')
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `{{` in `{body}`")))?;
    let rbrace = body
        .rfind('}')
        .ok_or_else(|| MlirBridgeError::Syntax(format!("expected `}}` in `{body}`")))?;
    if rbrace <= lbrace {
        return Err(MlirBridgeError::Syntax(format!(
            "malformed attr block in `{body}`",
        )));
    }
    let inner = &body[lbrace + 1..rbrace];
    let (k, v) = inner.split_once('=').ok_or_else(|| {
        MlirBridgeError::Syntax(format!("expected `key = value` in `{inner}`"))
    })?;
    if k.trim() != key {
        return Err(MlirBridgeError::Syntax(format!(
            "expected attr `{key}`, got `{}`",
            k.trim()
        )));
    }
    let v = v.trim();
    v.parse::<u32>()
        .map_err(|_| MlirBridgeError::BadInteger(v.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::embed_program;

    fn assert_roundtrip(t: &Term, label: &str) {
        let text = emit_meta_mlir(t);
        let t2 = parse_meta_mlir(&text).unwrap_or_else(|e| {
            panic!("[{label}] parse_meta_mlir failed: {e}\nMLIR:\n{text}");
        });
        assert_eq!(*t, t2, "[{label}] structural mismatch");
        assert_eq!(t.hash(), t2.hash(), "[{label}] hash mismatch");
    }

    // -------- 1. roundtrip_var --------
    #[test]
    fn roundtrip_var() {
        let t = Term::var(5);
        let text = emit_meta_mlir(&t);
        let parsed = parse_meta_mlir(&text).unwrap();
        assert_eq!(parsed, t);
        // Forme attendue, byte-exact.
        assert_eq!(
            text,
            "meta.term {\n  %0 = meta.var {index = 5}\n  meta.root %0\n}\n"
        );
    }

    // -------- 2. roundtrip_sort --------
    #[test]
    fn roundtrip_sort() {
        let t = Term::sort(7);
        let text = emit_meta_mlir(&t);
        let parsed = parse_meta_mlir(&text).unwrap();
        assert_eq!(parsed, t);
        assert_eq!(
            text,
            "meta.term {\n  %0 = meta.sort {level = 7}\n  meta.root %0\n}\n"
        );
    }

    // -------- 3. roundtrip_lam --------
    #[test]
    fn roundtrip_lam() {
        // λ x: Sort(0). x  ≡ identity at Type
        let t = Term::lam(Term::sort(0), Term::var(0));
        assert_roundtrip(&t, "lam_id");
    }

    // -------- 4. roundtrip_pi --------
    #[test]
    fn roundtrip_pi() {
        // Π x: Sort(0). x
        let t = Term::pi(Term::sort(0), Term::var(0));
        assert_roundtrip(&t, "pi_id");
    }

    // -------- 5. roundtrip_app --------
    #[test]
    fn roundtrip_app() {
        let t = Term::app(Term::var(0), Term::var(1));
        assert_roundtrip(&t, "app_var0_var1");
    }

    // -------- 6. roundtrip_polymorphic_identity --------
    #[test]
    fn roundtrip_polymorphic_identity() {
        // λ A: Sort(1). λ x: A. x
        let inner = Term::lam(Term::var(0), Term::var(0));
        let t = Term::lam(Term::sort(1), inner);
        assert_roundtrip(&t, "poly_id");
    }

    // -------- 7. roundtrip_nested_apps_and_binders --------
    #[test]
    fn roundtrip_nested_apps_and_binders() {
        // λ x: Sort(0). λ y: Sort(1). (Π z: Sort(2). App(App(x, y), App(z, x)))
        // Profondeur ≥ 5.
        let depth5 = Term::lam(
            Term::sort(0),
            Term::lam(
                Term::sort(1),
                Term::pi(
                    Term::sort(2),
                    Term::app(
                        Term::app(Term::var(2), Term::var(1)),
                        Term::app(Term::var(0), Term::var(2)),
                    ),
                ),
            ),
        );
        assert_roundtrip(&depth5, "depth5");
    }

    // -------- 8. emit_is_deterministic --------
    #[test]
    fn emit_is_deterministic() {
        let t = Term::lam(
            Term::sort(0),
            Term::app(Term::var(0), Term::lam(Term::sort(1), Term::var(0))),
        );
        let a = emit_meta_mlir(&t);
        let b = emit_meta_mlir(&t);
        assert_eq!(a, b, "emit non-deterministic");
    }

    // -------- 9. parse_rejects_garbage --------
    #[test]
    fn parse_rejects_garbage() {
        let bad = "this is not valid MLIR text at all\n";
        assert!(parse_meta_mlir(bad).is_err());

        let bad2 = "meta.term {\n  garbage line\n}\n";
        assert!(parse_meta_mlir(bad2).is_err());

        let bad3 = "meta.term {\n  %0 = meta.var {index = 5}\n";
        // Missing footer.
        assert!(matches!(parse_meta_mlir(bad3), Err(MlirBridgeError::BadFooter)));

        let bad_root = "meta.term {\n  %0 = meta.var {index = 5}\n}\n";
        // Missing meta.root line.
        assert!(matches!(parse_meta_mlir(bad_root), Err(MlirBridgeError::MissingRoot)));
    }

    // -------- 10. parse_rejects_unknown_op --------
    #[test]
    fn parse_rejects_unknown_op() {
        let bad = "meta.term {\n  %0 = meta.zorglub {index = 0}\n  meta.root %0\n}\n";
        let result = parse_meta_mlir(bad);
        assert!(matches!(result, Err(MlirBridgeError::UnknownOp(_))));

        // Mauvais préfixe (pas meta.).
        let bad2 = "meta.term {\n  %0 = kasm.input {slot = 0} : i64\n  meta.root %0\n}\n";
        let r2 = parse_meta_mlir(bad2);
        assert!(matches!(r2, Err(MlirBridgeError::UnknownOp(_))));
    }

    // -------- 11. roundtrip_preserves_hash --------
    #[test]
    fn roundtrip_preserves_hash() {
        // Échantillon varié — chaque term doit conserver son hash content-addressed.
        let cases = vec![
            ("var0", Term::var(0)),
            ("var_max", Term::var(u32::MAX)),
            ("sort0", Term::sort(0)),
            ("sort_max", Term::sort(u32::MAX)),
            ("id", Term::lam(Term::sort(0), Term::var(0))),
            ("pi_arrow", Term::pi(Term::sort(0), Term::sort(0))),
            (
                "self_app",
                Term::lam(Term::ty(), Term::app(Term::var(0), Term::var(0))),
            ),
        ];
        for (label, t) in cases {
            let text = emit_meta_mlir(&t);
            let t2 = parse_meta_mlir(&text)
                .unwrap_or_else(|e| panic!("[{label}] parse failed: {e}\n{text}"));
            assert_eq!(t.hash(), t2.hash(), "[{label}] hash drift after roundtrip");
        }
    }

    // -------- 13. roundtrip_on_kasm_embedded_program --------
    #[test]
    fn roundtrip_on_kasm_embedded_program() {
        use crate::kasm::{Node, Program, Target, Ty};

        // Petit programme jouet f(x) = 3*x + 1.
        let nodes = vec![
            Node::input(0),
            Node::const_i64(3),
            Node::mul(0, 1),
            Node::const_i64(1),
            Node::add(2, 3),
            Node::output(4, Ty::I64),
        ];
        let program = Program::new(Target::Cpu, 1, 1, 16, nodes).unwrap();
        let term = embed_program(&program);
        assert_roundtrip(&term, "kasm_embed_affine");
    }

    // -------- bonus : structurels supplémentaires --------

    #[test]
    fn topological_order_is_postorder() {
        // λ x: Sort(0). x → ordre attendu : %0 = sort, %1 = var, %2 = lam.
        let t = Term::lam(Term::sort(0), Term::var(0));
        let text = emit_meta_mlir(&t);
        let expected = "meta.term {\n  \
            %0 = meta.sort {level = 0}\n  \
            %1 = meta.var {index = 0}\n  \
            %2 = meta.lam %0, %1\n  \
            meta.root %2\n\
            }\n";
        assert_eq!(text, expected);
    }

    #[test]
    fn alpha_equivalent_terms_share_emission() {
        // En de Bruijn, l'α-équivalence est structurelle. Deux constructions
        // de "λ x: Type. x" donnent la même chaîne MLIR.
        let a = Term::lam(Term::ty(), Term::var(0));
        let b = Term::lam(Term::sort(0), Term::var(0));
        assert_eq!(emit_meta_mlir(&a), emit_meta_mlir(&b));
    }

    #[test]
    fn rejects_duplicate_root() {
        let bad = "meta.term {\n  \
            %0 = meta.var {index = 0}\n  \
            meta.root %0\n  \
            meta.root %0\n\
            }\n";
        assert!(matches!(
            parse_meta_mlir(bad),
            Err(MlirBridgeError::DuplicateRoot)
        ));
    }

    #[test]
    fn rejects_unresolved_ssa_ref() {
        let bad = "meta.term {\n  \
            %0 = meta.lam %7, %8\n  \
            meta.root %0\n\
            }\n";
        assert!(matches!(
            parse_meta_mlir(bad),
            Err(MlirBridgeError::UnresolvedRef(_))
        ));
    }

    #[test]
    fn rejects_out_of_order_indices() {
        // Format exige ordre croissant strict 0, 1, 2, ...
        let bad = "meta.term {\n  \
            %5 = meta.var {index = 0}\n  \
            meta.root %5\n\
            }\n";
        assert!(parse_meta_mlir(bad).is_err());
    }

    #[test]
    fn parse_strips_trailing_whitespace_only() {
        // Les lignes vides intercalaires sont tolérées (cohérent avec kasm/mlir).
        let text = "meta.term {\n\n  %0 = meta.var {index = 3}\n\n  meta.root %0\n}\n";
        let t = parse_meta_mlir(text).unwrap();
        assert_eq!(t, Term::var(3));
    }

    #[test]
    fn deep_self_app_roundtrip() {
        // (λ x: Type. x x) (λ x: Type. x x) = Ω. On le construit mais on ne
        // normalise pas — on roundtripe juste sa structure.
        let dup = Term::lam(Term::ty(), Term::app(Term::var(0), Term::var(0)));
        let omega = Term::app(dup.clone(), dup);
        assert_roundtrip(&omega, "omega_self_app");
    }

    #[test]
    fn arrow_type_roundtrip() {
        // a → b non dépendant : Π a. (b lift 0 1).
        let a = Term::sort(0);
        let b = Term::sort(1);
        let arrow = Term::arrow(a, b);
        assert_roundtrip(&arrow, "arrow_type");
    }

}
