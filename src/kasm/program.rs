//! KASM Program: serialised form, verification, and the public Program API.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::optimizer::{
    canonicalize, cse, semantic_fingerprint, simplify, static_output,
};
use super::types::{
    KasmError, Node, Op, PartialEvalReport, ProgramSig, RewriteReport, Target, Ty, FOOTER_LEN,
    HEADER_LEN, MAGIC, MAX_NODES, MAX_SLOTS, NODE_LEN, VERSION,
};

#[derive(Clone, Debug)]
pub struct Program {
    bytes: Vec<u8>,
    nodes: Vec<Node>,
    target: Target,
    inputs: u8,
    outputs: u8,
    fuel: u32,
}

static PROGRAM_BUILD_CACHE: OnceLock<Mutex<HashMap<[u8; 32], Program>>> = OnceLock::new();
const PROGRAM_BUILD_CACHE_MAX: usize = 4096;

impl Program {
    pub fn new(
        target: Target,
        inputs: u8,
        outputs: u8,
        fuel: u32,
        nodes: Vec<Node>,
    ) -> Result<Self, KasmError> {
        if inputs > MAX_SLOTS || outputs > MAX_SLOTS {
            return Err(KasmError::TooManySlots);
        }
        if nodes.is_empty() || nodes.len() > MAX_NODES {
            return Err(KasmError::BadNodeCount(nodes.len()));
        }
        if fuel < nodes.len() as u32 {
            return Err(KasmError::FuelTooSmall);
        }

        let cache_key = program_build_cache_key(target, inputs, outputs, fuel, &nodes);
        if let Some(program) = program_build_cache()
            .lock()
            .expect("program build cache poisoned")
            .get(&cache_key)
            .cloned()
        {
            return Ok(program);
        }

        let mut bytes = Vec::with_capacity(HEADER_LEN + nodes.len() * NODE_LEN + FOOTER_LEN);
        bytes.extend_from_slice(MAGIC);
        bytes.push(VERSION);
        bytes.push(target as u8);
        bytes.push(inputs);
        bytes.push(outputs);
        bytes.extend_from_slice(&fuel.to_le_bytes());
        bytes.extend_from_slice(&(nodes.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&[0u8; HEADER_LEN - 14]);
        for node in &nodes {
            node.encode(&mut bytes);
        }
        let footer = digest(&bytes);
        bytes.extend_from_slice(&footer);
        let program = verify(&bytes)?;
        let mut cache = program_build_cache()
            .lock()
            .expect("program build cache poisoned");
        if cache.len() >= PROGRAM_BUILD_CACHE_MAX {
            cache.clear();
        }
        cache.insert(cache_key, program.clone());
        Ok(program)
    }

    pub(crate) fn from_parts(
        bytes: Vec<u8>,
        nodes: Vec<Node>,
        target: Target,
        inputs: u8,
        outputs: u8,
        fuel: u32,
    ) -> Self {
        Self { bytes, nodes, target, inputs, outputs, fuel }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Σ.18 (Wave 14) — hot accessor inlined toujours pour permettre
    /// au compilateur d'éliminer l'indirection sur le slow lane interpreter.
    #[inline(always)]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn target(&self) -> Target {
        self.target
    }

    /// Σ.18 (Wave 14) — hot accessor inlined.
    #[inline(always)]
    pub fn inputs(&self) -> u8 {
        self.inputs
    }

    /// Σ.18 (Wave 14) — hot accessor inlined.
    #[inline(always)]
    pub fn outputs(&self) -> u8 {
        self.outputs
    }

    pub fn input_types(&self) -> Vec<Ty> {
        // Φ.0 — Input nodes carry their declared type on the node
        // itself. Recover the per-slot input type by scanning the
        // first appearance of each `Op::Input` slot. Unused slots
        // default to I64 (no observable difference: the wire format
        // is byte-identical for I64 and F64).
        let mut types = vec![Ty::I64; self.inputs as usize];
        let mut seen = vec![false; self.inputs as usize];
        for node in &self.nodes {
            if node.op == Op::Input {
                let slot = node.imm as usize;
                if slot < types.len() && !seen[slot] {
                    types[slot] = node.ty;
                    seen[slot] = true;
                }
            }
        }
        types
    }

    pub fn output_types(&self) -> Vec<Ty> {
        self.output_sources().into_iter().map(|(_, ty)| ty).collect()
    }

    /// Wave 4 (Phase Ω.10) — produce the program's type signature for
    /// `MultiMethod` lookup. Equivalent to
    /// `ProgramSig { inputs: self.input_types(), outputs: self.output_types() }`
    /// but allocates the two vectors in one place so call sites stay
    /// brief.
    pub fn sig(&self) -> ProgramSig {
        ProgramSig {
            inputs: self.input_types(),
            outputs: self.output_types(),
        }
    }

    pub fn fuel(&self) -> u32 {
        self.fuel
    }

    pub fn structural_hash_hex(&self) -> String {
        hex(&digest(&self.bytes[..self.bytes.len() - FOOTER_LEN]))
    }

    pub fn canonical(&self) -> Result<Self, KasmError> {
        canonicalize(self)
    }

    pub fn simplified(&self) -> Result<Self, KasmError> {
        self.rewrite_fixpoint().map(|(program, _)| program)
    }

    pub fn rewrite_fixpoint(&self) -> Result<(Self, RewriteReport), KasmError> {
        let mut current = self.canonical()?;
        let mut passes = 0usize;
        loop {
            passes += 1;
            let next = simplify(&current)?;
            if next.bytes() == current.bytes() {
                let reduced_to_constant = next.static_output().is_some();
                return Ok((
                    next,
                    RewriteReport {
                        passes,
                        residual_nodes: current.nodes().len(),
                        reduced_to_constant,
                    },
                ));
            }
            current = next;
        }
    }

    pub fn rewrite_report(&self) -> Result<RewriteReport, KasmError> {
        self.rewrite_fixpoint().map(|(_, report)| report)
    }

    pub fn partial_evaluate(&self) -> Result<(Self, PartialEvalReport), KasmError> {
        let residual = simplify(self)?;
        let report = PartialEvalReport::from_programs(
            self.nodes().len(),
            residual.nodes().len(),
            residual.static_output().is_some(),
        );
        Ok((residual, report))
    }

    pub fn partial_eval_report(&self) -> Result<PartialEvalReport, KasmError> {
        self.partial_evaluate().map(|(_, report)| report)
    }

    pub fn canonical_hash_hex(&self) -> Result<String, KasmError> {
        Ok(self.canonical()?.structural_hash_hex())
    }

    /// Semantic CSE: simplify + merge subexpressions that evaluate
    /// identically on deterministic sample inputs, even when their
    /// structure differs (`Shl(x,1)` ≡ `Add(x,x)` ≡ `Mul(x,2)`).
    pub fn cse(&self) -> Result<Self, KasmError> {
        cse(self)
    }

    pub fn semantic_fingerprint(&self) -> Result<[u8; 32], KasmError> {
        semantic_fingerprint(self)
    }

    pub fn semantic_fingerprint_hex(&self) -> Result<String, KasmError> {
        Ok(hex(&self.semantic_fingerprint()?))
    }

    pub fn static_output(&self) -> Option<Vec<u8>> {
        static_output(self)
    }

    pub(crate) fn output_sources(&self) -> Vec<(u16, Ty)> {
        self.nodes
            .iter()
            .filter(|node| node.op == Op::Output)
            .map(|node| (node.a, node.ty))
            .collect()
    }

    pub(crate) fn memoize_subprograms(&self) -> Result<Vec<Program>, KasmError> {
        let mut out = Vec::new();
        for (index, node) in self.nodes.iter().copied().enumerate() {
            if node.op == Op::Memoize {
                out.push(self.extract_output_subprogram(index as u16, node.ty)?);
            }
        }
        Ok(out)
    }

    pub(crate) fn extract_output_subprogram(
        &self,
        output_ref: u16,
        output_ty: Ty,
    ) -> Result<Program, KasmError> {
        let mut keep = vec![false; self.nodes.len()];
        mark_dependencies(self, output_ref as usize, &mut keep)?;

        let mut remap: Vec<Option<u16>> = vec![None; self.nodes.len()];
        let mut nodes = Vec::new();

        for (old_index, old_node) in self.nodes.iter().copied().enumerate() {
            if !keep[old_index] {
                continue;
            }
            let new_node = remap_node(old_index, old_node, &remap)?;
            let new_index =
                u16::try_from(nodes.len()).map_err(|_| KasmError::BadNodeCount(nodes.len()))?;
            remap[old_index] = Some(new_index);
            nodes.push(new_node);
        }

        let output_ref = remap
            .get(output_ref as usize)
            .and_then(|slot| *slot)
            .ok_or(KasmError::BadRef {
                node: self.nodes.len(),
                reference: output_ref,
            })?;
        nodes.push(Node::output(output_ref, output_ty));

        Program::new(self.target(), self.inputs(), 1, nodes.len() as u32, nodes)
    }

    /// Charge un programme depuis sa forme MLIR text canonique.
    ///
    /// **Entrée canonique officielle Ω-1** pour la forme texte. Équivalent
    /// à `kasm::parse_mlir(text)`.
    pub fn from_mlir(text: &str) -> Result<Self, super::mlir::MlirError> {
        super::mlir::parse_mlir(text)
    }

    /// Charge un programme depuis sa forme bytes (wire format).
    ///
    /// **Entrée canonique officielle Ω-1** pour la forme binaire. Le legacy
    /// `kasm::verify(bytes)` est rendu `pub(crate)` (Ω-1.0 critère #4) :
    /// les call sites externes doivent passer par `from_bytes` (forme
    /// binaire) ou `from_mlir` (forme texte).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, KasmError> {
        verify(bytes)
    }

    /// Émet le programme dans sa forme MLIR text canonique
    /// (`canonical_mlir_text(P) = emit_mlir(canonicalize(P))`).
    pub fn canonical_mlir_text(&self) -> Result<String, KasmError> {
        super::mlir::canonical_mlir_text(self)
    }

    /// Hash MLIR-canonique du programme, version texte hexadécimale.
    pub fn hash_mlir_canonical_hex(&self) -> Result<String, KasmError> {
        super::mlir::hash_mlir_canonical_hex(self)
    }
}

fn program_build_cache() -> &'static Mutex<HashMap<[u8; 32], Program>> {
    PROGRAM_BUILD_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn program_build_cache_key(
    target: Target,
    inputs: u8,
    outputs: u8,
    fuel: u32,
    nodes: &[Node],
) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(HEADER_LEN + nodes.len() * NODE_LEN);
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.push(target as u8);
    bytes.push(inputs);
    bytes.push(outputs);
    bytes.extend_from_slice(&fuel.to_le_bytes());
    bytes.extend_from_slice(&(nodes.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&[0u8; HEADER_LEN - 14]);
    for node in nodes {
        node.encode(&mut bytes);
    }
    digest(&bytes)
}

pub fn verify(bytes: &[u8]) -> Result<Program, KasmError> {
    if bytes.len() < HEADER_LEN + NODE_LEN + FOOTER_LEN {
        return Err(KasmError::BadLength);
    }
    if &bytes[..4] != MAGIC {
        return Err(KasmError::BadMagic);
    }
    if bytes[4] != VERSION {
        return Err(KasmError::BadVersion(bytes[4]));
    }

    let target = Target::from_byte(bytes[5])?;
    let inputs = bytes[6];
    let outputs = bytes[7];
    if inputs > MAX_SLOTS || outputs > MAX_SLOTS {
        return Err(KasmError::TooManySlots);
    }
    let fuel = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let node_count = u16::from_le_bytes(bytes[12..14].try_into().unwrap()) as usize;
    if node_count == 0 || node_count > MAX_NODES {
        return Err(KasmError::BadNodeCount(node_count));
    }
    if fuel < node_count as u32 {
        return Err(KasmError::FuelTooSmall);
    }

    let expected_len = HEADER_LEN + node_count * NODE_LEN + FOOTER_LEN;
    if bytes.len() != expected_len {
        return Err(KasmError::BadLength);
    }
    let footer_start = bytes.len() - FOOTER_LEN;
    if digest(&bytes[..footer_start]) != bytes[footer_start..] {
        return Err(KasmError::BadFooter);
    }

    let mut nodes = Vec::with_capacity(node_count);
    let mut types = Vec::with_capacity(node_count);
    let mut output_count = 0u8;
    for i in 0..node_count {
        let start = HEADER_LEN + i * NODE_LEN;
        let node = Node::decode(&bytes[start..start + NODE_LEN])?;
        verify_node(i, node, inputs, &types)?;
        if node.op == Op::Output {
            output_count = output_count.saturating_add(1);
        }
        types.push(node_result_type(node));
        nodes.push(node);
    }
    if output_count != outputs {
        return Err(KasmError::OutputCount { expected: outputs, got: output_count });
    }

    Ok(Program::from_parts(bytes.to_vec(), nodes, target, inputs, outputs, fuel))
}

pub(super) fn verify_node(index: usize, node: Node, inputs: u8, types: &[Ty]) -> Result<(), KasmError> {
    match node.op {
        Op::Input => {
            if node.imm < 0 || node.imm as u8 >= inputs {
                return Err(KasmError::BadInputSlot { node: index, slot: node.imm });
            }
            // Φ.0 — Input may now be I64 or F64. Bool inputs are
            // explicitly rejected (no use case + would require a
            // 1-byte-per-slot calling convention).
            // Wave 7b — Ty::VecI64 inputs accepted at the verifier
            // level. Wire format is `[u32 LE count | count × 8 bytes]`,
            // parsed dynamically by `kasm::execute()`.
            match node.ty {
                Ty::I64 | Ty::F64 | Ty::VecI64 => {}
                Ty::Bool => return Err(KasmError::TypeMismatch { node: index }),
            }
        }
        Op::ConstI64 => ensure_ty(index, node.ty, Ty::I64)?,
        Op::ConstF64 => ensure_ty(index, node.ty, Ty::F64)?,
        Op::F64Op => {
            let sub = super::types::F64SubOp::from_imm(node.imm)?;
            ensure_ty(index, node.ty, sub.result_ty())?;
            expect_ref(index, node.a, sub.a_ty(), types)?;
            if let Some(b_ty) = sub.b_ty() {
                expect_ref(index, node.b, b_ty, types)?;
            } else if node.b != 0 {
                // Unary sub-ops: `b` is reserved and must stay zero so
                // canonicalisation and content-addressing remain
                // deterministic across encoders.
                return Err(KasmError::BadRef { node: index, reference: node.b });
            }
        }
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::EqI64 | Op::LtI64 | Op::LeI64 => {
            ensure_ty(index, node.ty, Ty::Bool)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::Hash64
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64 => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
        }
        Op::PextI64 | Op::PdepI64 => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::Lazy => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            if node.b != 0 || node.imm != 0 {
                return Err(KasmError::BadRef { node: index, reference: node.b });
            }
        }
        Op::Force => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            if node.b != 0 || node.imm != 0 {
                return Err(KasmError::BadRef { node: index, reference: node.b });
            }
        }
        Op::Output => {
            // Wave 7b deployment — expect_ref now accepts Ty::VecI64
            // uniformly, so no Vec-specific branch needed. The check
            // collapses back to one line.
            expect_ref(index, node.a, node.ty, types)?;
        }
        Op::SelectI64 => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::Bool, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
            expect_ref(index, checked_imm_ref(node.imm, index)?, Ty::I64, types)?;
        }
        Op::ClampI64 => {
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
            expect_ref(index, checked_imm_ref(node.imm, index)?, Ty::I64, types)?;
        }
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            ensure_ty(index, node.ty, Ty::I64)?;
            if node.imm <= 0 {
                return Err(KasmError::BadReduceCount { node: index, count: node.imm });
            }
            let count = node.imm as usize;
            let base = node.a as usize;
            let end = base.checked_add(count).ok_or(KasmError::BadReduceCount {
                node: index,
                count: node.imm,
            })?;
            if end > types.len() {
                return Err(KasmError::BadReduceCount { node: index, count: node.imm });
            }
            for offset in 0..count {
                let r = (base + offset) as u16;
                expect_ref(index, r, Ty::I64, types)?;
            }
        }
        Op::AndBool | Op::OrBool => {
            ensure_ty(index, node.ty, Ty::Bool)?;
            expect_ref(index, node.a, Ty::Bool, types)?;
            expect_ref(index, node.b, Ty::Bool, types)?;
        }
        Op::NotBool => {
            ensure_ty(index, node.ty, Ty::Bool)?;
            expect_ref(index, node.a, Ty::Bool, types)?;
        }
        // ─── KASM v1.0 mutation — verifier acceptance ───────────────────
        // The verifier validates structural shape; runtime semantics live
        // in the interpreter / specialised backends. Each v1.0 op is
        // typed I64 by default; sub-ops that produce different types
        // (e.g. Op::Vmap producing a program-hash) carry the type via
        // `ty` directly.
        Op::Adaptive | Op::Memoize | Op::Comptime => {
            // Pass-through wrappers: ty = referenced slot's ty.
            expect_ref(index, node.a, node.ty, types)?;
        }
        Op::Grad => {
            // Symbolic derivative — produces a new program hash (I64).
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
        }
        Op::Cond => {
            // pred:Bool, then/else:I64 → I64
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::Bool, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
            expect_ref(index, checked_imm_ref(node.imm, index)?, Ty::I64, types)?;
        }
        Op::Pipeline => {
            // Two program-hash slots compose into one program-hash.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::Vmap | Op::Pmap => {
            // Meta-op : input is a program-hash (I64), output is a
            // program-hash (I64) for the vectorised version.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
        }
        Op::Fori | Op::WhileLoop | Op::Reduce | Op::Scan => {
            // Loop / reduction families — inputs are program-hash + state
            // slots (all I64). Detailed shape depends on the op but at
            // verifier level we only require I64 references.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            if node.b != 0 {
                expect_ref(index, node.b, Ty::I64, types)?;
            }
        }
        Op::VLenI64 => {
            // Wave 7d — Vec length query : input Ty::VecI64, output Ty::I64.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
        }
        Op::VSumI64 => {
            // Wave 7d-bis — Vec sum : input Ty::VecI64, output Ty::I64.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
        }
        Op::VAddI64 | Op::VMulI64 | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64
        | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64 => {
            // Wave 7d-bis + 7e + 7g — Vec element-wise binary : (Vec, Vec) → Vec.
            // Length matching is checked at runtime (verifier can't know
            // dynamic lengths from static node refs).
            ensure_ty(index, node.ty, Ty::VecI64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
            expect_ref(index, node.b, Ty::VecI64, types)?;
        }
        Op::VRangeI64 => {
            // Wave 7e — Vec range : input Ty::I64 (length), output Ty::VecI64.
            ensure_ty(index, node.ty, Ty::VecI64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
        }
        Op::VConcatI64 => {
            // Wave 7f — concatenation : (Vec, Vec) → Vec, output ty
            // matches inputs (lengths can differ — that's the point).
            ensure_ty(index, node.ty, Ty::VecI64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
            expect_ref(index, node.b, Ty::VecI64, types)?;
        }
        Op::VReverseI64 | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 => {
            // Wave 7f + 7h — Vec unary : Vec → Vec.
            ensure_ty(index, node.ty, Ty::VecI64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
        }
        Op::VBroadcastI64 => {
            // Wave 7f — broadcast/fill : (i64 value, i64 length) → Vec.
            ensure_ty(index, node.ty, Ty::VecI64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::Fractal | Op::Eval => {
            // Wave 8 self-hosting — runtime-only ops. Le verifier
            // accepte (a, b) comme refs i64 opaques (hash slot + args
            // slot pour Fractal, prog_bytes slot + args slot pour Eval).
            // La validation profonde (programme valide, hash existe)
            // est deferred au SelfHostingRuntime.
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::I64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
        Op::VGetI64 => {
            // Wave 7i — Vec random-access read : (Vec, i64 index) → i64.
            // Bounds handling is runtime (modulo len for total function).
            ensure_ty(index, node.ty, Ty::I64)?;
            expect_ref(index, node.a, Ty::VecI64, types)?;
            expect_ref(index, node.b, Ty::I64, types)?;
        }
    }
    Ok(())
}

fn mark_dependencies(program: &Program, index: usize, keep: &mut [bool]) -> Result<(), KasmError> {
    if index >= program.nodes.len() {
        return Err(KasmError::BadRef {
            node: program.nodes.len(),
            reference: index as u16,
        });
    }
    if keep[index] {
        return Ok(());
    }
    keep[index] = true;

    let node = program.nodes[index];
    match node.op {
        Op::Input | Op::ConstI64 | Op::ConstF64 => {}
        Op::F64Op => {
            let sub = super::types::F64SubOp::from_imm(node.imm)?;
            mark_dependencies(program, node.a as usize, keep)?;
            if sub.is_binary() {
                mark_dependencies(program, node.b as usize, keep)?;
            }
        }
        Op::Hash64
        | Op::NotBool
        | Op::Output
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64
        | Op::Lazy
        | Op::Force
        | Op::Adaptive
        | Op::Comptime
        | Op::Memoize
        | Op::Grad
        | Op::Vmap
        | Op::Pmap
        | Op::VLenI64
        | Op::VSumI64
        | Op::VRangeI64
        | Op::VReverseI64
        | Op::VAbsI64
        | Op::VNegI64
        | Op::VBitFlipI64 => {
            mark_dependencies(program, node.a as usize, keep)?;
        }
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64
        | Op::LtI64
        | Op::LeI64
        | Op::Pipeline
        | Op::Fori
        | Op::WhileLoop
        | Op::Reduce
        | Op::Scan
        | Op::VAddI64
        | Op::VMulI64
        | Op::VSubI64
        | Op::VMaxI64
        | Op::VMinI64
        | Op::VConcatI64
        | Op::VBroadcastI64
        | Op::VEqI64
        | Op::VAndI64
        | Op::VOrI64
        | Op::VXorI64
        | Op::VGetI64  // Wave 7i — (vec_slot, idx_slot)
        | Op::Fractal  // Wave 8 — (callee_hash_slot, args_slot)
        | Op::Eval => {  // Wave 8 — (prog_bytes_slot, args_slot)
            mark_dependencies(program, node.a as usize, keep)?;
            mark_dependencies(program, node.b as usize, keep)?;
        }
        Op::SelectI64 | Op::ClampI64 | Op::Cond => {
            mark_dependencies(program, node.a as usize, keep)?;
            mark_dependencies(program, node.b as usize, keep)?;
            mark_dependencies(program, checked_imm_ref(node.imm, index)? as usize, keep)?;
        }
        Op::ReduceAddI64 | Op::ReduceMulI64 => {
            if node.imm <= 0 {
                return Err(KasmError::BadReduceCount {
                    node: index,
                    count: node.imm,
                });
            }
            let count = node.imm as usize;
            let base = node.a as usize;
            let end = base.checked_add(count).ok_or(KasmError::BadReduceCount {
                node: index,
                count: node.imm,
            })?;
            if end > program.nodes.len() {
                return Err(KasmError::BadReduceCount {
                    node: index,
                    count: node.imm,
                });
            }
            for dep in base..end {
                mark_dependencies(program, dep, keep)?;
            }
        }
    }
    Ok(())
}

fn remap_node(index: usize, node: Node, remap: &[Option<u16>]) -> Result<Node, KasmError> {
    let mapped = |old: u16| {
        remap
            .get(old as usize)
            .and_then(|slot| *slot)
            .ok_or(KasmError::BadRef {
                node: index,
                reference: old,
            })
    };

    Ok(match node.op {
        Op::Input | Op::ConstI64 | Op::ConstF64 => node,
        Op::F64Op => {
            let sub = super::types::F64SubOp::from_imm(node.imm)?;
            let mut out = Node { a: mapped(node.a)?, ..node };
            if sub.is_binary() {
                out.b = mapped(node.b)?;
            }
            out
        }
        Op::Hash64
        | Op::NotBool
        | Op::Output
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64
        | Op::Lazy
        | Op::Force
        | Op::Adaptive
        | Op::Comptime
        | Op::Memoize
        | Op::Grad
        | Op::Vmap
        | Op::Pmap
        | Op::VLenI64
        | Op::VSumI64
        | Op::VRangeI64
        | Op::VReverseI64
        | Op::VAbsI64
        | Op::VNegI64
        | Op::VBitFlipI64 => Node {
            a: mapped(node.a)?,
            ..node
        },
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::EqI64
        | Op::AndBool
        | Op::OrBool
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::PextI64
        | Op::PdepI64
        | Op::LtI64
        | Op::LeI64
        | Op::Pipeline
        | Op::Fori
        | Op::WhileLoop
        | Op::Reduce
        | Op::Scan
        | Op::VAddI64
        | Op::VMulI64
        | Op::VSubI64
        | Op::VMaxI64
        | Op::VMinI64
        | Op::VConcatI64
        | Op::VBroadcastI64
        | Op::VEqI64
        | Op::VAndI64
        | Op::VOrI64
        | Op::VXorI64
        | Op::VGetI64  // Wave 7i — refs to vec + idx slots
        | Op::Fractal  // Wave 8 — refs to callee_hash + args slots
        | Op::Eval => Node {  // Wave 8 — refs to prog_bytes + args slots
            a: mapped(node.a)?,
            b: mapped(node.b)?,
            ..node
        },
        Op::ReduceAddI64 | Op::ReduceMulI64 => Node {
            a: mapped(node.a)?,
            ..node
        },
        Op::SelectI64 | Op::ClampI64 | Op::Cond => Node {
            a: mapped(node.a)?,
            b: mapped(node.b)?,
            imm: mapped(checked_imm_ref(node.imm, index)?)? as i16,
            ..node
        },
    })
}

pub(super) fn expect_ref(index: usize, reference: u16, ty: Ty, types: &[Ty]) -> Result<(), KasmError> {
    // Wave 7b — Ty::VecI64 is now a first-class type. Type equality
    // is the only check : a non-Vec op asking for I64 against a Vec
    // slot fails via the standard `*actual != ty` mismatch ; an
    // op explicitly asking for VecI64 (Op::Output Ty::VecI64) goes
    // through the same path symmetrically.
    let actual = types
        .get(reference as usize)
        .ok_or(KasmError::BadRef { node: index, reference })?;
    if *actual != ty {
        return Err(KasmError::TypeMismatch { node: index });
    }
    Ok(())
}

pub(super) fn ensure_ty(index: usize, actual: Ty, expected: Ty) -> Result<(), KasmError> {
    // Wave 7b — Vec types valid here (the equality check rejects
    // mismatches uniformly across all 4 Ty variants).
    if actual == expected {
        Ok(())
    } else {
        Err(KasmError::TypeMismatch { node: index })
    }
}

pub(super) fn node_result_type(node: Node) -> Ty {
    node.ty
}

pub(super) fn checked_imm_ref(reference: i16, node: usize) -> Result<u16, KasmError> {
    if reference < 0 {
        return Err(KasmError::BadRef { node, reference: reference as u16 });
    }
    Ok(reference as u16)
}

pub(crate) fn hash_i64(value: i64) -> i64 {
    let mut x = value as u64;
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    (x ^ (x >> 31)) as i64
}

pub(super) fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(super) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ─────────────────────────────────────────────────────────────────────
// Wave 4 (Phase Ω.10, Φ.11.3) — Multiple Dispatch
//
// First real Julia feature absorbed. A `MultiMethod` is a content-
// addressed bundle of `(ProgramSig, Hash)` pairs. Given runtime argument
// types, the dispatcher selects the program whose signature matches and
// runs it. Unlike Julia's mutable global method tables, MultiMethods are
// immutable: adding a method = building a new bundle (new content hash).
//
// Layout doctrine: lives in program.rs (no new module — fold into
// existing per CLAUDE.md). Encoding is canonical (methods sorted by sig
// lex order) so two equivalent bundles always hash identically.
// ─────────────────────────────────────────────────────────────────────

const MULTIMETHOD_MAGIC: &[u8; 4] = b"FMM\0";
const MULTIMETHOD_VERSION: u8 = 0;
/// Length of a Forge program hash on disk. Mirrors `crate::Hash`'s
/// 20-byte SHA-1 truncation used elsewhere in the storage layer.
const PROGRAM_HASH_LEN: usize = 20;

/// Wave 4 — bundle of typed methods sharing a logical name.
///
/// Conceptually a Julia generic function: one symbolic identity, many
/// implementations distinguished by argument types. The bundle itself
/// is content-addressed via `encode()`'s SHA-256, so adding a method
/// produces a fresh `MultiMethod` instance (no in-place mutation).
///
/// MVP semantics: **exact signature match**. A future wave can add a
/// subtype lattice (e.g. F64 → I64 implicit conversion) — for now the
/// runtime signature must equal an entry's signature byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiMethod {
    /// Sorted by `(inputs, outputs)` lex order. Hash is opaque — it is
    /// the SHA-1-truncated identity of a stored Program in the CAS.
    methods: Vec<(ProgramSig, [u8; PROGRAM_HASH_LEN])>,
}

impl MultiMethod {
    /// Build from an unsorted method list. Duplicates on signature are
    /// **not** rejected — the last-inserted method wins, matching
    /// Julia's "redefinition replaces" semantic. Sorting happens here so
    /// `encode()` is deterministic regardless of insertion order.
    pub fn new(methods: impl IntoIterator<Item = (ProgramSig, [u8; PROGRAM_HASH_LEN])>) -> Self {
        let mut by_sig: std::collections::BTreeMap<ProgramSig, [u8; PROGRAM_HASH_LEN]> =
            std::collections::BTreeMap::new();
        for (sig, hash) in methods {
            by_sig.insert(sig, hash);
        }
        Self {
            methods: by_sig.into_iter().collect(),
        }
    }

    /// Empty bundle. Resolves nothing — useful only as a starting point
    /// for `with_method` or as a `Default`.
    pub fn empty() -> Self {
        Self {
            methods: Vec::new(),
        }
    }

    /// Return a new bundle with `(sig, hash)` added or overriding any
    /// existing entry for `sig`. Immutable: the receiver is unchanged.
    /// O(n) — fine for small method tables (Julia's median is < 8).
    pub fn with_method(&self, sig: ProgramSig, hash: [u8; PROGRAM_HASH_LEN]) -> Self {
        let mut next: std::collections::BTreeMap<ProgramSig, [u8; PROGRAM_HASH_LEN]> =
            self.methods.iter().cloned().collect();
        next.insert(sig, hash);
        Self {
            methods: next.into_iter().collect(),
        }
    }

    /// Number of methods in the bundle.
    pub fn len(&self) -> usize {
        self.methods.len()
    }

    pub fn is_empty(&self) -> bool {
        self.methods.is_empty()
    }

    /// Iterate over methods in canonical (sorted) order.
    pub fn iter(&self) -> impl Iterator<Item = (&ProgramSig, &[u8; PROGRAM_HASH_LEN])> {
        self.methods.iter().map(|(s, h)| (s, h))
    }

    /// Wave 4 hot path — exact signature lookup. Returns the program
    /// hash whose signature matches `runtime_sig`, or `None` if no
    /// method applies (genuine "no match" — never a fake error, per the
    /// Tâche A.2 absence-as-Option invariant).
    pub fn resolve(&self, runtime_sig: &ProgramSig) -> Option<[u8; PROGRAM_HASH_LEN]> {
        // Methods are sorted by sig: binary search by Ord. Linear scan
        // would also work for small N but binary search keeps us future-
        // proof if a generic function ever grows past a few methods.
        self.methods
            .binary_search_by(|(sig, _)| sig.cmp(runtime_sig))
            .ok()
            .map(|idx| self.methods[idx].1)
    }

    /// Canonical wire encoding. Layout:
    ///
    /// ```text
    /// [0..4]   : magic "FMM\0"
    /// [4]      : version (currently 0)
    /// [5..7]   : u16 LE method count
    /// [7..]    : methods, each = [encoded sig][20-byte program hash]
    /// ```
    ///
    /// Two equivalent bundles produce byte-identical output thanks to
    /// the canonical sort in `new()` / `with_method()`. Hash this with
    /// SHA-256 to get the bundle's content-addressed identity.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + self.methods.len() * 32);
        out.extend_from_slice(MULTIMETHOD_MAGIC);
        out.push(MULTIMETHOD_VERSION);
        let count = self.methods.len() as u16;
        out.extend_from_slice(&count.to_le_bytes());
        for (sig, hash) in &self.methods {
            sig.encode_into(&mut out);
            out.extend_from_slice(hash);
        }
        out
    }

    /// Inverse of `encode()`. Validates magic, version, and that every
    /// method parses cleanly. Trailing bytes are an error (no implicit
    /// truncation — the CAS guarantees byte-exact roundtrips).
    pub fn decode(bytes: &[u8]) -> Result<Self, KasmError> {
        if bytes.len() < 7 {
            return Err(KasmError::BadMultiMethod("blob shorter than header".into()));
        }
        if &bytes[0..4] != MULTIMETHOD_MAGIC {
            return Err(KasmError::BadMultiMethod("bad magic".into()));
        }
        let version = bytes[4];
        if version != MULTIMETHOD_VERSION {
            return Err(KasmError::BadMultiMethod(format!(
                "unsupported version {version}"
            )));
        }
        let count = u16::from_le_bytes([bytes[5], bytes[6]]) as usize;
        let mut cursor = 7;
        let mut methods: Vec<(ProgramSig, [u8; PROGRAM_HASH_LEN])> = Vec::with_capacity(count);
        for _ in 0..count {
            let (sig, consumed) = ProgramSig::decode(&bytes[cursor..])?;
            cursor += consumed;
            if cursor + PROGRAM_HASH_LEN > bytes.len() {
                return Err(KasmError::BadMultiMethod("truncated program hash".into()));
            }
            let mut hash = [0u8; PROGRAM_HASH_LEN];
            hash.copy_from_slice(&bytes[cursor..cursor + PROGRAM_HASH_LEN]);
            cursor += PROGRAM_HASH_LEN;
            methods.push((sig, hash));
        }
        if cursor != bytes.len() {
            return Err(KasmError::BadMultiMethod(format!(
                "trailing bytes: {} extra",
                bytes.len() - cursor
            )));
        }
        // Validate canonical ordering. A roundtripped blob from another
        // node MUST already be sorted; if not, the bundle was hand-
        // forged and its content hash would diverge from any honest
        // producer's. Reject loudly.
        for window in methods.windows(2) {
            if window[0].0 >= window[1].0 {
                return Err(KasmError::BadMultiMethod(
                    "methods not in canonical sorted order".into(),
                ));
            }
        }
        Ok(Self { methods })
    }

    /// Content-addressed identity: SHA-256 of `encode()`. Two bundles
    /// with the same methods (regardless of insertion order) produce the
    /// same identity hash.
    pub fn identity(&self) -> [u8; 32] {
        digest(&self.encode())
    }
}

impl Default for MultiMethod {
    fn default() -> Self {
        Self::empty()
    }
}
