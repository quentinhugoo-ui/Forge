//! Ω-6 — Le Substrat Réversible : coût Landauer first-class.
//!
//! Chaque op KASM est tagguée selon sa **réversibilité info-théorique** :
//!
//!  * `Routing` — pas de calcul, pas d'erasure (Input, Output, Const).
//!  * `Bijective` — fonction bijective, exécutable sur hardware réversible
//!    sans coût Landauer (NotBool : `b → ¬b`).
//!  * `Lossy { bits_erased }` — l'op consomme N bits de plus qu'elle n'en
//!    produit. Coût Landauer ≥ N × kT·ln2.
//!
//! Le coût d'un programme entier = somme des bits erased × kT·ln2 × T.
//! À T = 300K, kT·ln2 ≈ 2.87 × 10⁻²¹ J. Les chiffres sont astronomiquement
//! petits aujourd'hui (un atome silicon dissipe bien plus). Mais la métrique
//! est **first-class** dans SCAN — la Gödel-machine peut désormais arbitrer
//! des rewrites par énergie, pas seulement par perf.
//!
//! ## Ce que ça permet
//!
//! - Détecter les ops "thermodynamiquement gourmandes" dans un programme.
//! - Comparer l'énergie minimale entre deux versions d'un optimizer.
//! - Préparer le terrain pour Ω-6.x où des KASM-réversibles seront introduits
//!   (XOR avec carry preservé, contre-add, etc.) qui auront `Bijective`.
//! - Mesurer une "journée MonsterNode" en joules cumulés (critère CARNET.md).
//!
//! ## Doctrine via negativa
//!
//! - Pas de simulation hardware réelle (pas de rod-logic, pas de supraconducteur).
//! - Pas de tagging dynamique (chaque op a un tag fixe, déterministe).
//! - Pas de modélisation de la chaleur dissipée par tour de boucle JIT —
//!   on prend Landauer comme **borne inférieure absolue** (le hardware
//!   actuel dissipe ~10⁵ × ce minimum).
//! - Pas d'unités étranges. Joules. Kelvins. Constantes physiques exactes.

use crate::kasm::tensor::{TensorOp, TensorProgram, TensorTy};
use crate::kasm::{Op, Program};

// ---------------------------------------------------------------------------
// Constantes physiques (CODATA 2018)
// ---------------------------------------------------------------------------

/// Constante de Boltzmann en J/K.
pub const BOLTZMANN_J_PER_K: f64 = 1.380_649e-23;

/// ln(2) — facteur Landauer.
const LN2: f64 = std::f64::consts::LN_2;

/// Température ambiante par défaut (300 K = 27 °C).
pub const DEFAULT_TEMP_K: f64 = 300.0;

/// Énergie Landauer pour effacer un bit à la température `t` kelvins.
pub fn landauer_per_bit_joules(t_kelvin: f64) -> f64 {
    BOLTZMANN_J_PER_K * t_kelvin * LN2
}

// ---------------------------------------------------------------------------
// Tagging des opcodes KASM
// ---------------------------------------------------------------------------

/// Réversibilité info-théorique d'une op KASM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reversibility {
    /// Pure routing — Input lit un slot, Output route, Const introduit
    /// une valeur constante. Aucun calcul, aucun bit effacé.
    Routing,
    /// Bijection — la fonction est inversible. À cette date, seul `NotBool`
    /// dans KASM est strictement bijective (`b → ¬b`).
    Bijective,
    /// Lossy — `bits_erased` bits perdus par invocation. Pour les ops à
    /// arity variable (Reduce*) la valeur retournée ici est conservatrice ;
    /// `program_cost` ajuste le coût exact selon `node.imm`.
    Lossy { bits_erased: u32 },
}

impl Reversibility {
    pub fn bits_erased(&self) -> u32 {
        match self {
            Reversibility::Routing | Reversibility::Bijective => 0,
            Reversibility::Lossy { bits_erased } => *bits_erased,
        }
    }

    pub fn is_irreversible(&self) -> bool {
        matches!(self, Reversibility::Lossy { .. })
    }
}

/// Réversibilité d'un opcode KASM. Voir le module-level doc pour le
/// modèle d'erasure utilisé.
///
/// Convention : un input i64 = 64 bits, i1 = 1 bit. `bits_erased` =
/// (somme des bits in) - (somme des bits out).
pub fn op_reversibility(op: Op) -> Reversibility {
    use Op::*;
    match op {
        // ----- Routing : aucun calcul -----
        Input | ConstI64 | Output => Reversibility::Routing,

        // ----- Bijective : NotBool + Ω-6.1 unaires bijectifs (BitFlip,
        // Neg via wrapping, ReverseBits, Byteswap). Aucun bit effacé :
        // chaque entrée a une unique sortie et inversement, exécutables
        // sur hardware réversible sans coût Landauer.
        NotBool | BitFlipI64 | NegI64 | ReverseBitsI64 | ByteswapI64 => Reversibility::Bijective,

        // ----- Bool binaires : 2 bits in, 1 bit out -----
        AndBool | OrBool => Reversibility::Lossy { bits_erased: 1 },

        // ----- Comparaisons i64×i64 → bool : 128 bits in, 1 bit out -----
        EqI64 | LtI64 | LeI64 => Reversibility::Lossy { bits_erased: 127 },

        // ----- Arithmétique i64×i64 → i64 : 128 bits in, 64 bits out -----
        AddI64 | SubI64 | MulI64 | DivI64Checked | MinI64 | MaxI64
        | BitAndI64 | BitOrI64 | BitXorI64 | ShlI64 | ShrI64
        | SatAddI64 | SatSubI64 | ModI64Checked
        | PextI64 | PdepI64 => Reversibility::Lossy { bits_erased: 64 },

        // Bit-count projections collapse a 64-bit pattern into a small
        // count, so they are globally useful but not reversible.
        PopcntI64 | LzcntI64 | TzcntI64 => Reversibility::Lossy { bits_erased: 58 },

        // ----- Hash64 : 64 bits in, 64 bits out, MAIS one-way en pratique.
        // L'op compresse l'information de façon irrécupérable (sha-style).
        // On compte 64 bits erased — c'est la borne théorique pour un
        // permutation aléatoire indistinguable d'une bijection ; on la
        // marque Lossy pour refléter la non-inversibilité opérationnelle.
        Hash64 => Reversibility::Lossy { bits_erased: 64 },

        // ----- Select i1×i64×i64 → i64 : 129 bits in, 64 bits out -----
        SelectI64 => Reversibility::Lossy { bits_erased: 65 },

        // ----- Clamp i64×i64×i64 → i64 : 192 bits in, 64 bits out -----
        ClampI64 => Reversibility::Lossy { bits_erased: 128 },

        // ----- Reduce* : count * 64 bits in, 64 bits out.
        // La valeur retournée ici est conservatrice (count = 1 → 0 erased,
        // mais count valide ≥ 1). `program_cost` lit `node.imm` pour le
        // coût exact.
        ReduceAddI64 | ReduceMulI64 => Reversibility::Lossy { bits_erased: 64 },

        // ----- Φ.0 — IEEE 754 layer.
        //   * `ConstF64`  : routing (immediate cast, 0 bits in).
        //   * `F64Op`     : binary sub-ops collapse 128→64 bits, unary
        //                   F64→F64 are bijective in the f64 domain
        //                   (Sqrt is bijective on its restricted domain
        //                   only, but we count it lossy because we
        //                   collapse non-finite results to 0). The
        //                   conversion sub-ops are routing (FromI64) or
        //                   lossy 64→64 (ToI64 collapses NaN/Inf to 0).
        ConstF64 => Reversibility::Routing,
        F64Op => {
            // We can't tell sub-op from `Op` alone. Take the worst-case
            // figure (binary, 64 bits erased) — `program_cost` walks
            // nodes individually so a finer accounting is possible if
            // a follow-up phase needs it.
            Reversibility::Lossy { bits_erased: 64 }
        }
        // ─── KASM v1.0 ────────────────────────────────────────────────
        // Adaptive / Memoize / Comptime are pass-through wrappers — same
        // bits in/out as the wrapped slot. Routing semantics.
        Adaptive | Memoize | Comptime | Lazy => Reversibility::Routing,
        Force => Reversibility::Lossy { bits_erased: 64 },
        // Cond is like SelectI64 — collapses 1+64+64 → 64. Lossy 65.
        Cond => Reversibility::Lossy { bits_erased: 65 },
        // Grad / Vmap / Pmap / Pipeline produce program-hash values.
        // Conservative: treat as 64-bit lossy (one program-hash from
        // one input slot).
        Grad | Vmap | Pmap | Pipeline => Reversibility::Lossy { bits_erased: 64 },
        // Loops and reductions: many bits in, one i64 out. Take the
        // conservative 64 bits erased per call. program_cost walks nodes
        // individually for finer accounting.
        Fori | WhileLoop | Reduce | Scan => Reversibility::Lossy { bits_erased: 64 },
        // Wave 7d — VLenI64 : Vec → i64 length query. Many bits in
        // (whole vec), 64 bits out. Conservative 64 bits erased.
        VLenI64 => Reversibility::Lossy { bits_erased: 64 },
        // Wave 7d-bis + 7e — VSumI64 collapse Vec → i64 ; VAddI64/
        // VMulI64/VSubI64 are pairwise wrapping (information-preserving
        // structurally but lossy bitwise) ; VMaxI64/VMinI64 collapse
        // 2 inputs to 1 (real loss) ; VRangeI64 generates from 1 i64
        // (a fresh vec, conservative 64).
        VSumI64 | VAddI64 | VMulI64 | VSubI64 | VMaxI64 | VMinI64
        | VRangeI64 | VConcatI64 | VReverseI64 | VBroadcastI64
        | VEqI64 | VAndI64 | VOrI64 | VXorI64
        | VAbsI64 | VNegI64 | VBitFlipI64
            => Reversibility::Lossy { bits_erased: 64 },
        // Wave 7i — VGetI64 reads one i64 from a Vec ; many bits in
        // (whole vec + index), 64 bits out. Conservative 64.
        VGetI64 => Reversibility::Lossy { bits_erased: 64 },
        // Wave 8 self-hosting — Fractal/Eval invoquent un sous-programme
        // KASM dont le coût Landauer dépend du callee. Conservative
        // 64 bits erased par invocation locale ; le SelfHostingRuntime
        // calculera le coût récursif réel via program_cost(callee).
        Fractal | Eval => Reversibility::Lossy { bits_erased: 64 },
    }
}

// ---------------------------------------------------------------------------
// Coût d'un programme
// ---------------------------------------------------------------------------

/// Coût Landauer cumulé d'un `Program`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramCost {
    pub total_bits_erased: u64,
    pub op_count: usize,
    pub bijective_ops: usize,
    pub routing_ops: usize,
    pub lossy_ops: usize,
}

impl ProgramCost {
    pub fn zero() -> Self {
        Self {
            total_bits_erased: 0,
            op_count: 0,
            bijective_ops: 0,
            routing_ops: 0,
            lossy_ops: 0,
        }
    }

    /// Énergie Landauer minimale en joules à `t_kelvin`.
    pub fn joules_at(&self, t_kelvin: f64) -> f64 {
        landauer_per_bit_joules(t_kelvin) * (self.total_bits_erased as f64)
    }

    /// Énergie à T = 300K (ambiante).
    pub fn joules_at_300k(&self) -> f64 {
        self.joules_at(DEFAULT_TEMP_K)
    }

    /// Ratio d'ops réversibles (bijective + routing) sur total.
    pub fn reversible_ratio(&self) -> f64 {
        if self.op_count == 0 {
            return 0.0;
        }
        let reversible = self.bijective_ops + self.routing_ops;
        reversible as f64 / self.op_count as f64
    }
}

/// Calcule le coût Landauer d'un programme KASM.
pub fn program_cost(p: &Program) -> ProgramCost {
    let mut cost = ProgramCost::zero();
    cost.op_count = p.nodes().len();

    for node in p.nodes() {
        let rev = op_reversibility(node.op);
        match rev {
            Reversibility::Routing => cost.routing_ops += 1,
            Reversibility::Bijective => cost.bijective_ops += 1,
            Reversibility::Lossy { bits_erased } => {
                cost.lossy_ops += 1;
                // Ajustement pour les Reduce* : count est dans node.imm.
                let bits = match node.op {
                    Op::ReduceAddI64 | Op::ReduceMulI64 => {
                        // count >= 1 par contrainte de validation KASM.
                        let count = node.imm.max(1) as u64;
                        // count * 64 bits in, 64 bits out → (count - 1) * 64.
                        count.saturating_sub(1).saturating_mul(64)
                    }
                    _ => bits_erased as u64,
                };
                cost.total_bits_erased = cost.total_bits_erased.saturating_add(bits);
            }
        }
    }

    cost
}

// ---------------------------------------------------------------------------
// Accumulateur de session — pour le critère "journée MonsterNode"
// ---------------------------------------------------------------------------

/// Accumule les coûts Landauer d'une session SCAN. Une "journée
/// MonsterNode" correspond à une session qui agrège chaque invocation
/// de programme. Le critère Ω-6 demande de pouvoir rapporter la
/// dissipation cumulée en joules.
#[derive(Clone, Copy, Debug, Default)]
pub struct LandauerAccumulator {
    pub total_bits_erased: u64,
    pub total_invocations: u64,
    pub total_op_count: u64,
}

impl LandauerAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enregistre une invocation d'un `Program` et accumule son coût.
    pub fn record_invocation(&mut self, p: &Program) {
        let cost = program_cost(p);
        self.total_bits_erased = self.total_bits_erased.saturating_add(cost.total_bits_erased);
        self.total_invocations = self.total_invocations.saturating_add(1);
        self.total_op_count = self
            .total_op_count
            .saturating_add(cost.op_count as u64);
    }

    /// Enregistre N invocations identiques (utile pour les hot loops).
    pub fn record_invocations_batched(&mut self, p: &Program, n: u64) {
        let cost = program_cost(p);
        self.total_bits_erased = self
            .total_bits_erased
            .saturating_add(cost.total_bits_erased.saturating_mul(n));
        self.total_invocations = self.total_invocations.saturating_add(n);
        self.total_op_count = self
            .total_op_count
            .saturating_add((cost.op_count as u64).saturating_mul(n));
    }

    /// Énergie Landauer cumulée à `t_kelvin`.
    pub fn cumulative_joules_at(&self, t_kelvin: f64) -> f64 {
        landauer_per_bit_joules(t_kelvin) * (self.total_bits_erased as f64)
    }

    pub fn cumulative_joules_at_300k(&self) -> f64 {
        self.cumulative_joules_at(DEFAULT_TEMP_K)
    }

    /// Enregistre un tensor program (Ω-6.3).
    pub fn record_tensor_invocation(&mut self, p: &TensorProgram) {
        let cost = tensor_program_cost(p);
        self.total_bits_erased = self.total_bits_erased.saturating_add(cost.total_bits_erased);
        self.total_invocations = self.total_invocations.saturating_add(1);
        self.total_op_count = self
            .total_op_count
            .saturating_add(cost.op_count as u64);
    }
}

// ---------------------------------------------------------------------------
// Ω-6.3 — Coût Landauer pour KASM-Tensor
// ---------------------------------------------------------------------------

/// Bits par élément selon le dtype tenseur. Source de vérité unique pour
/// le calcul Landauer.
pub fn tensor_dtype_bits(dt: TensorTy) -> u32 {
    match dt {
        TensorTy::F32 => 32,
        TensorTy::Posit16 => 16,
        TensorTy::Posit32 => 32,
        TensorTy::Rational => 256, // i128 num + i128 denom
    }
}

/// Coût Landauer d'un `TensorProgram`. Walk les nodes, applique le
/// modèle d'erasure par opcode + shape + dtype.
///
/// Modèle :
///  * Const, Input, Output : Routing (0 bits).
///  * Add/Mul élément-wise sur N éléments : N × dtype_bits.
///    (2N bits in, N bits out.)
///  * Matmul (M×K) × (K×N) : M×N × (2K-1) × dtype_bits.
///    (M×N output elements, chaque = K mults + (K-1) adds.)
///  * ReduceSumAxis : (input_elems - output_elems) × dtype_bits.
///  * Softmax : ~4N × dtype_bits (max + exp + sum + divide, conservateur).
///  * ReluF32 : N × 1 bit (comparaison signe → choix conditionnel).
///  * TanhF32, SigmoidF32, GeluTanhF32 : N × dtype_bits (non-linéaire,
///    perte conservatrice 1 bit erased par bit en sortie).
pub fn tensor_program_cost(p: &TensorProgram) -> ProgramCost {
    let mut cost = ProgramCost::zero();
    cost.op_count = p.nodes().len();

    let nodes = p.nodes();
    for (i, node) in nodes.iter().enumerate() {
        match node.op {
            TensorOp::Input | TensorOp::Const | TensorOp::Output => {
                cost.routing_ops += 1;
            }
            TensorOp::AddF32
            | TensorOp::MulF32
            | TensorOp::AddRational
            | TensorOp::MulRational
            | TensorOp::AddPosit16
            | TensorOp::MulPosit16
            | TensorOp::AddPosit32
            | TensorOp::MulPosit32 => {
                cost.lossy_ops += 1;
                let n_elems = node.shape.elements() as u64;
                let dt_bits = tensor_dtype_bits(node.dtype) as u64;
                cost.total_bits_erased = cost
                    .total_bits_erased
                    .saturating_add(n_elems.saturating_mul(dt_bits));
            }
            TensorOp::MatmulTile
            | TensorOp::MatmulTileRational
            | TensorOp::MatmulTilePosit16
            | TensorOp::MatmulTilePosit32 => {
                cost.lossy_ops += 1;
                let lhs_shape = nodes[node.a as usize].shape;
                let rhs_shape = nodes[node.b as usize].shape;
                if lhs_shape.dims == 2 && rhs_shape.dims == 2 {
                    let m = lhs_shape.d[0] as u64;
                    let k = lhs_shape.d[1] as u64;
                    let n = rhs_shape.d[1] as u64;
                    let dt_bits = tensor_dtype_bits(node.dtype) as u64;
                    let ops_per_elem = (2 * k).saturating_sub(1);
                    let bits = m
                        .saturating_mul(n)
                        .saturating_mul(ops_per_elem)
                        .saturating_mul(dt_bits);
                    cost.total_bits_erased = cost.total_bits_erased.saturating_add(bits);
                }
            }
            TensorOp::ReduceSumAxis => {
                cost.lossy_ops += 1;
                let src_shape = nodes[node.a as usize].shape;
                let in_elems = src_shape.elements() as u64;
                let out_elems = node.shape.elements() as u64;
                let dt_bits = tensor_dtype_bits(node.dtype) as u64;
                let dropped = in_elems.saturating_sub(out_elems);
                cost.total_bits_erased = cost
                    .total_bits_erased
                    .saturating_add(dropped.saturating_mul(dt_bits));
            }
            TensorOp::Softmax => {
                cost.lossy_ops += 1;
                let n_elems = node.shape.elements() as u64;
                let dt_bits = tensor_dtype_bits(node.dtype) as u64;
                // Conservateur : max + exp + sum + divide ≈ 4N × dtype_bits.
                cost.total_bits_erased = cost
                    .total_bits_erased
                    .saturating_add(n_elems.saturating_mul(4).saturating_mul(dt_bits));
            }
            TensorOp::ReluF32 => {
                cost.lossy_ops += 1;
                let n_elems = node.shape.elements() as u64;
                // ReLU = max(x, 0) : 1 bit erased par élément (sign).
                cost.total_bits_erased = cost.total_bits_erased.saturating_add(n_elems);
            }
            TensorOp::TanhF32 | TensorOp::SigmoidF32 | TensorOp::GeluTanhF32 => {
                cost.lossy_ops += 1;
                let n_elems = node.shape.elements() as u64;
                let dt_bits = tensor_dtype_bits(node.dtype) as u64;
                // Non-linéaires : conservateur dtype_bits par élément.
                cost.total_bits_erased = cost
                    .total_bits_erased
                    .saturating_add(n_elems.saturating_mul(dt_bits));
            }
        }
        let _ = i; // silence unused
    }

    cost
}

// ---------------------------------------------------------------------------
// Ω-6.4 — Connection Ω-5 : Benchmark énergétique pour la Gödel-machine
// ---------------------------------------------------------------------------

/// Benchmark qui retourne le coût Landauer (bits effacés) du programme
/// produit par `MonsterNode::train_i64_program` pour `f(x) = 7x + 3`.
/// Lit `max_nodes` et `beam_width` depuis `SharedConfig` — donc rewrites
/// affectent le score réel.
///
/// Utilisable directement comme `Box<dyn Benchmark>` dans une `CriteriaSuite`
/// de Ω-5. Le verifier accepte les rewrites qui produisent des programmes
/// entraînés avec moins de bits erased.
pub struct LandauerOfTrainedAffineBench {
    pub config: crate::godel::runner::SharedConfig,
}

impl crate::godel::criteria::Benchmark for LandauerOfTrainedAffineBench {
    fn name(&self) -> &str {
        LANDAUER_TRAINED_AFFINE_BENCH_NAME
    }

    fn run(&self, node: &crate::MonsterNode) -> u64 {
        let (max_nodes, beam_width) = {
            let cfg = self.config.borrow();
            (
                cfg.get("max_nodes").unwrap_or(20).max(0) as usize,
                cfg.get("beam_width").unwrap_or(256).max(0) as usize,
            )
        };
        let examples = [(-4i64, -25i64), (-1, -4), (0, 3), (2, 17), (5, 38)];
        let train_cfg = crate::MonsterTrainingConfig { max_nodes, beam_width, progress: None };
        match node.train_i64_program(&examples, train_cfg) {
            Ok(outcome) => program_cost(&outcome.program).total_bits_erased,
            // Si l'entraînement échoue, on retourne une pénalité haute pour
            // que le verifier voie une régression.
            Err(_) => u64::MAX / 4,
        }
    }
}

pub const LANDAUER_TRAINED_AFFINE_BENCH_NAME: &str = "LandauerOfTrainedAffine";

// ---------------------------------------------------------------------------
// Ω-6.5 — Observation passive d'une MonsterNode
// ---------------------------------------------------------------------------

/// Calcule le coût Landauer cumulé des programmes actuellement chargés
/// dans une `MonsterNode`. Lecture seule — utilise `observer::capture` pour
/// récupérer la liste des hashes, puis charge chaque programme depuis
/// le store.
///
/// Ce n'est PAS un compteur de session (pas d'enregistrement par
/// invocation) — c'est une "empreinte énergétique" instantanée.
pub fn loaded_programs_landauer_cost(node: &crate::MonsterNode) -> ProgramCost {
    let frame = crate::godel::observer::capture(node);
    let mut total = ProgramCost::zero();
    for hash in &frame.programs_loaded {
        if let Some(bytes) = node.store().load(hash) {
            if let Ok(program) = Program::from_bytes(&bytes) {
                let cost = program_cost(&program);
                total.total_bits_erased = total
                    .total_bits_erased
                    .saturating_add(cost.total_bits_erased);
                total.op_count = total.op_count.saturating_add(cost.op_count);
                total.bijective_ops = total.bijective_ops.saturating_add(cost.bijective_ops);
                total.routing_ops = total.routing_ops.saturating_add(cost.routing_ops);
                total.lossy_ops = total.lossy_ops.saturating_add(cost.lossy_ops);
            }
        }
    }
    total
}

// ---------------------------------------------------------------------------
// Ω-6.2 — Hardware energy model au-delà de Landauer
// ---------------------------------------------------------------------------

/// Modèle d'énergie hardware. Map les coûts thermodynamiques minimaux
/// (Landauer kT·ln2) aux coûts effectifs sur du hardware réel.
///
/// Sources des constantes (ordres de grandeur typiques industrie 2024) :
/// - CMOS 7nm : énergie de switch par gate ≈ 1e-15 J (~10^5 × Landauer 300K)
/// - CMOS 45nm : ≈ 1e-13 J (~10^7 × Landauer 300K)
/// - Adiabatique : facteur epsilon × Landauer où epsilon dépend de la
///   vitesse de switching (lent → epsilon→1, rapide → epsilon grand).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HardwareEnergyModel {
    /// Borne Landauer théorique. Bijectif = 0, lossy = kT·ln2 par bit.
    IdealLandauer,
    /// CMOS 7nm typique, ~1e-15 J par op (lossy ou bijectif), indépendant de T.
    Cmos7nm,
    /// CMOS 45nm legacy, ~1e-13 J par op.
    Cmos45nm,
    /// Logique adiabatique : epsilon × Landauer (epsilon >= 1).
    /// epsilon = 1.0 → asymptote vers Landauer. epsilon = 10 → 10× le minimum.
    Adiabatic { epsilon: f64 },
}

const CMOS_7NM_JOULES_PER_OP: f64 = 1.0e-15;
const CMOS_45NM_JOULES_PER_OP: f64 = 1.0e-13;

impl HardwareEnergyModel {
    /// Joules dissipés pour effacer un bit (op lossy) à `t_kelvin`.
    pub fn joules_per_lossy_bit(&self, t_kelvin: f64) -> f64 {
        match self {
            HardwareEnergyModel::IdealLandauer => landauer_per_bit_joules(t_kelvin),
            HardwareEnergyModel::Cmos7nm => CMOS_7NM_JOULES_PER_OP,
            HardwareEnergyModel::Cmos45nm => CMOS_45NM_JOULES_PER_OP,
            HardwareEnergyModel::Adiabatic { epsilon } => {
                epsilon * landauer_per_bit_joules(t_kelvin)
            }
        }
    }

    /// Joules dissipés par invocation d'op bijective.
    /// IdealLandauer et Adiabatic : 0 (réversible parfait).
    /// CMOS : énergie de switching résiduelle (les transistors dissipent
    /// quelle que soit la "réversibilité" logique du calcul).
    pub fn joules_per_bijective_op(&self, _t_kelvin: f64) -> f64 {
        match self {
            HardwareEnergyModel::IdealLandauer | HardwareEnergyModel::Adiabatic { .. } => 0.0,
            HardwareEnergyModel::Cmos7nm => CMOS_7NM_JOULES_PER_OP,
            HardwareEnergyModel::Cmos45nm => CMOS_45NM_JOULES_PER_OP,
        }
    }
}

impl ProgramCost {
    /// Coût total en joules sur un modèle hardware donné, à `t_kelvin`.
    pub fn joules_in_model(&self, model: HardwareEnergyModel, t_kelvin: f64) -> f64 {
        let lossy_bits = self.total_bits_erased as f64;
        let bij_ops = self.bijective_ops as f64;
        model.joules_per_lossy_bit(t_kelvin) * lossy_bits
            + model.joules_per_bijective_op(t_kelvin) * bij_ops
    }
}

/// Coût total d'un programme dans un modèle hardware donné.
pub fn program_joules(p: &Program, model: HardwareEnergyModel, t_kelvin: f64) -> f64 {
    program_cost(p).joules_in_model(model, t_kelvin)
}

/// Bench config-driven pour Ω-5 : retourne le coût en *femtojoules* (10^-15 J)
/// du programme entraîné `f(x) = 7x + 3` sur le modèle `Cmos7nm`. Format
/// entier (u64) pour rester compatible avec le trait Benchmark de Codex.
///
/// Pénalité u64::MAX/4 si le training échoue.
pub struct HardwareJoulesBench {
    pub config: crate::godel::runner::SharedConfig,
    pub model: HardwareEnergyModel,
}

impl crate::godel::criteria::Benchmark for HardwareJoulesBench {
    fn name(&self) -> &str {
        HARDWARE_JOULES_BENCH_NAME
    }

    fn run(&self, node: &crate::MonsterNode) -> u64 {
        let (max_nodes, beam_width) = {
            let cfg = self.config.borrow();
            (
                cfg.get("max_nodes").unwrap_or(20).max(0) as usize,
                cfg.get("beam_width").unwrap_or(256).max(0) as usize,
            )
        };
        let examples = [(-4i64, -25i64), (-1, -4), (0, 3), (2, 17), (5, 38)];
        let train_cfg = crate::MonsterTrainingConfig { max_nodes, beam_width, progress: None };
        match node.train_i64_program(&examples, train_cfg) {
            Ok(outcome) => {
                let joules = program_joules(&outcome.program, self.model, DEFAULT_TEMP_K);
                // Convert to femtojoules (1 fJ = 1e-15 J) pour rester en u64.
                let femtojoules = joules / 1.0e-15;
                if femtojoules.is_finite() && femtojoules >= 0.0 {
                    femtojoules.round() as u64
                } else {
                    u64::MAX / 4
                }
            }
            Err(_) => u64::MAX / 4,
        }
    }
}

pub const HARDWARE_JOULES_BENCH_NAME: &str = "HardwareJoules";

// ---------------------------------------------------------------------------
// Ω-6.2.x — Modèle de dissipation dynamique
// ---------------------------------------------------------------------------

/// Profil de dissipation dynamique. Ajoute au `HardwareEnergyModel` les
/// dimensions :
///  - `voltage_scale` : 1.0 = nominal. 0.7 = 49% de l'énergie nominale (V²).
///  - `pue` : Power Usage Effectiveness datacenter. PUE = 1.0 = pas
///    d'overhead. PUE = 1.5 = +50% pour cooling/lights/etc.
///  - `invocations_per_second` : fréquence de switching. 1e9 = 1 GHz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicDissipation {
    pub model: HardwareEnergyModel,
    pub voltage_scale: f64,
    pub pue: f64,
    pub invocations_per_second: f64,
}

impl DynamicDissipation {
    /// Profil baseline : modèle donné, voltage 1.0, PUE 1.0, 1 invocation/s.
    pub fn baseline(model: HardwareEnergyModel) -> Self {
        Self { model, voltage_scale: 1.0, pue: 1.0, invocations_per_second: 1.0 }
    }

    /// Profil typique datacenter cloud 2024 : Cmos7nm, voltage 0.85,
    /// PUE 1.5, 1 GHz.
    pub fn datacenter_cloud_2024() -> Self {
        Self {
            model: HardwareEnergyModel::Cmos7nm,
            voltage_scale: 0.85,
            pue: 1.5,
            invocations_per_second: 1.0e9,
        }
    }

    /// Énergie dissipée par invocation (joules), incluant voltage scaling
    /// et PUE.
    pub fn joules_per_invocation(&self, p: &Program, t_kelvin: f64) -> f64 {
        let base = program_joules(p, self.model, t_kelvin);
        // Voltage scaling : E ∝ V², donc on multiplie par voltage_scale².
        let voltage_factor = self.voltage_scale * self.voltage_scale;
        // PUE multiplie l'énergie consommée pour inclure overhead datacenter.
        base * voltage_factor * self.pue
    }

    /// Puissance moyenne (watts) à la fréquence donnée. P = E × f.
    pub fn average_watts(&self, p: &Program, t_kelvin: f64) -> f64 {
        self.joules_per_invocation(p, t_kelvin) * self.invocations_per_second
    }

    /// Énergie cumulée sur une durée donnée (secondes).
    pub fn cumulative_joules_over(&self, p: &Program, t_kelvin: f64, seconds: f64) -> f64 {
        self.average_watts(p, t_kelvin) * seconds
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Program, Target, Ty};

    fn approx_eq(a: f64, b: f64, rel_tol: f64) -> bool {
        let diff = (a - b).abs();
        let scale = a.abs().max(b.abs()).max(f64::MIN_POSITIVE);
        diff / scale < rel_tol
    }

    // ----- Tagging par opcode -----

    #[test]
    fn routing_ops_have_zero_bits_erased() {
        for op in [Op::Input, Op::ConstI64, Op::Output] {
            let r = op_reversibility(op);
            assert_eq!(r, Reversibility::Routing);
            assert_eq!(r.bits_erased(), 0);
        }
    }

    #[test]
    fn bijective_ops_are_exhaustive() {
        // Ω-6.0 + Ω-6.1 : NotBool + 4 unaires bijectifs i64. Aucune autre
        // op KASM n'est strictement bijective dans la sémantique actuelle.
        for op in [
            Op::NotBool,
            Op::BitFlipI64,
            Op::NegI64,
            Op::ReverseBitsI64,
            Op::ByteswapI64,
        ] {
            assert_eq!(
                op_reversibility(op),
                Reversibility::Bijective,
                "op {op:?} doit être Bijective"
            );
        }
        // Toutes les autres ops ne sont PAS Bijective.
        for op in [
            Op::Input, Op::ConstI64, Op::AddI64, Op::MulI64, Op::EqI64, Op::Hash64,
            Op::Output, Op::SubI64, Op::DivI64Checked, Op::MinI64, Op::MaxI64,
            Op::SelectI64, Op::AndBool, Op::OrBool, Op::LtI64, Op::LeI64,
            Op::BitAndI64, Op::BitOrI64, Op::BitXorI64, Op::ShlI64, Op::ShrI64,
            Op::SatAddI64, Op::SatSubI64, Op::ModI64Checked, Op::ClampI64,
            Op::ReduceAddI64, Op::ReduceMulI64,
        ] {
            assert_ne!(
                op_reversibility(op),
                Reversibility::Bijective,
                "op {op:?} ne doit pas être Bijective"
            );
        }
    }

    #[test]
    fn binary_i64_arithmetic_erases_64_bits() {
        for op in [
            Op::AddI64, Op::SubI64, Op::MulI64, Op::DivI64Checked,
            Op::MinI64, Op::MaxI64, Op::BitAndI64, Op::BitOrI64,
            Op::BitXorI64, Op::ShlI64, Op::ShrI64,
            Op::SatAddI64, Op::SatSubI64, Op::ModI64Checked,
        ] {
            assert_eq!(
                op_reversibility(op),
                Reversibility::Lossy { bits_erased: 64 },
                "op {op:?}",
            );
        }
    }

    #[test]
    fn comparisons_erase_127_bits() {
        for op in [Op::EqI64, Op::LtI64, Op::LeI64] {
            assert_eq!(op_reversibility(op), Reversibility::Lossy { bits_erased: 127 });
        }
    }

    #[test]
    fn bool_binaries_erase_1_bit() {
        for op in [Op::AndBool, Op::OrBool] {
            assert_eq!(op_reversibility(op), Reversibility::Lossy { bits_erased: 1 });
        }
    }

    #[test]
    fn select_erases_65_bits() {
        assert_eq!(op_reversibility(Op::SelectI64), Reversibility::Lossy { bits_erased: 65 });
    }

    #[test]
    fn clamp_erases_128_bits() {
        assert_eq!(op_reversibility(Op::ClampI64), Reversibility::Lossy { bits_erased: 128 });
    }

    // ----- Constantes physiques -----

    #[test]
    fn landauer_per_bit_at_300k_matches_textbook() {
        // Référence : kT·ln2 à 300K ≈ 2.87 × 10⁻²¹ J.
        let v = landauer_per_bit_joules(300.0);
        assert!(approx_eq(v, 2.870e-21, 1e-2), "got {v:e}");
    }

    #[test]
    fn landauer_scales_linearly_with_temperature() {
        let v300 = landauer_per_bit_joules(300.0);
        let v600 = landauer_per_bit_joules(600.0);
        assert!(approx_eq(v600, 2.0 * v300, 1e-12));
    }

    // ----- ProgramCost -----

    fn affine_program() -> Program {
        // f(x) = 7x + 3 : Input + Const(7) + Mul + Const(3) + Add + Output = 6 nodes.
        Program::new(
            Target::Cpu, 1, 1, 16,
            vec![
                Node::input(0),       // Routing
                Node::const_i64(7),   // Routing
                Node::mul(0, 1),      // Lossy 64
                Node::const_i64(3),   // Routing
                Node::add(2, 3),      // Lossy 64
                Node::output(4, Ty::I64), // Routing
            ],
        ).unwrap()
    }

    #[test]
    fn program_cost_counts_categories() {
        let p = affine_program();
        let c = program_cost(&p);
        assert_eq!(c.op_count, 6);
        assert_eq!(c.routing_ops, 4);  // Input + 2 Const + Output
        assert_eq!(c.bijective_ops, 0);
        assert_eq!(c.lossy_ops, 2);    // Mul + Add
        assert_eq!(c.total_bits_erased, 128); // 64 + 64
    }

    #[test]
    fn program_cost_joules_consistent() {
        let p = affine_program();
        let c = program_cost(&p);
        let joules = c.joules_at_300k();
        let expected = 128.0 * landauer_per_bit_joules(300.0);
        assert!(approx_eq(joules, expected, 1e-12));
    }

    #[test]
    fn empty_arithmetic_program_has_zero_lossy_ops() {
        // Programme purement routing : Input + Output (le minimum syntaxique).
        let p = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        let c = program_cost(&p);
        assert_eq!(c.lossy_ops, 0);
        assert_eq!(c.total_bits_erased, 0);
        assert_eq!(c.joules_at_300k(), 0.0);
    }

    #[test]
    fn reversible_ratio_correctness() {
        let p = affine_program();
        let c = program_cost(&p);
        // 4 routing + 0 bijective sur 6 ops total = 4/6 ≈ 0.666.
        assert!(approx_eq(c.reversible_ratio(), 4.0 / 6.0, 1e-12));
    }

    #[test]
    fn reduce_add_cost_scales_with_count() {
        // ReduceAdd avec count = 4 : 4*64 bits in, 64 out, 192 bits erased.
        let p = Program::new(
            Target::Cpu, 0, 1, 16,
            vec![
                Node::const_i64(1),
                Node::const_i64(2),
                Node::const_i64(3),
                Node::const_i64(4),
                Node::reduce_add(0, 4),
                Node::output(4, Ty::I64),
            ],
        ).unwrap();
        let c = program_cost(&p);
        assert_eq!(c.total_bits_erased, 192, "(4-1)*64 = 192");
    }

    #[test]
    fn reduce_with_count_one_has_zero_cost() {
        // Edge case : ReduceAdd count=1 → 1*64 in, 64 out, 0 erased.
        let p = Program::new(
            Target::Cpu, 0, 1, 8,
            vec![
                Node::const_i64(42),
                Node::reduce_add(0, 1),
                Node::output(1, Ty::I64),
            ],
        ).unwrap();
        let c = program_cost(&p);
        assert_eq!(c.total_bits_erased, 0);
    }

    // ----- LandauerAccumulator -----

    #[test]
    fn accumulator_records_single_invocation() {
        let mut acc = LandauerAccumulator::new();
        let p = affine_program();
        acc.record_invocation(&p);
        assert_eq!(acc.total_invocations, 1);
        assert_eq!(acc.total_bits_erased, 128);
        assert_eq!(acc.total_op_count, 6);
    }

    #[test]
    fn accumulator_records_batch_invocations() {
        let mut acc = LandauerAccumulator::new();
        let p = affine_program();
        acc.record_invocations_batched(&p, 1_000_000);
        assert_eq!(acc.total_invocations, 1_000_000);
        assert_eq!(acc.total_bits_erased, 128 * 1_000_000);
    }

    #[test]
    fn accumulator_joules_for_hot_program() {
        // Simulons une journée d'invocations hot : 1 milliard d'appels.
        // Chaque appel = affine_program (128 bits erased).
        let mut acc = LandauerAccumulator::new();
        let p = affine_program();
        acc.record_invocations_batched(&p, 1_000_000_000);

        // 128 × 10⁹ bits = 1.28e11 bits.
        let bits = 128.0e9_f64;
        let expected_joules = bits * landauer_per_bit_joules(300.0);
        let got = acc.cumulative_joules_at_300k();
        assert!(approx_eq(got, expected_joules, 1e-9));
        // Sanity : ~0.37 nanojoules pour un milliard d'invocations affines.
        // C'est minuscule — Landauer est une borne thermodynamique, pas
        // une mesure du hardware actuel qui dissipe 10⁵ × plus.
    }

    #[test]
    fn accumulator_aggregates_distinct_programs() {
        let mut acc = LandauerAccumulator::new();
        let p1 = affine_program();
        // p2 : programme avec un Lt (127 bits erased) et un Output.
        let p2 = Program::new(
            Target::Cpu, 2, 1, 8,
            vec![
                Node::input(0),
                Node::input(1),
                Node::lt(0, 1),
                Node::output(2, Ty::Bool),
            ],
        ).unwrap();
        acc.record_invocation(&p1);
        acc.record_invocation(&p2);
        assert_eq!(acc.total_invocations, 2);
        // p1 = 128 bits, p2 = 127 bits.
        assert_eq!(acc.total_bits_erased, 128 + 127);
    }

    // ----- Cross-cap : programme extrait via Ω-2.0 -----

    #[test]
    fn extracted_programs_are_costable() {
        use crate::extract::extract;
        let p = extract(2, |inputs| (inputs[0] + inputs[1]) * 7).unwrap();
        let c = program_cost(&p);
        // Programme : Input(0) + Input(1) + Const(7) + Add + Mul + Output.
        // Lossy : Add + Mul = 128 bits erased.
        assert_eq!(c.total_bits_erased, 128);
        assert!(c.joules_at_300k() > 0.0);
    }

    #[test]
    fn cost_is_deterministic() {
        let p = affine_program();
        let c1 = program_cost(&p);
        let c2 = program_cost(&p);
        assert_eq!(c1, c2);
    }

    #[test]
    fn cost_invariant_under_canonicalize_for_simple_programs() {
        // Un programme simple sans dead code → canonicalize est l'identité,
        // donc le coût est identique.
        let p = affine_program();
        let c1 = program_cost(&p);
        let canon = p.canonical().unwrap();
        let c2 = program_cost(&canon);
        // Affine_program n'a pas de dead code ni de redondance, donc
        // canonicalize devrait préserver le coût.
        assert_eq!(c1.total_bits_erased, c2.total_bits_erased);
    }

    // ----- Ω-6.3 : Tensor Landauer cost -----

    #[test]
    fn tensor_dtype_bits_match_byte_size_x8() {
        assert_eq!(tensor_dtype_bits(TensorTy::F32), 32);
        assert_eq!(tensor_dtype_bits(TensorTy::Posit16), 16);
        assert_eq!(tensor_dtype_bits(TensorTy::Posit32), 32);
        assert_eq!(tensor_dtype_bits(TensorTy::Rational), 256);
    }

    fn tensor_addf32_program() -> TensorProgram {
        // Const + Input + AddF32 + Output sur shape vec(4) en F32.
        use crate::kasm::tensor::{TensorNode, TensorShape};
        let shape = TensorShape::vec(4).unwrap();
        let pool: Vec<u8> = [1.0f32, 2.0, 3.0, 4.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let nodes = vec![
            TensorNode::const_at(0, pool.len() as u32, TensorTy::F32, shape),
            TensorNode::input(0, TensorTy::F32, shape),
            TensorNode::add(0, 1, TensorTy::F32, shape),
            TensorNode::output(2, TensorTy::F32, shape),
        ];
        TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap()
    }

    #[test]
    fn tensor_program_cost_addf32_4_elements_is_128_bits() {
        // AddF32 sur 4 éléments F32 (32 bits chacun) : 4 × 32 = 128 bits erased.
        let p = tensor_addf32_program();
        let c = tensor_program_cost(&p);
        assert_eq!(c.op_count, 4);
        assert_eq!(c.routing_ops, 3); // Const + Input + Output
        assert_eq!(c.lossy_ops, 1);   // AddF32
        assert_eq!(c.total_bits_erased, 128);
    }

    #[test]
    fn tensor_program_cost_matmul_2x3_3x2_correct() {
        // Matmul (2x3) × (3x2) sur F32 : output 2×2 = 4 elements.
        // Chaque element = 3 mults + 2 adds = 2*3-1 = 5 ops.
        // Total bits = 4 × 5 × 32 = 640 bits.
        use crate::kasm::tensor::{TensorNode, TensorShape};
        let a_vals = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b_vals = [1.0f32, 0.0, 0.0, 1.0, 2.0, 3.0];
        let mut pool: Vec<u8> = a_vals.iter().flat_map(|f| f.to_le_bytes()).collect();
        let b_off = pool.len() as u32;
        pool.extend(b_vals.iter().flat_map(|f| f.to_le_bytes()));
        let a_shape = TensorShape::matrix(2, 3).unwrap();
        let b_shape = TensorShape::matrix(3, 2).unwrap();
        let out_shape = TensorShape::matrix(2, 2).unwrap();
        let nodes = vec![
            TensorNode::const_at(0, (a_vals.len() * 4) as u32, TensorTy::F32, a_shape),
            TensorNode::const_at(b_off, (b_vals.len() * 4) as u32, TensorTy::F32, b_shape),
            TensorNode::matmul(0, 1, TensorTy::F32, out_shape),
            TensorNode::output(2, TensorTy::F32, out_shape),
        ];
        let program = TensorProgram::new(0, 1, nodes.len() as u32, nodes, pool).unwrap();
        let c = tensor_program_cost(&program);
        assert_eq!(c.total_bits_erased, 4 * 5 * 32);
    }

    #[test]
    fn tensor_dtype_costs_scale_with_dtype_bits() {
        // Même structure, dtypes différents : bits erased scale linéairement.
        // Compare F32 (32 bits) vs Posit16 (16 bits) sur add 4-elem.
        use crate::kasm::tensor::{TensorNode, TensorShape};
        let shape = TensorShape::vec(4).unwrap();
        // Posit16 : 4 × 16 = 64 bits.
        let pool_p16: Vec<u8> = [0u16, 0, 0, 0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let nodes = vec![
            TensorNode::const_at(0, pool_p16.len() as u32, TensorTy::Posit16, shape),
            TensorNode::input(0, TensorTy::Posit16, shape),
            TensorNode::add_posit16(0, 1, shape),
            TensorNode::output(2, TensorTy::Posit16, shape),
        ];
        let p = TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool_p16).unwrap();
        let c = tensor_program_cost(&p);
        assert_eq!(c.total_bits_erased, 64);
    }

    // ----- Ω-6.5 : Observation passive d'une MonsterNode -----

    #[test]
    fn loaded_programs_landauer_cost_zero_for_empty_node() {
        use crate::{MemoryGovernor, Store};
        
        fn fresh_path(tag: &str) -> std::path::PathBuf {
            crate::fresh_tmp_path("scan-landauer", tag)
        }
        let node = crate::MonsterNode::new(
            Store::open(fresh_path("empty-landauer")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let cost = loaded_programs_landauer_cost(&node);
        assert_eq!(cost.op_count, 0);
        assert_eq!(cost.total_bits_erased, 0);
    }

    // ----- Ω-6.4 : Bench Landauer config-driven (intégration Ω-5) -----

    #[test]
    fn landauer_of_trained_affine_bench_runs_and_returns_finite() {
        use crate::godel::criteria::Benchmark;
        use crate::godel::applicator::GodelMutableConfig;
        use crate::godel::runner::shared_config;
        use crate::{MemoryGovernor, Store};
        
        fn fresh_path(tag: &str) -> std::path::PathBuf {
            crate::fresh_tmp_path("scan-landauer", tag)
        }
        let node = crate::MonsterNode::new(
            Store::open(fresh_path("trained-affine-landauer")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        cfg.borrow_mut().set("max_nodes", 20);
        cfg.borrow_mut().set("beam_width", 256);
        let bench = LandauerOfTrainedAffineBench {
            config: std::rc::Rc::clone(&cfg),
        };
        let score = bench.run(&node);
        assert!(score > 0, "trained affine doit avoir un coût Landauer non-nul");
        assert!(score < u64::MAX / 4, "training réussi → pas de pénalité");
    }

    #[test]
    fn canonicalize_can_reduce_cost_when_dead_code_eliminated() {
        // Programme avec une op morte : output ignore le résultat de Mul.
        let p = Program::new(
            Target::Cpu, 1, 1, 16,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::mul(0, 1),     // Cette Mul est dead — output n'utilise pas.
                Node::output(0, Ty::I64),
            ],
        ).unwrap();
        let c_before = program_cost(&p);
        let canon = p.canonical().unwrap();
        let c_after = program_cost(&canon);
        // Le canonicalize doit éliminer la Mul morte.
        assert!(c_after.total_bits_erased < c_before.total_bits_erased);
    }

    // ----- Ω-6.2 : Hardware energy models -----

    #[test]
    fn ideal_landauer_matches_existing_landauer() {
        let model = HardwareEnergyModel::IdealLandauer;
        let v = model.joules_per_lossy_bit(300.0);
        assert!(approx_eq(v, landauer_per_bit_joules(300.0), 1e-12));
    }

    #[test]
    fn cmos_7nm_about_5_orders_above_landauer_at_300k() {
        let cmos = HardwareEnergyModel::Cmos7nm.joules_per_lossy_bit(300.0);
        let ideal = HardwareEnergyModel::IdealLandauer.joules_per_lossy_bit(300.0);
        let ratio = cmos / ideal;
        // Attendu : ratio ~ 1e-15 / 2.87e-21 ~ 3.5e5.
        assert!(ratio > 1.0e5 && ratio < 1.0e7, "ratio={ratio:e}");
    }

    #[test]
    fn cmos_45nm_higher_than_cmos_7nm() {
        let m45 = HardwareEnergyModel::Cmos45nm.joules_per_lossy_bit(300.0);
        let m7 = HardwareEnergyModel::Cmos7nm.joules_per_lossy_bit(300.0);
        assert!(m45 > m7);
    }

    #[test]
    fn adiabatic_scales_linearly_with_epsilon() {
        let m1 = HardwareEnergyModel::Adiabatic { epsilon: 1.0 }.joules_per_lossy_bit(300.0);
        let m5 = HardwareEnergyModel::Adiabatic { epsilon: 5.0 }.joules_per_lossy_bit(300.0);
        assert!(approx_eq(m5, 5.0 * m1, 1e-12));
    }

    #[test]
    fn adiabatic_with_epsilon_one_equals_ideal_landauer() {
        let adia = HardwareEnergyModel::Adiabatic { epsilon: 1.0 }.joules_per_lossy_bit(300.0);
        let ideal = HardwareEnergyModel::IdealLandauer.joules_per_lossy_bit(300.0);
        assert!(approx_eq(adia, ideal, 1e-12));
    }

    #[test]
    fn ideal_landauer_zero_for_bijective_ops() {
        let m = HardwareEnergyModel::IdealLandauer;
        assert_eq!(m.joules_per_bijective_op(300.0), 0.0);
    }

    #[test]
    fn adiabatic_zero_for_bijective_ops() {
        let m = HardwareEnergyModel::Adiabatic { epsilon: 100.0 };
        assert_eq!(m.joules_per_bijective_op(300.0), 0.0);
    }

    #[test]
    fn cmos_nonzero_for_bijective_ops() {
        // Sur CMOS, même les ops bijectives coutent (energy de switching transistor).
        let m = HardwareEnergyModel::Cmos7nm;
        assert!(m.joules_per_bijective_op(300.0) > 0.0);
    }

    #[test]
    fn program_joules_affine_in_cmos_7nm() {
        let p = affine_program();
        let j = program_joules(&p, HardwareEnergyModel::Cmos7nm, 300.0);
        // affine_program a 128 bits erased + 0 ops bijectives.
        let expected = 128.0 * CMOS_7NM_JOULES_PER_OP;
        assert!(approx_eq(j, expected, 1e-12));
    }

    #[test]
    fn program_joules_distinguishes_bijective_in_cmos_vs_landauer() {
        use crate::kasm::{Node, Target, Ty};
        // Programme avec une op bijective bit_flip.
        let p = crate::kasm::Program::new(
            Target::Cpu, 1, 1, 8,
            vec![
                Node::input(0),
                Node::bit_flip(0),
                Node::output(1, Ty::I64),
            ],
        ).unwrap();
        let j_landauer = program_joules(&p, HardwareEnergyModel::IdealLandauer, 300.0);
        let j_cmos = program_joules(&p, HardwareEnergyModel::Cmos7nm, 300.0);
        // Landauer : bit_flip = bijectif = 0 J. Le programme ne fait que routing
        // et bijection donc 0 bits erased, 1 bijective op.
        assert_eq!(j_landauer, 0.0);
        // CMOS : bit_flip dissipe quand même. > 0.
        assert!(j_cmos > 0.0);
    }

    #[test]
    fn hardware_joules_bench_runs_finite() {
        use crate::godel::criteria::Benchmark;
        use crate::godel::applicator::GodelMutableConfig;
        use crate::godel::runner::shared_config;
        use crate::{MemoryGovernor, Store};
        
        fn fresh_path(tag: &str) -> std::path::PathBuf {
            crate::fresh_tmp_path("scan-hwbench", tag)
        }
        let node = crate::MonsterNode::new(
            Store::open(fresh_path("hw")).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        let cfg = shared_config(GodelMutableConfig::with_defaults());
        cfg.borrow_mut().set("max_nodes", 20);
        cfg.borrow_mut().set("beam_width", 256);
        let bench = HardwareJoulesBench {
            config: std::rc::Rc::clone(&cfg),
            model: HardwareEnergyModel::Cmos7nm,
        };
        let score = bench.run(&node);
        assert!(score > 0, "bench doit retourner score > 0 sur entraînement réussi");
        assert!(score < u64::MAX / 4, "score ne doit pas être pénalité");
    }

    // ----- Ω-6.2.x : Dissipation dynamique -----

    fn small_lossy_program() -> Program {
        // 2 ops lossy = 128 bits erased.
        affine_program()
    }

    #[test]
    fn baseline_dissipation_equals_static_program_joules() {
        let p = small_lossy_program();
        let d = DynamicDissipation::baseline(HardwareEnergyModel::Cmos7nm);
        let dyn_j = d.joules_per_invocation(&p, 300.0);
        let stat_j = program_joules(&p, HardwareEnergyModel::Cmos7nm, 300.0);
        assert!(approx_eq(dyn_j, stat_j, 1e-12));
    }

    #[test]
    fn voltage_scale_reduces_energy_quadratically() {
        let p = small_lossy_program();
        let nominal = DynamicDissipation::baseline(HardwareEnergyModel::Cmos7nm);
        let mut undervolted = nominal;
        undervolted.voltage_scale = 0.5;
        // 0.5² = 0.25, donc undervolted = nominal * 0.25.
        let n = nominal.joules_per_invocation(&p, 300.0);
        let u = undervolted.joules_per_invocation(&p, 300.0);
        assert!(approx_eq(u, 0.25 * n, 1e-9));
    }

    #[test]
    fn pue_overhead_multiplies_energy_linearly() {
        let p = small_lossy_program();
        let mut d1 = DynamicDissipation::baseline(HardwareEnergyModel::Cmos7nm);
        d1.pue = 1.0;
        let mut d2 = d1;
        d2.pue = 1.5;
        let e1 = d1.joules_per_invocation(&p, 300.0);
        let e2 = d2.joules_per_invocation(&p, 300.0);
        assert!(approx_eq(e2, 1.5 * e1, 1e-9));
    }

    #[test]
    fn average_watts_scales_with_invocations_per_second() {
        let p = small_lossy_program();
        let mut d = DynamicDissipation::baseline(HardwareEnergyModel::Cmos7nm);
        d.invocations_per_second = 1.0e9;
        let watts = d.average_watts(&p, 300.0);
        let energy_per_call = d.joules_per_invocation(&p, 300.0);
        assert!(approx_eq(watts, energy_per_call * 1.0e9, 1e-3));
    }

    #[test]
    fn datacenter_cloud_2024_profile_distinct_from_baseline() {
        let p = small_lossy_program();
        let baseline = DynamicDissipation::baseline(HardwareEnergyModel::Cmos7nm);
        let cloud = DynamicDissipation::datacenter_cloud_2024();
        let e_baseline = baseline.joules_per_invocation(&p, 300.0);
        let e_cloud = cloud.joules_per_invocation(&p, 300.0);
        // Cloud a voltage 0.85 (E×0.7225) et PUE 1.5, donc ratio ~1.084.
        let expected_ratio = 0.85 * 0.85 * 1.5;
        let actual_ratio = e_cloud / e_baseline;
        assert!(approx_eq(actual_ratio, expected_ratio, 1e-9));
    }

    #[test]
    fn cumulative_over_time_scales_linearly() {
        let p = small_lossy_program();
        let d = DynamicDissipation::datacenter_cloud_2024();
        let one_sec = d.cumulative_joules_over(&p, 300.0, 1.0);
        let ten_sec = d.cumulative_joules_over(&p, 300.0, 10.0);
        assert!(approx_eq(ten_sec, 10.0 * one_sec, 1e-9));
    }

    #[test]
    fn ideal_landauer_bijective_zero_remains_zero_under_dynamic() {
        // Programme avec 0 lossy ops (juste routing).
        let p = Program::new(
            Target::Cpu, 1, 1, 4,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        ).unwrap();
        let d = DynamicDissipation::baseline(HardwareEnergyModel::IdealLandauer);
        let e = d.joules_per_invocation(&p, 300.0);
        assert_eq!(e, 0.0);
    }
}
