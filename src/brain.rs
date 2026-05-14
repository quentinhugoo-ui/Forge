//! Lean Forge brain: persistent memory + KASM tightening + Godel v2.
//!
//! This module deliberately keeps the path short. A program is stored in the
//! append-only CAS, tightened by the old symbolic/CSE tools, verified by the
//! Godel v2 semantic checker, then swapped in the active set with one memory
//! record written back to the same CAS.

use std::collections::BTreeSet;
use std::fmt;
use std::io;

use crate::agent::SymbolicAgent;
use crate::godel::applicator_v2::{ApplicatorV2, ApplicatorV2Error};
use crate::godel::observer::{capture, frame_hash};
use crate::godel::verifier_v2::{
    verify_v2_with_policy, RewriteV2, SemanticPolicy, VerificationOutcomeV2,
};
use crate::kasm::{execute, KasmError, Program};
use crate::{Hash, MonsterNode};

pub const BRAIN_HEAD_REF: &str = "refs/brain/latest";
pub const BRAIN_LATEST_ACTIVE_REF: &str = "refs/brain/latest-active";
pub const BRAIN_STATE_REF: &str = "refs/brain/state";
pub const BRAIN_SEMANTIC_REF_PREFIX: &str = "refs/brain/semantic/";
pub const BRAIN_SUBSTITUTION_REF_PREFIX: &str = "refs/brain/substitution/";
pub const BRAIN_MIN_EQUIV_SAMPLES: usize = 64;
pub const BRAIN_MAX_EQUIV_SAMPLES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrainAction {
    AlreadyTight,
    AcceptedSubstitution,
    RejectedCandidate,
}

impl BrainAction {
    fn as_str(self) -> &'static str {
        match self {
            BrainAction::AlreadyTight => "already_tight",
            BrainAction::AcceptedSubstitution => "accepted_substitution",
            BrainAction::RejectedCandidate => "rejected_candidate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainMemory {
    pub action: BrainAction,
    pub from: Hash,
    pub to: Option<Hash>,
    pub memory_hash: Option<Hash>,
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub candidate_count: usize,
    pub samples: usize,
    pub frame_before: [u8; 32],
    pub frame_after: [u8; 32],
    pub reasons: Vec<String>,
}

impl BrainMemory {
    pub fn accepted(&self) -> bool {
        self.action == BrainAction::AcceptedSubstitution
    }
}

#[derive(Debug)]
pub enum BrainError {
    Io(io::Error),
    ProgramMissing(Hash),
    ProgramParse(KasmError),
    Apply(ApplicatorV2Error),
}

impl fmt::Display for BrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrainError::Io(err) => write!(f, "brain store error: {err}"),
            BrainError::ProgramMissing(hash) => write!(f, "program {hash:?} missing from store"),
            BrainError::ProgramParse(err) => write!(f, "program parse failed: {err}"),
            BrainError::Apply(err) => write!(f, "godel applicator failed: {err:?}"),
        }
    }
}

impl std::error::Error for BrainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BrainError::Io(err) => Some(err),
            BrainError::ProgramParse(err) => Some(err),
            BrainError::ProgramMissing(_) | BrainError::Apply(_) => None,
        }
    }
}

impl From<io::Error> for BrainError {
    fn from(err: io::Error) -> Self {
        BrainError::Io(err)
    }
}

impl From<ApplicatorV2Error> for BrainError {
    fn from(err: ApplicatorV2Error) -> Self {
        BrainError::Apply(err)
    }
}

#[derive(Debug)]
pub struct ForgeBrain {
    applicator: ApplicatorV2,
    symbolic: SymbolicAgent,
    memories: Vec<BrainMemory>,
    samples: usize,
    last_memory: Option<Hash>,
}

impl Default for ForgeBrain {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeBrain {
    pub fn new() -> Self {
        Self::with_samples(8)
    }

    pub fn with_samples(samples: usize) -> Self {
        Self {
            applicator: ApplicatorV2::new(),
            symbolic: SymbolicAgent::new(),
            memories: Vec::new(),
            samples: samples.max(1),
            last_memory: None,
        }
    }

    pub fn rehydrate(node: &MonsterNode) -> Result<Self, BrainError> {
        Self::rehydrate_with_samples(node, 8)
    }

    pub fn rehydrate_with_samples(
        node: &MonsterNode,
        samples: usize,
    ) -> Result<Self, BrainError> {
        let mut brain = Self::with_samples(samples);
        let state = read_brain_state(node);
        let active = state
            .active
            .or_else(|| node.store().lookup_ref(BRAIN_LATEST_ACTIVE_REF))
            .map(|hash| resolve_program_hash(node, hash));
        if let Some(active) = active {
            if node.store().load(&active).is_some() {
                brain.applicator.activate(active);
            }
        }
        brain.last_memory = state.memory.or_else(|| node.store().lookup_ref(BRAIN_HEAD_REF));
        Ok(brain)
    }

    /// Store and activate a KASM program in the persistent brain refs.
    pub fn remember_program(
        &mut self,
        node: &MonsterNode,
        program: &Program,
    ) -> Result<Hash, BrainError> {
        let hash = node.store().store(program.bytes())?;
        self.applicator.activate(hash);
        node.store()
            .write_ref(&program_ref(hash), &hash, "brain program")?;
        node.store()
            .write_ref(&active_ref(hash), &hash, "brain active program")?;
        node.store()
            .write_ref(BRAIN_LATEST_ACTIVE_REF, &hash, "brain latest active")?;
        write_brain_state(
            node,
            Some(hash),
            node.store().lookup_ref(BRAIN_HEAD_REF),
        )?;
        Ok(hash)
    }

    /// Convenience entry point for callers that want the shortest path.
    pub fn absorb_program(
        &mut self,
        node: &MonsterNode,
        program: &Program,
    ) -> Result<BrainMemory, BrainError> {
        let hash = self.remember_program(node, program)?;
        self.tighten_program(node, hash)
    }

    /// Try to replace an active program by a smaller verified equivalent.
    pub fn tighten_program(
        &mut self,
        node: &MonsterNode,
        from: Hash,
    ) -> Result<BrainMemory, BrainError> {
        let bytes = node
            .store()
            .load(&from)
            .ok_or(BrainError::ProgramMissing(from))?;
        let program = Program::from_bytes(&bytes).map_err(BrainError::ProgramParse)?;
        let frame_before = frame_hash(&capture(node));
        let candidates = self.candidate_programs(&program);
        let candidate_count = candidates.len();
        let nodes_before = program.nodes().len();

        if candidates.is_empty() {
            let frame_after = frame_hash(&capture(node));
            return self.persist_memory(
                node,
                BrainMemory {
                    action: BrainAction::AlreadyTight,
                    from,
                    to: None,
                    memory_hash: None,
                    nodes_before,
                    nodes_after: nodes_before,
                    candidate_count,
                    samples: self.samples,
                    frame_before,
                    frame_after,
                    reasons: Vec::new(),
                },
            );
        }

        let mut rejected_reasons = Vec::new();
        for candidate in candidates {
            let to = node.store().store(candidate.bytes())?;
            let rewrite = RewriteV2::ProgramSubstitution { from, to };
            let verification_samples = strict_equiv_samples(self.samples);
            match verify_program_substitution_strict(
                node,
                &rewrite,
                &program,
                &candidate,
                verification_samples,
            ) {
                VerificationOutcomeV2::Accept => {
                    let _trace = self.applicator.apply(rewrite, node)?;
                    node.store()
                        .delete_ref(&active_ref(from))
                        .map_err(BrainError::Io)?;
                    node.store()
                        .write_ref(&program_ref(to), &to, "brain program")?;
                    node.store()
                        .write_ref(&active_ref(to), &to, "brain active program")?;
                    node.store().write_ref(
                        &brain_substitution_ref(from),
                        &to,
                        "brain accepted substitution",
                    )?;
                    node.store().write_ref(
                        BRAIN_LATEST_ACTIVE_REF,
                        &to,
                        "brain latest active",
                    )?;
                    let frame_after = frame_hash(&capture(node));
                    return self.persist_memory(
                        node,
                        BrainMemory {
                            action: BrainAction::AcceptedSubstitution,
                            from,
                            to: Some(to),
                            memory_hash: None,
                            nodes_before,
                            nodes_after: candidate.nodes().len(),
                            candidate_count,
                            samples: verification_samples,
                            frame_before,
                            frame_after,
                            reasons: Vec::new(),
                        },
                    );
                }
                VerificationOutcomeV2::Reject { reasons } => {
                    rejected_reasons.extend(reasons);
                }
            }
        }

        let frame_after = frame_hash(&capture(node));
        self.persist_memory(
            node,
            BrainMemory {
                action: BrainAction::RejectedCandidate,
                from,
                to: None,
                memory_hash: None,
                nodes_before,
                nodes_after: nodes_before,
                candidate_count,
                samples: strict_equiv_samples(self.samples),
                frame_before,
                frame_after,
                reasons: rejected_reasons,
            },
        )
    }

    pub fn is_active(&self, hash: &Hash) -> bool {
        self.applicator.is_active(hash)
    }

    pub fn active_count(&self) -> usize {
        self.applicator.active_count()
    }

    pub fn memories(&self) -> &[BrainMemory] {
        &self.memories
    }

    pub fn latest_memory_hash(&self) -> Option<Hash> {
        self.last_memory
    }

    fn candidate_programs(&self, program: &Program) -> Vec<Program> {
        let original = Hash::for_blob(program.bytes());
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();

        push_candidate(&mut out, &mut seen, original, program.canonical());
        push_candidate(&mut out, &mut seen, original, program.simplified());
        push_candidate(&mut out, &mut seen, original, program.cse());
        for ranked in self.symbolic.propose_rewrites(program) {
            push_candidate(&mut out, &mut seen, original, Ok(ranked.program));
        }

        out.sort_by_key(|p| {
            (
                p.nodes().len(),
                p.bytes().len(),
                Hash::for_blob(p.bytes()),
            )
        });
        out
    }

    fn persist_memory(
        &mut self,
        node: &MonsterNode,
        memory: BrainMemory,
    ) -> Result<BrainMemory, BrainError> {
        let previous = self
            .last_memory
            .or_else(|| node.store().lookup_ref(BRAIN_HEAD_REF));
        let memory = persist_memory_record(node, memory, previous)?;
        self.last_memory = memory.memory_hash;
        self.memories.push(memory.clone());
        Ok(memory)
    }
}

pub fn publish_program_substitution(
    node: &MonsterNode,
    from: Hash,
    from_program: &Program,
    to_program: &Program,
    samples: usize,
) -> Result<Option<Hash>, BrainError> {
    if Hash::for_blob(from_program.bytes()) != from {
        return Ok(None);
    }
    let to = Hash::for_blob(to_program.bytes());
    if to == from || to_program.nodes().len() >= from_program.nodes().len() {
        return Ok(None);
    }

    let frame_before = frame_hash(&capture(node));
    node.store().store(from_program.bytes())?;
    node.store().store(to_program.bytes())?;

    let rewrite = RewriteV2::ProgramSubstitution { from, to };
    let samples = strict_equiv_samples(samples);
    match verify_program_substitution_strict(node, &rewrite, from_program, to_program, samples) {
        VerificationOutcomeV2::Accept => {
            node.store()
                .delete_ref(&active_ref(from))
                .map_err(BrainError::Io)?;
            node.store()
                .write_ref(&program_ref(to), &to, "brain program")?;
            node.store()
                .write_ref(&active_ref(to), &to, "brain active program")?;
            node.store().write_ref(
                &brain_substitution_ref(from),
                &to,
                "brain accepted substitution",
            )?;
            node.store()
                .write_ref(BRAIN_LATEST_ACTIVE_REF, &to, "brain latest active")?;
            write_brain_state(node, Some(to), node.store().lookup_ref(BRAIN_HEAD_REF))?;
            let frame_after = frame_hash(&capture(node));
            let memory = BrainMemory {
                action: BrainAction::AcceptedSubstitution,
                from,
                to: Some(to),
                memory_hash: None,
                nodes_before: from_program.nodes().len(),
                nodes_after: to_program.nodes().len(),
                candidate_count: 1,
                samples,
                frame_before,
                frame_after,
                reasons: Vec::new(),
            };
            let _ = persist_memory_record(node, memory, node.store().lookup_ref(BRAIN_HEAD_REF))?;
            Ok(Some(to))
        }
        VerificationOutcomeV2::Reject { .. } => Ok(None),
    }
}

pub fn publish_semantic_attractor(
    node: &MonsterNode,
    program_hash: Hash,
    program: &Program,
    samples: usize,
) -> Result<Option<Hash>, BrainError> {
    if Hash::for_blob(program.bytes()) != program_hash {
        return Ok(None);
    }
    let Ok(fingerprint) = program.semantic_fingerprint() else {
        return Ok(None);
    };
    node.store().store(program.bytes())?;
    node.store()
        .write_ref(&program_ref(program_hash), &program_hash, "brain program")?;

    let semantic_ref = brain_semantic_ref(&fingerprint);
    let Some(existing_hash) = node.store().lookup_ref(&semantic_ref) else {
        node.store()
            .write_ref(&semantic_ref, &program_hash, "brain semantic attractor")?;
        return Ok(Some(program_hash));
    };
    if existing_hash == program_hash {
        return Ok(Some(program_hash));
    }

    let Some(existing_bytes) = node.store().load(&existing_hash) else {
        node.store()
            .write_ref(&semantic_ref, &program_hash, "brain semantic attractor")?;
        return Ok(Some(program_hash));
    };
    let Ok(existing_program) = Program::from_bytes(&existing_bytes) else {
        node.store()
            .write_ref(&semantic_ref, &program_hash, "brain semantic attractor")?;
        return Ok(Some(program_hash));
    };

    let existing_score = program_score(&existing_program, existing_hash);
    let current_score = program_score(program, program_hash);
    if existing_score <= current_score {
        if let Some(to) =
            publish_program_substitution(node, program_hash, program, &existing_program, samples)?
        {
            return Ok(Some(to));
        }
        return Ok(Some(existing_hash));
    }

    if publish_program_substitution(node, existing_hash, &existing_program, program, samples)?
        .is_some()
    {
        node.store()
            .write_ref(&semantic_ref, &program_hash, "brain semantic attractor")?;
        Ok(Some(program_hash))
    } else {
        Ok(Some(existing_hash))
    }
}

pub fn tighten_program_for_execution(
    node: &MonsterNode,
    from: Hash,
    program: Program,
    samples: usize,
) -> Program {
    let mut current = program;
    if let Ok(candidate) = current.cse() {
        if Hash::for_blob(candidate.bytes()) != from
            && candidate.nodes().len() < current.nodes().len()
        {
            let _ = publish_program_substitution(node, from, &current, &candidate, samples);
            current = candidate;
        }
    }

    let current_hash = Hash::for_blob(current.bytes());
    if let Ok(Some(attractor)) =
        publish_semantic_attractor(node, current_hash, &current, samples)
    {
        if attractor != current_hash {
            if let Some(bytes) = node.store().load(&attractor) {
                if let Ok(program) = Program::from_bytes(&bytes) {
                    return program;
                }
            }
        }
    }
    current
}

fn persist_memory_record(
    node: &MonsterNode,
    mut memory: BrainMemory,
    previous: Option<Hash>,
) -> Result<BrainMemory, BrainError> {
    let bytes = encode_memory(&memory, previous);
    let hash = node.store().store(&bytes)?;
    node.store()
        .write_ref(BRAIN_HEAD_REF, &hash, "brain latest memory")?;
    write_brain_state(
        node,
        node.store().lookup_ref(BRAIN_LATEST_ACTIVE_REF),
        Some(hash),
    )?;
    memory.memory_hash = Some(hash);
    Ok(memory)
}

#[derive(Debug, Default, Clone, Copy)]
struct BrainState {
    active: Option<Hash>,
    memory: Option<Hash>,
}

fn read_brain_state(node: &MonsterNode) -> BrainState {
    let Some(hash) = node.store().lookup_ref(BRAIN_STATE_REF) else {
        return BrainState::default();
    };
    let Some(bytes) = node.store().load(&hash) else {
        return BrainState::default();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return BrainState::default();
    };
    let mut state = BrainState::default();
    for line in text.lines() {
        if let Some(hex) = line.strip_prefix("active=") {
            state.active = Hash::from_hex(hex);
        } else if let Some(hex) = line.strip_prefix("memory=") {
            state.memory = Hash::from_hex(hex);
        }
    }
    state
}

fn write_brain_state(
    node: &MonsterNode,
    active: Option<Hash>,
    memory: Option<Hash>,
) -> Result<Hash, BrainError> {
    let mut out = String::new();
    out.push_str("forge-brain-state-v1\n");
    push_hash_line(&mut out, "active", active);
    push_hash_line(&mut out, "memory", memory);
    let hash = node.store().store(out.as_bytes())?;
    node.store()
        .write_ref(BRAIN_STATE_REF, &hash, "brain compact state")?;
    Ok(hash)
}

fn push_candidate(
    out: &mut Vec<Program>,
    seen: &mut BTreeSet<Hash>,
    original: Hash,
    candidate: Result<Program, KasmError>,
) {
    let Ok(program) = candidate else {
        return;
    };
    let hash = Hash::for_blob(program.bytes());
    if hash == original || !seen.insert(hash) {
        return;
    }
    out.push(program);
}

fn program_score(program: &Program, hash: Hash) -> (usize, usize, Hash) {
    (program.nodes().len(), program.bytes().len(), hash)
}

fn strict_equiv_samples(samples: usize) -> usize {
    samples
        .max(BRAIN_MIN_EQUIV_SAMPLES)
        .min(BRAIN_MAX_EQUIV_SAMPLES)
}

fn verify_program_substitution_strict(
    node: &MonsterNode,
    rewrite: &RewriteV2,
    from_program: &Program,
    to_program: &Program,
    samples: usize,
) -> VerificationOutcomeV2 {
    let RewriteV2::ProgramSubstitution { from, to } = rewrite else {
        return verify_v2_with_policy(rewrite, node, SemanticPolicy::Trust);
    };
    let samples = strict_equiv_samples(samples);
    let mut reasons = Vec::new();

    if Hash::for_blob(from_program.bytes()) != *from {
        reasons.push("source program hash does not match rewrite".to_string());
    }
    if Hash::for_blob(to_program.bytes()) != *to {
        reasons.push("target program hash does not match rewrite".to_string());
    }
    if from_program.inputs() != to_program.inputs() {
        reasons.push(format!(
            "input arity mismatch: from has {}, to has {}",
            from_program.inputs(),
            to_program.inputs()
        ));
    }
    if from_program.outputs() != to_program.outputs() {
        reasons.push(format!(
            "output arity mismatch: from has {}, to has {}",
            from_program.outputs(),
            to_program.outputs()
        ));
    }
    if from_program.output_types() != to_program.output_types() {
        reasons.push("output type mismatch".to_string());
    }
    if from_program.target().needs_external_backend() || to_program.target().needs_external_backend() {
        reasons.push("external-backend programs cannot be brain-substituted".to_string());
    }

    match (from_program.semantic_fingerprint(), to_program.semantic_fingerprint()) {
        (Ok(left), Ok(right)) if left == right => {}
        (Ok(_), Ok(_)) => reasons.push("semantic fingerprint mismatch".to_string()),
        (Err(err), _) => reasons.push(format!("source semantic fingerprint failed: {err}")),
        (_, Err(err)) => reasons.push(format!("target semantic fingerprint failed: {err}")),
    }

    if !reasons.is_empty() {
        return VerificationOutcomeV2::Reject { reasons };
    }

    match verify_v2_with_policy(rewrite, node, SemanticPolicy::SampleBased { samples }) {
        VerificationOutcomeV2::Accept => {}
        rejected @ VerificationOutcomeV2::Reject { .. } => return rejected,
    }

    for sample_idx in 0..samples {
        let args = brain_guard_sample_args(from_program.inputs(), sample_idx as u64);
        let from_out = match execute(from_program, &args) {
            Ok(out) => out,
            Err(err) => {
                reasons.push(format!("source execute on guard sample {sample_idx} failed: {err}"));
                continue;
            }
        };
        let to_out = match execute(to_program, &args) {
            Ok(out) => out,
            Err(err) => {
                reasons.push(format!("target execute on guard sample {sample_idx} failed: {err}"));
                continue;
            }
        };
        if from_out != to_out {
            reasons.push(format!(
                "guard sample {sample_idx} output mismatch: from={:?} to={:?}",
                from_out, to_out
            ));
        }
    }

    if reasons.is_empty() {
        VerificationOutcomeV2::Accept
    } else {
        VerificationOutcomeV2::Reject { reasons }
    }
}

fn brain_guard_sample_args(inputs: u8, sample_idx: u64) -> Vec<u8> {
    let corners: [i64; 16] = [
        0,
        1,
        -1,
        2,
        -2,
        7,
        -7,
        13,
        -13,
        i8::MAX as i64,
        i8::MIN as i64,
        i16::MAX as i64,
        i16::MIN as i64,
        i32::MAX as i64,
        i32::MIN as i64,
        i64::MAX,
    ];
    let mut out = Vec::with_capacity(inputs as usize * 8);
    for slot in 0..inputs {
        let value = if (sample_idx as usize) < corners.len() {
            corners[sample_idx as usize].wrapping_add((slot as i64).wrapping_mul(31))
        } else {
            let mut x = sample_idx
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                ^ ((slot as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
            x ^= x >> 30;
            x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
            x ^= x >> 27;
            x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
            (x ^ (x >> 31)) as i64
        };
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn program_ref(hash: Hash) -> String {
    format!("refs/brain/program/{}", hash.as_hex())
}

fn active_ref(hash: Hash) -> String {
    format!("refs/brain/active/{}", hash.as_hex())
}

pub fn brain_substitution_ref(hash: Hash) -> String {
    format!("{BRAIN_SUBSTITUTION_REF_PREFIX}{}", hash.as_hex())
}

pub fn brain_semantic_ref(fingerprint: &[u8; 32]) -> String {
    format!("{BRAIN_SEMANTIC_REF_PREFIX}{}", hex_bytes(fingerprint))
}

pub fn resolve_program_hash(node: &MonsterNode, func: Hash) -> Hash {
    let mut current = func;
    for _ in 0..8 {
        let Some(next) = node.store().lookup_ref(&brain_substitution_ref(current)) else {
            break;
        };
        if next == current || node.store().load(&next).is_none() {
            break;
        }
        current = next;
    }
    current
}

fn encode_memory(memory: &BrainMemory, previous: Option<Hash>) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("forge-brain-v1\n");
    out.push_str("action=");
    out.push_str(memory.action.as_str());
    out.push('\n');
    push_hash_line(&mut out, "from", Some(memory.from));
    push_hash_line(&mut out, "to", memory.to);
    push_hash_line(&mut out, "prev", previous);
    out.push_str(&format!("nodes_before={}\n", memory.nodes_before));
    out.push_str(&format!("nodes_after={}\n", memory.nodes_after));
    out.push_str(&format!("candidate_count={}\n", memory.candidate_count));
    out.push_str(&format!("samples={}\n", memory.samples));
    out.push_str("frame_before=");
    out.push_str(&hex_bytes(&memory.frame_before));
    out.push('\n');
    out.push_str("frame_after=");
    out.push_str(&hex_bytes(&memory.frame_after));
    out.push('\n');
    if memory.reasons.is_empty() {
        out.push_str("reasons=\n");
    } else {
        out.push_str("reasons=");
        for (idx, reason) in memory.reasons.iter().enumerate() {
            if idx > 0 {
                out.push_str(" | ");
            }
            out.push_str(&sanitize_line(reason));
        }
        out.push('\n');
    }
    out.into_bytes()
}

fn push_hash_line(out: &mut String, key: &str, hash: Option<Hash>) {
    out.push_str(key);
    out.push('=');
    if let Some(hash) = hash {
        out.push_str(&hash.as_hex());
    }
    out.push('\n');
}

fn sanitize_line(value: &str) -> String {
    value
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::{Node, Target, Ty};
    use crate::{MemoryGovernor, Store, TmpDir};

    fn fresh_node(tag: &str) -> (MonsterNode, TmpDir) {
        let path = TmpDir::new(crate::fresh_tmp_path("forge-brain", tag));
        let node = MonsterNode::new(
            Store::open(path.as_ref()).unwrap(),
            MemoryGovernor::new(1024 * 1024),
        );
        (node, path)
    }

    fn add_zero_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            4,
            vec![
                Node::input(0),
                Node::const_i64(0),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    fn id_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            2,
            vec![Node::input(0), Node::output(0, Ty::I64)],
        )
        .unwrap()
    }

    fn add_self_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            3,
            vec![Node::input(0), Node::add(0, 0), Node::output(1, Ty::I64)],
        )
        .unwrap()
    }

    fn shl_one_program() -> Program {
        Program::new(
            Target::Cpu,
            1,
            1,
            4,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::shl(0, 1),
                Node::output(2, Ty::I64),
            ],
        )
        .unwrap()
    }

    #[test]
    fn brain_fuses_symbolic_godel_and_persistent_memory() {
        let (node, _path) = fresh_node("accept");
        let mut brain = ForgeBrain::new();
        let program = add_zero_program();
        let from = brain.remember_program(&node, &program).unwrap();

        let memory = brain.tighten_program(&node, from).unwrap();

        assert_eq!(memory.action, BrainAction::AcceptedSubstitution);
        assert!(memory.nodes_after < memory.nodes_before);
        assert_eq!(memory.samples, BRAIN_MIN_EQUIV_SAMPLES);
        let to = memory.to.expect("accepted substitution target");
        assert!(!brain.is_active(&from));
        assert!(brain.is_active(&to));
        assert_eq!(brain.active_count(), 1);
        assert_eq!(node.store().lookup_ref(BRAIN_LATEST_ACTIVE_REF), Some(to));
        assert_eq!(
            node.store().lookup_ref(&brain_substitution_ref(from)),
            Some(to),
            "accepted rewrite must persist as a direct shortcut"
        );
        assert_eq!(resolve_program_hash(&node, from), to);

        let memory_hash = node.store().lookup_ref(BRAIN_HEAD_REF).unwrap();
        assert_eq!(memory.memory_hash, Some(memory_hash));
        let memory_text = String::from_utf8(node.store().load(&memory_hash).unwrap()).unwrap();
        assert!(memory_text.contains("forge-brain-v1"));
        assert!(memory_text.contains("action=accepted_substitution"));
        assert!(memory_text.contains("nodes_before=4"));
        assert!(memory_text.contains("nodes_after=2"));
        assert!(memory_text.contains("samples=64"));
    }

    #[test]
    fn brain_keeps_tight_program_active_without_extra_branching() {
        let (node, _path) = fresh_node("tight");
        let mut brain = ForgeBrain::new();
        let from = brain.remember_program(&node, &id_program()).unwrap();

        let memory = brain.tighten_program(&node, from).unwrap();

        assert_eq!(memory.action, BrainAction::AlreadyTight);
        assert_eq!(memory.to, None);
        assert!(brain.is_active(&from));
        assert_eq!(brain.active_count(), 1);
        assert_eq!(node.store().lookup_ref(BRAIN_LATEST_ACTIVE_REF), Some(from));
        let memory_hash = node.store().lookup_ref(BRAIN_HEAD_REF).unwrap();
        let memory_text = String::from_utf8(node.store().load(&memory_hash).unwrap()).unwrap();
        assert!(memory_text.contains("action=already_tight"));
    }

    #[test]
    fn call_bytes_auto_publishes_brain_substitution() {
        let (node, _path) = fresh_node("call-auto");
        let from = node
            .store()
            .store(add_zero_program().bytes())
            .expect("store program");

        let call = node.call_bytes(&from, &5i64.to_le_bytes()).unwrap();
        let output = node.store().load(&call.result).unwrap();
        assert_eq!(i64::from_le_bytes(output.try_into().unwrap()), 5);

        let to = node
            .store()
            .lookup_ref(&brain_substitution_ref(from))
            .expect("auto substitution ref");
        assert_ne!(from, to);
        assert_eq!(resolve_program_hash(&node, from), to);
    }

    #[test]
    fn semantic_attractor_converges_equivalent_programs_to_shorter_canon() {
        let (node, _path) = fresh_node("semantic-attractor");
        let short = add_self_program();
        let long = shl_one_program();
        let short_hash = node.store().store(short.bytes()).expect("short store");
        let long_hash = node.store().store(long.bytes()).expect("long store");

        let short_call = node.call_bytes(&short_hash, &7i64.to_le_bytes()).unwrap();
        let short_output = node.store().load(&short_call.result).unwrap();
        assert_eq!(i64::from_le_bytes(short_output.try_into().unwrap()), 14);
        let semantic_ref = brain_semantic_ref(&short.semantic_fingerprint().unwrap());
        assert_eq!(node.store().lookup_ref(&semantic_ref), Some(short_hash));

        let long_call = node.call_bytes(&long_hash, &9i64.to_le_bytes()).unwrap();
        let long_output = node.store().load(&long_call.result).unwrap();
        assert_eq!(i64::from_le_bytes(long_output.try_into().unwrap()), 18);

        assert_eq!(node.store().lookup_ref(&semantic_ref), Some(short_hash));
        assert_eq!(resolve_program_hash(&node, long_hash), short_hash);
        assert_eq!(
            node.store().lookup_ref(&brain_substitution_ref(long_hash)),
            Some(short_hash)
        );
    }

    #[test]
    fn strict_substitution_rejects_shorter_non_equivalent_program() {
        let (node, _path) = fresh_node("strict-reject");
        let from_program = add_self_program();
        let to_program = id_program();
        let from = node.store().store(from_program.bytes()).expect("from store");
        node.store().store(to_program.bytes()).expect("to store");

        let accepted =
            publish_program_substitution(&node, from, &from_program, &to_program, 1).unwrap();

        assert_eq!(accepted, None);
        assert_eq!(node.store().lookup_ref(&brain_substitution_ref(from)), None);
    }

    #[test]
    fn brain_rehydrates_active_state_after_store_reopen() {
        let path = TmpDir::new(crate::fresh_tmp_path("forge-brain", "rehydrate"));
        let store_path = path.path().to_path_buf();
        let from;
        let to;
        let memory_hash;

        {
            let node = MonsterNode::new(
                Store::open(store_path.clone()).unwrap(),
                MemoryGovernor::new(1024 * 1024),
            );
            let mut brain = ForgeBrain::new();
            from = brain
                .remember_program(&node, &add_zero_program())
                .expect("remember");
            let memory = brain.tighten_program(&node, from).expect("tighten");
            to = memory.to.expect("accepted target");
            memory_hash = memory.memory_hash.expect("memory hash");
            assert!(brain.is_active(&to));
        }

        {
            let node = MonsterNode::new(
                Store::open(store_path.clone()).unwrap(),
                MemoryGovernor::new(1024 * 1024),
            );
            let brain = ForgeBrain::rehydrate(&node).expect("rehydrate");
            assert!(brain.is_active(&to));
            assert!(!brain.is_active(&from));
            assert_eq!(brain.active_count(), 1);
            assert_eq!(brain.latest_memory_hash(), Some(memory_hash));
            assert_eq!(resolve_program_hash(&node, from), to);

            let call = node.call_bytes(&from, &13i64.to_le_bytes()).unwrap();
            let output = node.store().load(&call.result).unwrap();
            assert_eq!(i64::from_le_bytes(output.try_into().unwrap()), 13);
        }
    }
}
