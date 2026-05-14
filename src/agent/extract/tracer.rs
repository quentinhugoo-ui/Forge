//! Tracer + builder thread-local pour Ω-2.0.
//!
//! Le `Tracer` est un index opaque dans un builder de programme KASM.
//! Toutes les opérations sur Tracers s'enregistrent comme des nœuds
//! KASM via le builder thread-local. Quand l'extraction se termine,
//! le builder est consommé pour produire un `Program` final.

use std::cell::RefCell;
use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Shl, Shr, Sub};

use crate::kasm::{KasmError, Node, Program, Target, Ty, MAX_NODES};

#[derive(Debug)]
pub enum ExtractError {
    /// Tentative d'extraction nichée (pas supporté en Ω-2.0).
    NestedExtraction,
    /// Builder absent au moment de l'op (Tracer utilisé hors extract()).
    NoActiveBuilder,
    /// Plus de 4096 nœuds (limite KASM).
    TooManyNodes,
    /// Plus de 16 inputs ou outputs.
    TooManySlots,
    /// Const i64 hors plage i16 (limitation KASM ConstI64 imm).
    ConstOutOfRange(i64),
    /// Builder inconsistant (état corrompu).
    BuilderCorrupted,
    /// Erreur KASM lors de la finalisation.
    Kasm(KasmError),
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtractError::NestedExtraction => write!(f, "nested extraction not supported"),
            ExtractError::NoActiveBuilder => write!(f, "no active extraction builder"),
            ExtractError::TooManyNodes => write!(f, "more than {} nodes", MAX_NODES),
            ExtractError::TooManySlots => write!(f, "too many input/output slots"),
            ExtractError::ConstOutOfRange(v) => {
                write!(f, "const {v} does not fit in i16")
            }
            ExtractError::BuilderCorrupted => write!(f, "builder state corrupted"),
            ExtractError::Kasm(e) => write!(f, "kasm: {e}"),
        }
    }
}

impl std::error::Error for ExtractError {}

impl From<KasmError> for ExtractError {
    fn from(e: KasmError) -> Self {
        ExtractError::Kasm(e)
    }
}

// ---------------------------------------------------------------------------
// Builder thread-local
// ---------------------------------------------------------------------------

struct Builder {
    nodes: Vec<Node>,
    inputs: u8,
}

impl Builder {
    fn new(inputs: u8) -> Self {
        let mut nodes = Vec::with_capacity(64);
        for slot in 0..inputs {
            nodes.push(Node::input(slot));
        }
        Self { nodes, inputs }
    }

    fn push(&mut self, node: Node) -> Result<u16, ExtractError> {
        if self.nodes.len() >= MAX_NODES {
            return Err(ExtractError::TooManyNodes);
        }
        let idx = self.nodes.len() as u16;
        self.nodes.push(node);
        Ok(idx)
    }
}

thread_local! {
    static BUILDER: RefCell<Option<Builder>> = const { RefCell::new(None) };
}

fn with_builder<R>(f: impl FnOnce(&mut Builder) -> Result<R, ExtractError>) -> R {
    BUILDER.with(|b| {
        let mut slot = b.borrow_mut();
        let builder = slot.as_mut().expect(
            "Tracer used outside of extract() — extraction context inactive",
        );
        f(builder).expect("tracer op failed inside extract()")
    })
}

// ---------------------------------------------------------------------------
// Tracer
// ---------------------------------------------------------------------------

/// Handle opaque vers un nœud KASM dans le builder thread-local courant.
///
/// `Tracer` n'est utilisable QU'À L'INTÉRIEUR d'un appel à `extract()`.
/// Toute opération std::ops::* enregistre un nœud dans le builder courant.
#[derive(Clone, Copy, Debug)]
pub struct Tracer {
    node_idx: u16,
}

impl Tracer {
    /// Construit un Tracer pour un slot d'entrée du programme.
    /// Réservé à l'usage interne d'`extract()` ; les utilisateurs reçoivent
    /// les inputs via le `Vec<Tracer>` passé à leur closure.
    fn from_input_slot(slot: u8) -> Self {
        // Les inputs sont les `inputs` premiers nodes du builder, donc
        // node_idx = slot pour les inputs (Builder::new les empile dans
        // l'ordre des slots).
        Self { node_idx: slot as u16 }
    }

    /// Construit un Tracer constant à partir d'une valeur i16.
    /// Pour les valeurs hors i16, voir Ω-2.0.x (encodage par bit_xor / shifts).
    pub fn const_i16(value: i16) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::const_i64(value))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Helper : construit un Tracer constant depuis i64. Erreur compile-time
    /// impossible (i64 -> i16 cast), mais panique à runtime si hors plage.
    pub fn const_(value: i64) -> Self {
        let v = i16::try_from(value).expect("Tracer::const_ value out of i16 range");
        Self::const_i16(v)
    }

    /// Min(self, other) — élémentaire dans KASM.
    pub fn min(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::min(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Max(self, other).
    pub fn max(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::max(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Division checkée (a / b, ou 0 si b == 0).
    pub fn div_checked(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::div_checked(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Modulo Euclidien (a mod b, ou 0 si b == 0).
    pub fn mod_checked(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::mod_checked(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Saturating add.
    pub fn sat_add(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sat_add(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Saturating sub.
    pub fn sat_sub(self, other: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sat_sub(self.node_idx, other.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Cross-type : i64 → bool (Ω-2.0.1) -----

    /// `self == other` → `BoolTracer`.
    pub fn eq(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::eq(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    /// `self < other` (signed) → `BoolTracer`.
    pub fn lt(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::lt(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    /// `self <= other` (signed) → `BoolTracer`.
    pub fn le(self, other: Self) -> BoolTracer {
        with_builder(|b| {
            let idx = b.push(Node::le(self.node_idx, other.node_idx))?;
            Ok(BoolTracer { node_idx: idx })
        })
    }

    // ----- Clamp ternaire (Ω-2.0.3) -----

    /// `clamp(self, lo, hi)` — KASM Clamp ternaire.
    pub fn clamp(self, lo: Self, hi: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::clamp(self.node_idx, lo.node_idx, hi.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Hash64 (Ω-2.0.4) -----

    /// Hash 64-bit du Tracer (KASM Hash64).
    pub fn hash64(self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::hash64(self.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Reduce ops (Ω-2.0.2) -----

    /// Réduction par addition d'un slice de Tracers contigus.
    ///
    /// `items` doit être non-vide ET les Tracers doivent être contigus
    /// dans le builder (i.e. `items[i].node_idx == items[0].node_idx + i`
    /// pour tout `i`). C'est une contrainte structurelle de KASM
    /// `ReduceAddI64` qui référence un intervalle `[base, base + count)`.
    ///
    /// # Panique
    ///
    /// - Si `items` est vide.
    /// - Si les Tracers ne sont pas contigus.
    /// - Si `items.len() > i16::MAX` (limite KASM count).
    pub fn reduce_add(items: &[Tracer]) -> Tracer {
        let (base, count) = check_contiguous(items, "reduce_add");
        with_builder(|b| {
            let idx = b.push(Node::reduce_add(base, count))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Réduction par multiplication d'un slice de Tracers contigus.
    /// Mêmes contraintes que `reduce_add`.
    pub fn reduce_mul(items: &[Tracer]) -> Tracer {
        let (base, count) = check_contiguous(items, "reduce_mul");
        with_builder(|b| {
            let idx = b.push(Node::reduce_mul(base, count))?;
            Ok(Self { node_idx: idx })
        })
    }

    // ----- Constantes wide (Ω-2.0.6) -----

    /// Construit un Tracer pour n'importe quelle valeur i64.
    ///
    /// Pour les valeurs dans `i16::MIN..=i16::MAX`, c'est un seul nœud
    /// Const. Sinon, on décompose en 4 chunks de 16 bits + shifts/ors —
    /// jusqu'à 15 nœuds dans le pire cas. Le coût est documenté dans le
    /// doc Ω-2.
    ///
    /// Astuce de masquage : pour zeroer les bits au-delà des 16 bas d'un
    /// nombre `x`, on calcule `(x << 48) >> 48` (logical right shift KASM).
    pub fn const_i64_wide(value: i64) -> Self {
        if let Ok(small) = i16::try_from(value) {
            return Self::const_i16(small);
        }

        let bits = value as u64;
        let chunk = |shift: u32| -> u16 { ((bits >> shift) & 0xFFFF) as u16 };

        // Construit les 4 chunks. Pour les chunks bas (qu'on va shifter et
        // masquer), on utilise le trick (x << 48) >> 48.
        // Le top chunk (bits 63..48) peut directement être un i16 signé,
        // car le sign-extend est ce qu'on veut pour préserver les bits hauts.
        let top_chunk = chunk(48);
        let mid_hi_chunk = chunk(32);
        let mid_lo_chunk = chunk(16);
        let low_chunk = chunk(0);

        let const48 = Self::const_i16(48);

        // Top chunk : on l'utilise tel quel via i16 (sign-extend OK car c'est
        // le top de l'i64).
        let top_signed = top_chunk as i16;
        let top = Self::const_i16(top_signed);
        let top_shifted = top << Self::const_i16(48);

        // Helper pour construire un chunk masqué et shifté à une position.
        // Utilise le trick (x << 48) >> 48 pour zeroer le sign-extend.
        let masked_chunk = |c: u16| -> Self {
            let signed = c as i16; // peut sign-extend, OK car on masque ensuite
            let raw = Self::const_i16(signed);
            // (raw << 48) >> 48 = juste les 16 bits bas
            let up = raw << const48;
            up >> const48
        };

        let mid_hi = masked_chunk(mid_hi_chunk) << Self::const_i16(32);
        let mid_lo = masked_chunk(mid_lo_chunk) << Self::const_i16(16);
        let low = masked_chunk(low_chunk);

        // Combine via BitOr.
        top_shifted | mid_hi | mid_lo | low
    }
}

/// Vérifie qu'un slice de Tracers est non-vide, que `len` tient dans
/// `i16::MAX` (contrainte KASM count), et que les indices sont contigus.
/// Retourne `(base, count)` prêts à passer à `Node::reduce_*`.
fn check_contiguous(items: &[Tracer], op_name: &str) -> (u16, u16) {
    assert!(
        !items.is_empty(),
        "Tracer::{op_name}: items must be non-empty (KASM Reduce* requires count >= 1)"
    );
    assert!(
        items.len() <= i16::MAX as usize,
        "Tracer::{op_name}: count {} exceeds KASM limit i16::MAX",
        items.len()
    );
    let base = items[0].node_idx;
    for (i, t) in items.iter().enumerate() {
        let expected = base.checked_add(i as u16).unwrap_or_else(|| {
            panic!("Tracer::{op_name}: contiguous range overflows u16 at i={i}")
        });
        assert_eq!(
            t.node_idx, expected,
            "Tracer::{op_name}: items must be contiguous in builder; \
             items[{i}].node_idx = {} expected {} (base = {base}). \
             Reduce* requires the slice to map to a contiguous KASM index range.",
            t.node_idx, expected
        );
    }
    (base, items.len() as u16)
}

// ---------------------------------------------------------------------------
// BoolTracer (Ω-2.0.1)
// ---------------------------------------------------------------------------

/// Tracer bool. Produit par les comparaisons (`Tracer::eq/lt/le`) ;
/// supporte les opérations logiques (`&`, `|`, `not`) et le `select`
/// ternaire qui retombe sur i64.
#[derive(Clone, Copy, Debug)]
pub struct BoolTracer {
    node_idx: u16,
}

impl BoolTracer {
    /// Négation logique. KASM NotBool est bijective (Landauer-cost zéro).
    pub fn not(self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::not(self.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }

    /// Sélection ternaire : `if self { then_branch } else { else_branch }`.
    pub fn select(self, then_branch: Tracer, else_branch: Tracer) -> Tracer {
        with_builder(|b| {
            let idx = b.push(Node::select_i64(
                self.node_idx,
                then_branch.node_idx,
                else_branch.node_idx,
            ))?;
            Ok(Tracer { node_idx: idx })
        })
    }
}

impl std::ops::BitAnd for BoolTracer {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::and(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl std::ops::BitOr for BoolTracer {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::or(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

// ---------------------------------------------------------------------------
// std::ops::* — opérateurs Rust → nœuds KASM
// ---------------------------------------------------------------------------

impl Add for Tracer {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::add(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Sub for Tracer {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::sub(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Mul for Tracer {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::mul(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitAnd for Tracer {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_and(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitOr for Tracer {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_or(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl BitXor for Tracer {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::bit_xor(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Shl for Tracer {
    type Output = Self;
    fn shl(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::shl(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

impl Shr for Tracer {
    type Output = Self;
    fn shr(self, rhs: Self) -> Self {
        with_builder(|b| {
            let idx = b.push(Node::shr(self.node_idx, rhs.node_idx))?;
            Ok(Self { node_idx: idx })
        })
    }
}

// Ergonomie : Tracer + i64 (et symétrique).
impl Add<i64> for Tracer {
    type Output = Self;
    fn add(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self + c
    }
}

impl Mul<i64> for Tracer {
    type Output = Self;
    fn mul(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self * c
    }
}

impl Sub<i64> for Tracer {
    type Output = Self;
    fn sub(self, rhs: i64) -> Self {
        let c = Tracer::const_(rhs);
        self - c
    }
}

// ---------------------------------------------------------------------------
// extract() — point d'entrée principal
// ---------------------------------------------------------------------------

/// Extrait une fonction `Fn(Vec<Tracer>) -> Tracer` en `kasm::Program`.
///
/// `n_inputs` doit être ≥ 1 et ≤ `MAX_SLOTS` (16). Le programme produit
/// a 1 output (le Tracer retourné par la closure).
///
/// Aucune extraction nichée n'est supportée — appeler `extract()` à
/// l'intérieur d'une autre extraction renvoie `NestedExtraction`.
pub fn extract<F>(n_inputs: u8, f: F) -> Result<Program, ExtractError>
where
    F: FnOnce(Vec<Tracer>) -> Tracer,
{
    if n_inputs == 0 || n_inputs as usize > 16 {
        return Err(ExtractError::TooManySlots);
    }

    // Setup builder.
    let already_active = BUILDER.with(|b| b.borrow().is_some());
    if already_active {
        return Err(ExtractError::NestedExtraction);
    }
    BUILDER.with(|b| {
        *b.borrow_mut() = Some(Builder::new(n_inputs));
    });

    // Construit les Tracers d'entrée.
    let inputs: Vec<Tracer> = (0..n_inputs).map(Tracer::from_input_slot).collect();

    // Run the closure. On capture le résultat AVANT de finaliser le builder
    // pour que les éventuels panics dans f libèrent la TLS proprement.
    let result_idx = {
        // Wrap dans un guard pour reset le TLS si f panique.
        let guard = ExtractGuard;
        let result = f(inputs);
        std::mem::forget(guard); // succès — pas besoin de cleanup panic-side
        result.node_idx
    };

    // Récupère le builder, ajoute output, finalise.
    let builder = BUILDER
        .with(|b| b.borrow_mut().take())
        .ok_or(ExtractError::BuilderCorrupted)?;

    let mut nodes = builder.nodes;
    let total_nodes_before_output = nodes.len();
    if total_nodes_before_output >= MAX_NODES {
        return Err(ExtractError::TooManyNodes);
    }
    nodes.push(Node::output(result_idx, Ty::I64));

    let total = nodes.len() as u32;
    let program = Program::new(Target::Cpu, builder.inputs, 1, total, nodes)?;
    Ok(program)
}

/// Guard RAII : si `extract()` panique au milieu, libère la TLS.
struct ExtractGuard;
impl Drop for ExtractGuard {
    fn drop(&mut self) {
        BUILDER.with(|b| {
            *b.borrow_mut() = None;
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::execute;

    fn exec_one(p: &Program, args: &[i64]) -> Vec<u8> {
        let bytes: Vec<u8> = args.iter().flat_map(|v| v.to_le_bytes()).collect();
        execute(p, &bytes).expect("kasm execute")
    }

    fn read_one_i64(out: &[u8]) -> i64 {
        i64::from_le_bytes(out[..8].try_into().unwrap())
    }

    #[test]
    fn extract_identity_function() {
        let prog = extract(1, |inputs| inputs[0]).unwrap();
        for x in [-100, -1, 0, 1, 42, 1_000_000] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), x);
        }
    }

    #[test]
    fn extract_addition() {
        let prog = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        for (a, b) in [(0, 0), (1, 2), (-3, 7), (1_000, 2_000)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    #[test]
    fn extract_affine_via_const() {
        // f(x) = 7 * x + 3 — utilise const_ via i64 ergonomique.
        let prog = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        for x in [-10, 0, 1, 5, 100] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), 7 * x + 3);
        }
    }

    #[test]
    fn extract_complex_arithmetic() {
        // f(x, y) = (x + y) * (x - y) = x² - y²
        let prog = extract(2, |inputs| {
            let sum = inputs[0] + inputs[1];
            let diff = inputs[0] - inputs[1];
            sum * diff
        })
        .unwrap();
        for (x, y) in [(3, 2), (10, 7), (-5, 3), (100, 99)] {
            let out = exec_one(&prog, &[x, y]);
            assert_eq!(read_one_i64(&out), x * x - y * y);
        }
    }

    #[test]
    fn extract_bitwise_ops() {
        // f(x, y) = (x & y) | (x ^ y)
        let prog = extract(2, |inputs| {
            (inputs[0] & inputs[1]) | (inputs[0] ^ inputs[1])
        })
        .unwrap();
        for (x, y) in [(0b1010, 0b1100), (0xff00, 0x00ff), (-1, 1)] {
            let out = exec_one(&prog, &[x, y]);
            let expected = (x & y) | (x ^ y);
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    #[test]
    fn extract_shifts() {
        // f(x, k) = (x << k) | (x >> k)
        let prog = extract(2, |inputs| {
            (inputs[0] << inputs[1]) | (inputs[0] >> inputs[1])
        })
        .unwrap();
        for (x, k) in [(1i64, 3i64), (0xff, 4), (1024, 10)] {
            let out = exec_one(&prog, &[x, k]);
            let kasm_shl = (x as u64).wrapping_shl((k as u64 & 63) as u32) as i64;
            let kasm_shr = (x as u64).wrapping_shr((k as u64 & 63) as u32) as i64;
            assert_eq!(read_one_i64(&out), kasm_shl | kasm_shr);
        }
    }

    #[test]
    fn extract_min_max() {
        let prog = extract(2, |inputs| inputs[0].min(inputs[1])).unwrap();
        let out = exec_one(&prog, &[3, 7]);
        assert_eq!(read_one_i64(&out), 3);
        let prog = extract(2, |inputs| inputs[0].max(inputs[1])).unwrap();
        let out = exec_one(&prog, &[3, 7]);
        assert_eq!(read_one_i64(&out), 7);
    }

    #[test]
    fn extract_div_mod() {
        // f(a, b) = a / b + a mod b
        let prog = extract(2, |inputs| {
            inputs[0].div_checked(inputs[1]) + inputs[0].mod_checked(inputs[1])
        })
        .unwrap();
        for (a, b) in [(10, 3), (100, 7), (-5, 2)] {
            let out = exec_one(&prog, &[a, b]);
            // KASM div_checked retourne 0 si b==0 ; mod_checked = a.checked_rem(b).unwrap_or(0).
            // Pour b ≠ 0, c'est a/b + a%b.
            let expected = a.checked_div(b).unwrap_or(0) + a.checked_rem(b).unwrap_or(0);
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    #[test]
    fn extract_sat_arithmetic() {
        let prog = extract(2, |inputs| inputs[0].sat_add(inputs[1])).unwrap();
        let out = exec_one(&prog, &[i64::MAX, 100]);
        assert_eq!(read_one_i64(&out), i64::MAX); // saturé.
    }

    #[test]
    fn extracted_program_is_content_addressed() {
        let prog1 = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        let prog2 = extract(1, |inputs| inputs[0] * 7 + 3).unwrap();
        assert_eq!(
            prog1.canonical_hash_hex().unwrap(),
            prog2.canonical_hash_hex().unwrap(),
            "deux extractions de la même closure → même hash canonique"
        );
    }

    #[test]
    fn extracted_distinct_closures_have_distinct_hashes() {
        let p_add = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        let p_mul = extract(2, |inputs| inputs[0] * inputs[1]).unwrap();
        assert_ne!(
            p_add.canonical_hash_hex().unwrap(),
            p_mul.canonical_hash_hex().unwrap()
        );
    }

    #[test]
    fn nested_extraction_errors() {
        let result = extract(1, |inputs| {
            // Tentative d'extraction interne.
            let _ = extract(1, |inner| inner[0]);
            inputs[0]
        });
        // L'inner extract() retourne Err(NestedExtraction), mais la closure
        // externe fait inputs[0], donc l'extract externe RÉUSSIT, juste
        // l'inner a été ignorée. Ce test vérifie que l'extract externe
        // ne crashe PAS et retourne le bon programme.
        assert!(result.is_ok());
    }

    #[test]
    fn nested_extraction_inner_returns_error() {
        let _outer_result = extract(1, |inputs| {
            // Inner extract() doit retourner NestedExtraction.
            let inner = extract(1, |inner_inputs| inner_inputs[0]);
            assert!(matches!(inner, Err(ExtractError::NestedExtraction)));
            inputs[0]
        });
    }

    #[test]
    fn extracted_program_has_correct_io_counts() {
        let prog = extract(2, |inputs| inputs[0] + inputs[1]).unwrap();
        assert_eq!(prog.inputs(), 2);
        assert_eq!(prog.outputs(), 1);
    }

    #[test]
    fn extracted_through_meta_embedding_is_hashable() {
        // Cross-cap : Ω-2.0 + Ω-4.1. L'extraction produit un Program qui
        // s'embed via meta::embed_program.
        use crate::meta::embed_program;
        let prog = extract(1, |inputs| inputs[0] + 7).unwrap();
        let term = embed_program(&prog);
        let h = term.hash();
        assert_ne!(h, [0u8; 32]);
    }

    // ----- Bool ops + Select (Ω-2.0.1) -----

    #[test]
    fn extract_eq_via_bool() {
        // f(x, y) = if x == y then 100 else 0
        let prog = extract(2, |inputs| {
            let cond = inputs[0].eq(inputs[1]);
            cond.select(Tracer::const_(100), Tracer::const_(0))
        })
        .unwrap();
        let out = exec_one(&prog, &[5, 5]);
        assert_eq!(read_one_i64(&out), 100);
        let out = exec_one(&prog, &[5, 6]);
        assert_eq!(read_one_i64(&out), 0);
    }

    #[test]
    fn extract_lt_le_select() {
        // f(x, y) = if x < y then x else y (= min)
        let prog = extract(2, |inputs| {
            inputs[0].lt(inputs[1]).select(inputs[0], inputs[1])
        })
        .unwrap();
        for (a, b) in [(3i64, 7), (10, 5), (-1, 1)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a.min(b));
        }
    }

    #[test]
    fn extract_bool_and_or_not() {
        // f(x, y, z) = if (x < y) && !(y < z) then 1 else 0
        let prog = extract(3, |inputs| {
            let lt_xy = inputs[0].lt(inputs[1]);
            let lt_yz = inputs[1].lt(inputs[2]);
            let cond = lt_xy & lt_yz.not();
            cond.select(Tracer::const_(1), Tracer::const_(0))
        })
        .unwrap();
        let out = exec_one(&prog, &[1, 5, 3]); // 1<5 et !(5<3) → 1
        assert_eq!(read_one_i64(&out), 1);
        let out = exec_one(&prog, &[1, 2, 3]); // 1<2 et !(2<3) = !true = false → 0
        assert_eq!(read_one_i64(&out), 0);
    }

    #[test]
    fn extract_bool_or_combines_predicates() {
        // f(x) = if x < 0 || x > 100 then -1 else x
        let prog = extract(1, |inputs| {
            let too_low = inputs[0].lt(Tracer::const_(0));
            let too_high = Tracer::const_(100).lt(inputs[0]);
            (too_low | too_high).select(Tracer::const_(-1), inputs[0])
        })
        .unwrap();
        for x in [-5i64, 0, 50, 100, 150] {
            let out = exec_one(&prog, &[x]);
            let expected = if x < 0 || x > 100 { -1 } else { x };
            assert_eq!(read_one_i64(&out), expected);
        }
    }

    // ----- Clamp (Ω-2.0.3) -----

    #[test]
    fn extract_clamp() {
        let prog = extract(1, |inputs| {
            inputs[0].clamp(Tracer::const_(-10), Tracer::const_(10))
        })
        .unwrap();
        for x in [-100i64, -10, 0, 10, 100] {
            let out = exec_one(&prog, &[x]);
            assert_eq!(read_one_i64(&out), x.clamp(-10, 10));
        }
    }

    // ----- Hash64 (Ω-2.0.4) -----

    #[test]
    fn extract_hash64_is_deterministic() {
        let prog = extract(1, |inputs| inputs[0].hash64()).unwrap();
        let h1 = exec_one(&prog, &[42]);
        let h2 = exec_one(&prog, &[42]);
        assert_eq!(h1, h2, "hash déterministe pour même input");
        let h3 = exec_one(&prog, &[43]);
        assert_ne!(h1, h3, "hash distingue inputs distincts");
    }

    // ----- Const wide (Ω-2.0.6) -----

    #[test]
    fn const_i64_wide_in_i16_range_is_single_node() {
        let prog = extract(1, |inputs| inputs[0] + Tracer::const_i64_wide(100)).unwrap();
        // 1 input + 1 const (i16 case) + 1 add + 1 output = 4 nodes.
        assert_eq!(prog.nodes().len(), 4);
        let out = exec_one(&prog, &[5]);
        assert_eq!(read_one_i64(&out), 105);
    }

    #[test]
    fn const_i64_wide_outside_i16_works() {
        let target: i64 = 0x1234_5678_9ABC_DEF0_u64 as i64;
        let prog = extract(1, |inputs| {
            inputs[0] + Tracer::const_i64_wide(target)
        })
        .unwrap();
        // Doit fonctionner — même si plusieurs nœuds.
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), target);
    }

    #[test]
    fn const_i64_wide_negative_large() {
        let target: i64 = -1_234_567_890_i64;
        let prog = extract(1, |inputs| {
            inputs[0] + Tracer::const_i64_wide(target)
        })
        .unwrap();
        let out = exec_one(&prog, &[1_000_000_000]);
        assert_eq!(read_one_i64(&out), 1_000_000_000 + target);
    }

    #[test]
    fn const_i64_wide_max_value() {
        let prog = extract(1, |inputs| {
            inputs[0] | Tracer::const_i64_wide(i64::MAX)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), i64::MAX);
    }

    // ----- Reduce (Ω-2.0.2) -----

    #[test]
    fn extract_reduce_add_4_items() {
        // sum(1, 2, 3, 4) = 10. Les 4 consts sont créées dans le builder
        // dans l'ordre, donc contiguës. ReduceAdd les réduit en une somme.
        let prog = extract(1, |_inputs| {
            let items = [
                Tracer::const_i16(1),
                Tracer::const_i16(2),
                Tracer::const_i16(3),
                Tracer::const_i16(4),
            ];
            Tracer::reduce_add(&items)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), 10);
    }

    #[test]
    fn extract_reduce_mul_3_items() {
        // prod(2, 3, 5) = 30.
        let prog = extract(1, |_inputs| {
            let items = [
                Tracer::const_i16(2),
                Tracer::const_i16(3),
                Tracer::const_i16(5),
            ];
            Tracer::reduce_mul(&items)
        })
        .unwrap();
        let out = exec_one(&prog, &[0]);
        assert_eq!(read_one_i64(&out), 30);
    }

    #[test]
    #[should_panic(expected = "must be contiguous")]
    fn extract_reduce_non_contiguous_panics() {
        // On insère un nœud entre deux consts, qui casse la contiguïté.
        let _ = extract(2, |inputs| {
            let a = Tracer::const_i16(1);
            // Cette opération crée un nœud entre `a` et `b` → casse contiguïté.
            let _filler = inputs[0] + inputs[1];
            let b = Tracer::const_i16(2);
            // a et b ne sont PAS contigus : panic attendu.
            Tracer::reduce_add(&[a, b])
        });
    }

    #[test]
    #[should_panic(expected = "non-empty")]
    fn extract_reduce_empty_panics() {
        let _ = extract(1, |_inputs| Tracer::reduce_add(&[]));
    }

    #[test]
    fn extract_reduce_add_with_inputs_contiguous() {
        // 2 inputs sont déjà placés en %0 et %1 (contigus). Reduce sur
        // les inputs eux-mêmes est valide.
        let prog = extract(2, |inputs| Tracer::reduce_add(&inputs)).unwrap();
        for (a, b) in [(3i64, 7), (-5, 5), (100, 200)] {
            let out = exec_one(&prog, &[a, b]);
            assert_eq!(read_one_i64(&out), a + b);
        }
    }

    // ----- Cross-cap : Ω-2 + Ω-6 (Landauer cost on bool/select programs) -----

    #[test]
    fn extracted_select_program_has_landauer_cost() {
        use crate::landauer::program_cost;
        let prog = extract(2, |inputs| {
            inputs[0].lt(inputs[1]).select(inputs[0], inputs[1])
        })
        .unwrap();
        let cost = program_cost(&prog);
        // Le programme contient au moins une comparaison (127 bits) + un select (65 bits).
        assert!(cost.total_bits_erased >= 127 + 65);
    }

    #[test]
    fn extracted_program_survives_canonicalization() {
        let prog = extract(2, |inputs| (inputs[0] + 0) * 1).unwrap();
        // Le canonicalize doit éliminer les opérations triviales (+0, *1).
        let canon = prog.canonical().unwrap();
        // Le programme canonique a strictement moins de nœuds.
        assert!(canon.nodes().len() < prog.nodes().len());
    }
}
