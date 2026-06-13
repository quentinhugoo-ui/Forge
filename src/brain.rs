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
pub const BRAIN_SKILL_REF_PREFIX: &str = "refs/brain/skill/";
pub const BRAIN_SKILL_PROMOTION_REF_PREFIX: &str = "refs/brain/skill-promotion/";
pub const BRAIN_SEARCHARCHIVE_COMMAND: &str = "/searcharchive_";
pub const BRAIN_SEARCHARCHIVE_RESULT_SCHEMA: &str = "forge.brain.searcharchive.result.v1";
pub const BRAIN_RENAME_SESSION_COMMAND: &str = "/rename_session_";
pub const BRAIN_RENAME_SESSION_RESULT_SCHEMA: &str = "forge.brain.rename_session.result.v1";
pub const BRAIN_GOOGLEWEB_COMMAND: &str = "/googleweb_";
pub const BRAIN_GOOGLEWEB_RESULT_SCHEMA: &str = "forge.webexplorer.googleweb.result.v1";
pub const BRAIN_MAPS_COMMAND: &str = "/maps_";
pub const BRAIN_MAPS_RESULT_SCHEMA: &str = "forge.webexplorer.maps.result.v1";
pub const BRAIN_GMAIL_COMMAND: &str = "/gmail_";
pub const BRAIN_GMAIL_COM_COMMAND: &str = "/gmail_com";
pub const BRAIN_GMAIL_RESULT_SCHEMA: &str = "forge.webexplorer.gmail.result.v1";
pub const BRAIN_AIRBNB_COMMAND: &str = "/airbnb_";
pub const BRAIN_AIRBNB_RESULT_SCHEMA: &str = "forge.webexplorer.airbnb.result.v1";
pub const BRAIN_NEWIMAGE_COMMAND: &str = "/newimage_";
pub const BRAIN_EDITIMAGE_COMMAND: &str = "/editimage_";
pub const BRAIN_IMAGE_RESULT_SCHEMA: &str = "forge.image.result.v1";
pub const BRAIN_QUESTIONNAIRE_COMMAND: &str = "/questionnaire_";
pub const BRAIN_QUESTIONNAIRE_RESULT_SCHEMA: &str = "forge.questionnaire.result.v1";
pub const BRAIN_SCIENCE_COMMAND: &str = "/sciencebrain_";
pub const BRAIN_CODING_COMMAND: &str = "/codingbrain_";
pub const BRAIN_SEGMENT_RESULT_SCHEMA: &str = "forge.brain.segment.result.v1";
pub const BRAIN_CODEACT_ROUTING_RULES: &str = "Brain segment priority: while the active segment is general, the LLM must classify the user's task by domain before answering. On the first user message of a session, identify the chat subject, choose a short specific title, and emit the internal line /\"nomduchat\"_renamechat_ where nomduchat is the chosen title. The title must be 2-5 natural words, nominal and specific, not a copy of the prompt, not only a proper noun, and the rename must not be described in visible prose. If the task belongs to science, engineering, mathematics, biology, chemistry, physics, cryptography, optimization, formal analysis, physical product design, electronics, mechanics, robotics, CAD/3D, Banger, Monster, /newcompute_ or future Banger 3D work, first activate /sciencebrain_ before giving the specialized answer. Physical product or prototype conception is engineering by default, even when the object is ordinary or newly mentioned. Geography routing is a hard module rule: geographic place detected alone means /maps_; geographic place plus travel/vacation/stay lexical field means /maps_ first, then /airbnb_ as the next WebExplorer page. Ordinary city, place, local weather, map, route, country, region, island, coordinates, Google Earth, where-is, or geographic-context requests are geospatial tasks: prefer /maps_ and do not activate /sciencebrain_ unless the user explicitly asks for scientific meteorology, climate modeling, physics, engineering, or compute analysis. In visible prose, wrap cities, local places and countries as #{Name}, and other named geographic or space entities as @{Name}: regions, continents, seas, oceans, rivers, lakes, mountains, islands, addresses, landmarks, GPS coordinates, planets, moons, stars and constellations; do not wrap generic category words. This is semantic domain routing, not keyword routing: the LLM must infer the implied domain from the user's natural-language request, even when the object, field or project name has never appeared in this Brain. If the task belongs to software engineering, coding, websites, applications, repositories, debugging, refactoring, tests, architecture, scripts, Rust, TypeScript, Electron, APIs, build systems or developer tooling, first activate /codingbrain_ before giving the specialized answer. Module priority: when the user asks about a city, place, country, region, island, local weather for a place, route, map, coordinates, Google Earth, where something is, or any geographic context without travel/vacation/stay intent, prefer /maps_ over /sciencebrain_, /airbnb_, and /googleweb_; use the Brain home city as the default maps target when no place is specified. When a geographic place appears with travel/vacation/stay language such as voyage, vacances, partir, visiter, tourisme, sejour, destination, dates, guests, lodging, accommodation, hotel-like stay, vacation rental, house/apartment/home rental, booking, budget for stays, or short-term stay intent, emit /maps_ first for Google Earth context and then /airbnb_ for the next WebExplorer page. /googleweb_ is only for generic web research that is not owned by a more specific module. Clarifying questionnaire tools belong to specialized Brain catalogs; while the active segment is general, switch to the correct Brain segment before opening a questionnaire for specialized work.";
pub const BRAIN_WORKSPACE_COMMAND: &str = "/workspace_";
pub const BRAIN_NEWCOMPUTE_COMMAND: &str = "/newcompute_";
pub const BRAIN_SELECTCOMPUTE_COMMAND: &str = "/selectcompute_";
pub const BRAIN_NAMED_COMPUTE_COMMAND: &str = "/compute_<name>_";
pub const BRAIN_NEWOBJECT_COMMAND: &str = "/newobject_";
pub const BRAIN_WEB_COMMAND: &str = "/web_";
pub const BRAIN_FRONTDESIGN_COMMAND: &str = "/frontdesign_";
pub const BRAIN_GOOGLE_AGENDA_COMMAND: &str = "/google_agenda_";
pub const BRAIN_BRAIN_COMMAND: &str = "/brain_";
pub const BRAIN_NEWMODULE_COMMAND: &str = "/newmodule_";
pub const BRAIN_RUST_PORT_ADAPTER_COMMAND: &str = "/rust_port_adapter_";
pub const BRAIN_RUST_STATE_STORE_COMMAND: &str = "/rust_state_store_";
pub const BRAIN_SEARCHARCHIVE_COMMAND_DESCRIPTION: &str = "Search Brain memory when the user asks to recall prior sessions, past decisions, archived context, previous files, or something already discussed. Do not use for fresh web search or current file/project work.";
pub const BRAIN_RENAME_SESSION_COMMAND_DESCRIPTION: &str = "Rename the current chat session with the Brain-owned compact syntax /\"nomduchat\"_renamechat_ after identifying the first user message subject. The app uses the quoted nomduchat field as the sidebar title. The event is internal and must not be described in visible prose. Use 2-5 natural words, like Codex or Claude; avoid copying the prompt or using only a proper noun.";
pub const BRAIN_GOOGLEWEB_COMMAND_DESCRIPTION: &str = "Open contained WebExplorer on a generic Google search when the user wants current web information and no specific module owns the request. Do not use for Gmail, Airbnb/travel lodging, image generation/editing, or local workspace work.";
pub const BRAIN_MAPS_COMMAND_DESCRIPTION: &str = "Open contained WebExplorer on Google Earth for any detected geographic place. For geography alone, Maps is the only WebExplorer page. For geography plus travel/vacation/stay intent, open Maps first and Airbnb next. Prefer over /sciencebrain_ and /googleweb_ for ordinary geographic context; use Brain home city when no target is specified. Do not read device location silently; current-location use requires explicit user permission.";
pub const BRAIN_GMAIL_COMMAND_DESCRIPTION: &str = "Use Gmail for mail tasks: open mailbox, search messages, inspect, summarize, draft, or prepare replies. The LLM writes a natural sentence first; never send email automatically and do not use /googleweb_ for Gmail.";
pub const BRAIN_GMAIL_COM_COMMAND_DESCRIPTION: &str = "Open the Gmail sign-in/mail entry URL directly in split-screen WebExplorer when the user asks to access or open Gmail. Use only for navigation to Gmail, not for generic mail reasoning or web search.";
pub const BRAIN_AIRBNB_COMMAND_DESCRIPTION: &str = "Use Airbnb after /maps_ when a geographic place is detected together with travel/vacation/stay language: voyage, vacances, partir, visiter, tourisme, sejour, destination, dates, guests, lodging, accommodation, hotel-like stay, home/apartment rental, vacation rental, booking, budget for stays, or short-term stay search. Do not use for city facts, local weather, geography, maps, or routes without travel/vacation/stay intent.";
pub const BRAIN_NEWIMAGE_COMMAND_DESCRIPTION: &str = "Generate a brand-new image from a text prompt: draw, create, render, imagine, make a logo/poster/scene/asset. Do not use /workspace_; no local project folder is required.";
pub const BRAIN_EDITIMAGE_COMMAND_DESCRIPTION: &str = "Edit an existing image attached, selected, or visible in the conversation: retouch, remove/add/replace elements, change colors/background/style, crop, upscale, or transform. Prefer over /workspace_; do not answer with an external-tool prompt.";
pub const BRAIN_QUESTIONNAIRE_COMMAND_DESCRIPTION: &str = "Open a paginated questionnaire above the chat composer when the LLM needs multiple clarifying questions. Use after any required Brain segment switch; never use before /sciencebrain_ or /codingbrain_ while the active Brain is general and the request is specialized. Put each page question in q1, q2, q3... and provide exactly three choices with q1_options, q2_options...";
pub const BRAIN_SCIENCE_COMMAND_DESCRIPTION: &str = "Mandatory first action while the active Brain is general and the LLM understands that the user's task belongs to science, engineering, mathematics, biology, chemistry, physics, cryptography, formal analysis, physical product design, electronics, mechanics, robotics, CAD/3D, Banger, Monster, /newcompute_ or future Banger 3D work. Physical product/prototype conception is engineering by default. Activate /sciencebrain_ before questionnaire or specialized answer; after activation, use the injected science brain catalog.";
pub const BRAIN_CODING_COMMAND_DESCRIPTION: &str = "Mandatory first action while the active Brain is general and the LLM understands that the user's task belongs to software engineering, coding, websites, applications, repositories, debugging, refactoring, tests, architecture, scripts, Rust/TypeScript/Electron, APIs, build systems, or developer tooling. Activate /codingbrain_ before the specialized answer; after activation, use the injected coding brain catalog.";
pub const BRAIN_WORKSPACE_COMMAND_DESCRIPTION: &str = "Ask the user to choose a local project/workspace folder only for coding, repository, filesystem, build, script, or project-file work. Never use for web, Gmail, Airbnb, Brain memory, image generation, or image editing.";
pub const BRAIN_NEWCOMPUTE_COMMAND_DESCRIPTION: &str = "Start a new typed Monster compute when the user needs heavy/local math, simulation, numeric analysis, tensors, optimization, batch calculation, or verifiable compute artifacts rather than a normal prose answer.";
pub const BRAIN_SELECTCOMPUTE_COMMAND_DESCRIPTION: &str = "Reuse a saved compute from the Brain compute library when the user wants to rerun, adapt, compare, or continue a previous/saved compute instead of defining a new one.";
pub const BRAIN_NAMED_COMPUTE_COMMAND_DESCRIPTION: &str = "Invoke a promoted named compute specialization when the user names or clearly matches a known compute pattern. Use after the named compute exists; otherwise start with /newcompute_.";
pub const BRAIN_NEWOBJECT_COMMAND_DESCRIPTION: &str = "Create or modify a Banger 3D/object contract from an explicit object, scene, SDF, geometry, material, or computational-design request. Do not use for 2D image editing.";
pub const BRAIN_WEB_COMMAND_DESCRIPTION: &str = "Run bounded web research through the contained web peripheral for research/navigation workflows not owned by Gmail, Airbnb, or Google-specific search. Never make browser content the global app shell.";
pub const BRAIN_FRONTDESIGN_COMMAND_DESCRIPTION: &str = "Change the app display colors or color palettes when the user asks. Use only for visual color/theme changes, not general coding or image generation.";
pub const BRAIN_GOOGLE_AGENDA_COMMAND_DESCRIPTION: &str = "Prepare a Google Calendar action when the user asks to create, inspect, or plan calendar events with dates, times, attendees, reminders, or schedule details. Execution stays user-confirmed.";
pub const BRAIN_BRAIN_COMMAND_DESCRIPTION: &str = "Operate on Brain itself: save/recall/update memory, adjust CodeAct descriptions, promote procedural knowledge, inspect Brain state, or reason about Brain rules. Not for ordinary user tasks.";
pub const BRAIN_NEWMODULE_COMMAND_DESCRIPTION: &str = "Create a new narrow module contract when the user wants a new app/module integration or sidebar capability. Define the module before adding broad implementation surface.";
pub const BRAIN_RUST_PORT_ADAPTER_COMMAND_DESCRIPTION: &str = "Prepare a Rust service adapter for an external service, vertical capability, native bridge, or backend integration behind a narrow typed interface.";
pub const BRAIN_RUST_STATE_STORE_COMMAND_DESCRIPTION: &str = "Prepare a Rust-owned durable state store for local service state, caches, indexes, credentials metadata, or persistent domain data with explicit schema and ownership.";
pub const BRAIN_SCIENCE_VISIBLE_CATALOG: &str = "SCIENCE_ENGINEERING_3D_BRAIN v1\nPurpose: active session catalog for scientific reasoning, mathematics, statistics, biology, chemistry, physics, cryptography, optimization, formal analysis, engineering, robotics, physical prototypes, CAD/3D conception, Banger objects, future Banger 3D work, Monster compute, simulation and verified design.\nUse /questionnaire_ when several clarifying questions are needed after this Science/Engineering/3D Brain is active; keep Canvas prose short and fill expert option presets.\nUse /newcompute_ for any heavy/verifiable compute: math, physics, biology models, chemistry calculations, cryptography analysis, optimization, simulation, numerical analysis, sizing, performance envelopes, statistical analysis or proof artifacts.\nUse /newobject_ for 3D/Banger object, geometry, SDF, CAD-like part, scene, material, assembly or computational design requests.\nUse /web_ or /googleweb_ only when current external research is needed for papers, standards, datasheets, components, protocols, biological references or scientific sources.\nUse /workspace_ only if actual local project/code/files are needed; scientific discussion, calculations, cryptography reasoning, biology reasoning, image work and 3D conception do not need a workspace.\nPrefer explicit assumptions, constraints, units, safety margins, verification plans, reproducible calculations and artifact/proof summaries.";
pub const BRAIN_CODING_VISIBLE_CATALOG: &str = "CODING_BRAIN v1\nPurpose: active session catalog for software engineering, repositories, debugging, implementation, tests, refactors, architecture, scripts, build systems, APIs, Rust, TypeScript, Electron and developer tooling.\nUse /questionnaire_ when several clarifying questions are needed after this Coding Brain is active; keep Canvas prose short and fill expert option presets.\nUse /workspace_ when local repository or filesystem access is required before coding, running tests or inspecting files.\nUse /searcharchive_ to recall prior project decisions, previous bugs, existing architecture notes or earlier implementation context.\nUse /newmodule_ when the user wants a new app/module integration or new product capability before broad implementation.\nUse /rust_port_adapter_ for external services, native bridges or backend adapters behind narrow Rust interfaces.\nUse /rust_state_store_ for durable state, caches, indexes, credentials metadata or persistent domain data.\nUse /newcompute_ only for heavy/verifiable computation inside coding workflows, not for ordinary code edits.\nDo not use /workspace_ for image generation/editing, Gmail, Airbnb or pure web research.";
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
    pub substitution_candidate_hash: Hash,
    pub verifier_hash: Hash,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainProofProjection {
    pub projection_hash: String,
    pub action: &'static str,
    pub accepted: bool,
    pub from: String,
    pub to: Option<String>,
    pub memory_hash: Option<String>,
    pub nodes_before: usize,
    pub nodes_after: usize,
    pub node_delta: isize,
    pub candidate_count: usize,
    pub samples: usize,
    pub frame_before: String,
    pub frame_after: String,
    pub substitution_candidate_hash: String,
    pub verifier_hash: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainSemanticSmokeProof {
    pub proof_hash: String,
    pub note_hash: String,
    pub evidence_hash: String,
    pub recall_hash: String,
    pub verification_hash: String,
    pub before_frame_hash: String,
    pub after_frame_hash: String,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainMemoryProofManifest {
    pub manifest_kind: &'static str,
    pub scope: String,
    pub memory_layer: String,
    pub text_hash: String,
    pub evidence_hash: String,
    pub semantic_identity_hash: String,
    pub recalled_identity_hashes: Vec<String>,
    pub recall_identity_hash: String,
    pub verification_hash: String,
    pub proof_hash: String,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainExperienceDigest {
    pub session_id: String,
    pub title: String,
    pub created_at: String,
    pub archived: bool,
    pub outcome: String,
    pub evidence_hash: String,
    pub codeact_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainCodeActTemplateSlot {
    pub name: String,
    pub required: bool,
    pub default_value: String,
    pub allowed_values: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainGeneralCodeActTemplate {
    pub command: String,
    pub section: String,
    pub purpose: String,
    pub slots: Vec<BrainCodeActTemplateSlot>,
    pub result_schema: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainSkillCandidate {
    pub skill_id: String,
    pub activation: String,
    pub codeact_command: String,
    pub procedure_steps: Vec<String>,
    pub avoid_patterns: Vec<String>,
    pub required_evidence_hashes: Vec<String>,
    pub source_session_ids: Vec<String>,
    pub previous_skill_hash: Option<String>,
    pub rollback_ref: String,
    pub user_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrainSkillPromotionProof {
    pub accepted: bool,
    pub candidate_hash: String,
    pub proof_hash: String,
    pub manifest_hash: Option<String>,
    pub skill_id: String,
    pub promoted_skill_ref: Option<String>,
    pub rollback_ref: String,
    pub session_index_hash: String,
    pub retrieval_keys: Vec<String>,
    pub reasons: Vec<String>,
}

impl BrainMemory {
    pub fn accepted(&self) -> bool {
        self.action == BrainAction::AcceptedSubstitution
    }

    pub fn proof_projection(&self) -> BrainProofProjection {
        let action = self.action.as_str();
        let from = self.from.as_hex();
        let to = self.to.map(|hash| hash.as_hex());
        let memory_hash = self.memory_hash.map(|hash| hash.as_hex());
        let frame_before = hex_bytes(&self.frame_before);
        let frame_after = hex_bytes(&self.frame_after);
        let substitution_candidate_hash = self.substitution_candidate_hash.as_hex();
        let verifier_hash = self.verifier_hash.as_hex();
        let node_delta = self.nodes_after as isize - self.nodes_before as isize;
        let canonical = format!(
            "forge-brain-proof-projection-v1\naction={action}\naccepted={}\nfrom={from}\nto={}\nmemory_hash={}\nnodes_before={}\nnodes_after={}\nnode_delta={node_delta}\ncandidate_count={}\nsamples={}\nframe_before={frame_before}\nframe_after={frame_after}\nsubstitution_candidate_hash={substitution_candidate_hash}\nverifier_hash={verifier_hash}\nreasons={}\n",
            self.accepted(),
            to.as_deref().unwrap_or(""),
            memory_hash.as_deref().unwrap_or(""),
            self.nodes_before,
            self.nodes_after,
            self.candidate_count,
            self.samples,
            self.reasons
                .iter()
                .map(|reason| sanitize_line(reason))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        BrainProofProjection {
            projection_hash: Hash::for_blob(canonical.as_bytes()).as_hex(),
            action,
            accepted: self.accepted(),
            from,
            to,
            memory_hash,
            nodes_before: self.nodes_before,
            nodes_after: self.nodes_after,
            node_delta,
            candidate_count: self.candidate_count,
            samples: self.samples,
            frame_before,
            frame_after,
            substitution_candidate_hash,
            verifier_hash,
            reasons: self.reasons.clone(),
        }
    }
}

pub fn semantic_note_smoke_proof(
    note_hash: &str,
    evidence_hash: &str,
    recall_hash: &str,
    recalled_note_hashes: &[String],
) -> BrainSemanticSmokeProof {
    let verification = crate::godel::verifier::verify_semantic_note_recall(
        note_hash,
        evidence_hash,
        recall_hash,
        recalled_note_hashes,
    );
    let canonical = format!(
        "forge-brain-semantic-smoke-v1\nnote_hash={}\nevidence_hash={}\nrecall_hash={}\nverification_hash={}\naccepted={}\nbefore_frame_hash={}\nafter_frame_hash={}\nreasons={}\n",
        sanitize_line(note_hash),
        sanitize_line(evidence_hash),
        sanitize_line(recall_hash),
        verification.verification_hash,
        verification.accepted,
        verification.before_frame_hash,
        verification.after_frame_hash,
        verification
            .reasons
            .iter()
            .map(|reason| sanitize_line(reason))
            .collect::<Vec<_>>()
            .join(" | ")
    );
    BrainSemanticSmokeProof {
        proof_hash: Hash::for_blob(canonical.as_bytes()).as_hex(),
        note_hash: note_hash.to_string(),
        evidence_hash: evidence_hash.to_string(),
        recall_hash: recall_hash.to_string(),
        verification_hash: verification.verification_hash,
        before_frame_hash: verification.before_frame_hash,
        after_frame_hash: verification.after_frame_hash,
        accepted: verification.accepted,
        reasons: verification.reasons,
    }
}

pub fn semantic_note_reusable_proof_manifest(
    scope: &str,
    memory_layer: &str,
    text_hash: &str,
    evidence_hash: &str,
    recalled_identity_hashes: &[String],
) -> BrainMemoryProofManifest {
    let scope = sanitize_line(scope);
    let memory_layer = sanitize_line(memory_layer);
    let text_hash = sanitize_line(text_hash);
    let evidence_hash = sanitize_line(evidence_hash);
    let semantic_identity_hash = Hash::for_blob(
        format!(
            "forge-brain-semantic-note-identity-v1\nscope={scope}\nmemory_layer={memory_layer}\ntext_hash={text_hash}\nevidence_hash={evidence_hash}\n"
        )
        .as_bytes(),
    )
    .as_hex();
    let mut recalled_identity_hashes = recalled_identity_hashes
        .iter()
        .map(|value| sanitize_line(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    recalled_identity_hashes.sort();
    recalled_identity_hashes.dedup();
    let recall_identity_hash = Hash::for_blob(
        format!(
            "forge-brain-semantic-recall-set-v1\ncount={}\n{}\n",
            recalled_identity_hashes.len(),
            recalled_identity_hashes
                .iter()
                .enumerate()
                .map(|(idx, value)| format!("identity_{idx}={value}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
        .as_bytes(),
    )
    .as_hex();
    let accepted = recalled_identity_hashes
        .iter()
        .any(|value| value == &semantic_identity_hash);
    let reasons = if accepted {
        Vec::new()
    } else {
        vec!["semantic identity missing from bounded recall set".to_string()]
    };
    let verification_hash = Hash::for_blob(
        format!(
            "forge-brain-reusable-proof-verification-v1\nsemantic_identity_hash={semantic_identity_hash}\nrecall_identity_hash={recall_identity_hash}\naccepted={accepted}\nreasons={}\n",
            reasons
                .iter()
                .map(|reason| sanitize_line(reason))
                .collect::<Vec<_>>()
                .join(" | ")
        )
        .as_bytes(),
    )
    .as_hex();
    let proof_hash = Hash::for_blob(
        format!(
            "forge-brain-reusable-proof-manifest-v1\nscope={scope}\nmemory_layer={memory_layer}\ntext_hash={text_hash}\nevidence_hash={evidence_hash}\nsemantic_identity_hash={semantic_identity_hash}\nrecall_identity_hash={recall_identity_hash}\nverification_hash={verification_hash}\naccepted={accepted}\nreasons={}\n",
            reasons
                .iter()
                .map(|reason| sanitize_line(reason))
                .collect::<Vec<_>>()
                .join(" | ")
        )
        .as_bytes(),
    )
    .as_hex();
    BrainMemoryProofManifest {
        manifest_kind: "forge_brain_memory_proof_manifest_v0",
        scope,
        memory_layer,
        text_hash,
        evidence_hash,
        semantic_identity_hash,
        recalled_identity_hashes,
        recall_identity_hash,
        verification_hash,
        proof_hash,
        accepted,
        reasons,
    }
}

pub fn verify_brain_skill_promotion(
    candidate: &BrainSkillCandidate,
    sessions: &[BrainExperienceDigest],
) -> BrainSkillPromotionProof {
    let mut reasons = Vec::new();
    let canonical_candidate = canonical_skill_candidate(candidate);
    let candidate_hash = Hash::for_blob(canonical_candidate.as_bytes()).as_hex();
    let session_index_hash = brain_session_index_hash(sessions);
    let retrieval_keys = brain_skill_retrieval_keys(candidate);

    if sanitize_ref_segment(&candidate.skill_id).is_empty() {
        reasons.push("skill id is empty or cannot be used as a ref".to_string());
    }
    if candidate.activation.trim().len() < 8 {
        reasons.push("activation must describe a concrete future task pattern".to_string());
    }
    if !candidate.codeact_command.trim().starts_with('/') {
        reasons.push("CodeAct command must start with '/'".to_string());
    }
    if candidate.procedure_steps.len() < 2 {
        reasons.push("procedure needs at least two reusable steps".to_string());
    }
    if candidate.required_evidence_hashes.is_empty() {
        reasons.push("promotion requires at least one evidence hash".to_string());
    }
    if candidate.source_session_ids.is_empty() {
        reasons.push("promotion requires at least one source session".to_string());
    }
    if candidate.rollback_ref.trim().is_empty() {
        reasons.push("rollback ref is required before promotion".to_string());
    }

    let mut session_ids = BTreeSet::new();
    let mut evidence_hashes = BTreeSet::new();
    let mut verified_sources = 0usize;
    for session in sessions {
        session_ids.insert(sanitize_line(&session.session_id));
        if !session.evidence_hash.trim().is_empty() {
            evidence_hashes.insert(sanitize_line(&session.evidence_hash));
        }
        let outcome = session.outcome.to_ascii_lowercase();
        if outcome.contains("verified") || outcome.contains("passed") || outcome.contains("success") {
            verified_sources += 1;
        }
    }

    for source in &candidate.source_session_ids {
        if !session_ids.contains(&sanitize_line(source)) {
            reasons.push(format!("source session '{}' is not in recents/archive ledger", sanitize_line(source)));
        }
    }
    for evidence in &candidate.required_evidence_hashes {
        if !evidence_hashes.contains(&sanitize_line(evidence)) {
            reasons.push(format!("evidence hash '{}' was not found in source sessions", sanitize_line(evidence)));
        }
    }
    if verified_sources == 0 {
        reasons.push("no verified or successful source session supports this procedure".to_string());
    }

    let protected_text = format!(
        "{}\n{}\n{}\n{}",
        candidate.activation,
        candidate.codeact_command,
        candidate.procedure_steps.join("\n"),
        candidate.user_note
    )
    .to_ascii_lowercase();
    for needle in [
        "api_key",
        "secret",
        "password",
        "token=",
        "credential",
        "bypass",
        "disable godel",
        "ignore agens",
        "ignore agents",
    ] {
        if protected_text.contains(needle) {
            reasons.push(format!("unsafe protected text detected: {needle}"));
        }
    }

    let accepted = reasons.is_empty();
    let promoted_skill_ref = if accepted {
        Some(format!(
            "{BRAIN_SKILL_REF_PREFIX}{}",
            sanitize_ref_segment(&candidate.skill_id)
        ))
    } else {
        None
    };
    let proof_hash = Hash::for_blob(
        canonical_skill_promotion_verification(
            accepted,
            &candidate_hash,
            &session_index_hash,
            &retrieval_keys,
            &reasons,
        )
        .as_bytes(),
    )
    .as_hex();

    BrainSkillPromotionProof {
        accepted,
        candidate_hash,
        proof_hash,
        manifest_hash: None,
        skill_id: sanitize_line(&candidate.skill_id),
        promoted_skill_ref,
        rollback_ref: sanitize_line(&candidate.rollback_ref),
        session_index_hash,
        retrieval_keys,
        reasons,
    }
}

pub fn publish_brain_skill_promotion(
    node: &MonsterNode,
    candidate: &BrainSkillCandidate,
    sessions: &[BrainExperienceDigest],
) -> Result<BrainSkillPromotionProof, BrainError> {
    let mut proof = verify_brain_skill_promotion(candidate, sessions);
    let manifest = encode_brain_skill_promotion(candidate, sessions, &proof);
    let manifest_hash = node.store().store(manifest.as_bytes())?;
    node.store().write_ref(
        &format!(
            "{BRAIN_SKILL_PROMOTION_REF_PREFIX}{}",
            proof.candidate_hash
        ),
        &manifest_hash,
        "brain skill promotion proof",
    )?;
    if proof.accepted {
        if let Some(skill_ref) = &proof.promoted_skill_ref {
            node.store()
                .write_ref(skill_ref, &manifest_hash, "brain promoted CodeAct skill")?;
        }
    }
    proof.manifest_hash = Some(manifest_hash.as_hex());
    Ok(proof)
}

pub fn brain_general_codeact_templates() -> Vec<BrainGeneralCodeActTemplate> {
    vec![
        brain_searcharchive_codeact_template(),
        brain_rename_session_codeact_template(),
        brain_googleweb_codeact_template(),
        brain_maps_codeact_template(),
        brain_gmail_codeact_template(),
        brain_gmail_com_codeact_template(),
        brain_airbnb_codeact_template(),
        brain_newimage_codeact_template(),
        brain_editimage_codeact_template(),
        brain_science_codeact_template(),
        brain_coding_codeact_template(),
    ]
}

pub fn brain_searcharchive_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_SEARCHARCHIVE_COMMAND.to_string(),
        section: "general".to_string(),
        purpose: "Search recent and archived session history by exact context, returning bounded snippets, neighbor turns, attachment refs and proof hashes without app-side reasoning.".to_string(),
        result_schema: BRAIN_SEARCHARCHIVE_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "query".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Keyword, phrase, filename or metric text to find in archived session records.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "scope".to_string(),
                required: false,
                default_value: "all".to_string(),
                allowed_values: vec!["recent".to_string(), "archived".to_string(), "all".to_string()],
                description: "Choose recent sessions, archived sessions or both.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "targets".to_string(),
                required: false,
                default_value: "session_text|attachments|files|computes|proofs|metrics".to_string(),
                allowed_values: vec![
                    "session_text".to_string(),
                    "attachments".to_string(),
                    "files".to_string(),
                    "computes".to_string(),
                    "proofs".to_string(),
                    "metrics".to_string(),
                ],
                description: "Archive fields to search; implementations may ignore unsupported targets but must stay read-only.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "top_k".to_string(),
                required: false,
                default_value: "5".to_string(),
                allowed_values: Vec::new(),
                description: "Maximum ranked hits to return; bounded by the host.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "context_window".to_string(),
                required: false,
                default_value: "turns:1".to_string(),
                allowed_values: Vec::new(),
                description: "Neighbor turns before and after each match; bounded by the host to avoid token explosions.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "snippets".to_string(),
                allowed_values: vec![
                    "snippets".to_string(),
                    "manifest".to_string(),
                    "restore_context".to_string(),
                ],
                description: "Result mode; default returns snippets and refs, restore_context may fetch more in a second bounded call.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_rename_session_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_RENAME_SESSION_COMMAND.to_string(),
        section: "general".to_string(),
        purpose: "Rename the current session from the LLM's short, product-quality title for the first user message topic. Preferred activation is /\"nomduchat\"_renamechat_; the app uses the quoted nomduchat field as the sidebar title and keeps Brain archive history aligned without visible rename chatter.".to_string(),
        result_schema: BRAIN_RENAME_SESSION_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "title".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Short relevant session title used as nomduchat in /\"nomduchat\"_renamechat_: 2-5 natural words, nominal and specific like Codex or Claude. Do not copy the prompt and do not use only a proper noun.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "reason".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Optional compact reason for the title choice; not displayed in the sidebar.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_googleweb_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_GOOGLEWEB_COMMAND.to_string(),
        section: "webexplorer".to_string(),
        purpose: "Open the contained native WebExplorer on a Google search while the assistant keeps conversing in the left transcript; keywords make the web intent explicit and verifiable.".to_string(),
        result_schema: BRAIN_GOOGLEWEB_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "query".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Primary natural-language search topic, for example \"ville de Kagoshima\".".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "keywords".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Comma-separated special keywords that must be included in the Google query, such as history, tourism, population, official sources or locale words.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "locale".to_string(),
                required: false,
                default_value: "fr".to_string(),
                allowed_values: vec!["fr".to_string(), "en".to_string(), "ja".to_string()],
                description: "Preferred Google UI/search language for the first navigation.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "open_mode".to_string(),
                required: false,
                default_value: "split_webexplorer".to_string(),
                allowed_values: vec!["split_webexplorer".to_string()],
                description: "Open the contained WebExplorer split; never replace the global product shell.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_navigation".to_string(),
                allowed_values: vec!["conversation_and_navigation".to_string(), "navigation_only".to_string()],
                description: "Default writes a bounded assistant status into the left transcript and navigates the WebExplorer on the right.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_maps_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_MAPS_COMMAND.to_string(),
        section: "webexplorer".to_string(),
        purpose: "Open the contained native WebExplorer on Google Earth while the assistant keeps conversing in the left transcript. Use this for every detected geographic place: cities, places, countries, regions, local weather at a place, where-is questions, maps, routes, coordinates, Google Earth, or ordinary geographic context. For geography alone, this is the only WebExplorer page. When the same place appears with voyage, vacances, partir, visiter, tourisme, sejour, destination, dates, guests, lodging, accommodation, booking, or stay-search intent, emit this /maps_ command first and then /airbnb_ as the next WebExplorer page. Default uses the Brain home city when no target is specified; optional latitude and longitude slots let the LLM target explicit WGS84 coordinates. Device location must never be read silently; current-location flows require explicit user permission before any coordinate is used.".to_string(),
        result_schema: BRAIN_MAPS_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "target".to_string(),
                required: false,
                default_value: "default_google_earth_view".to_string(),
                allowed_values: Vec::new(),
                description: "Human-readable city, place, country, region, weather-location, route, map, or coordinate target preserved in the proof. Leave default_google_earth_view when the user only asks to open Maps/Earth or when the Brain home city should be used.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "latitude".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Optional decimal WGS84 latitude in [-90, 90]. Only use when the user supplied coordinates or explicitly approved device-location access.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "longitude".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Optional decimal WGS84 longitude in [-180, 180]. Only use when the user supplied coordinates or explicitly approved device-location access.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "open_mode".to_string(),
                required: false,
                default_value: "split_webexplorer".to_string(),
                allowed_values: vec!["split_webexplorer".to_string()],
                description: "Open Google Earth inside the contained WebExplorer split; never replace the global product shell.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_navigation".to_string(),
                allowed_values: vec!["conversation_and_navigation".to_string(), "navigation_only".to_string()],
                description: "Default preserves the LLM-authored natural sentence in the left transcript and navigates Google Earth on the right.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_gmail_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_GMAIL_COMMAND.to_string(),
        section: "webexplorer".to_string(),
        purpose: "Open the contained WebExplorer on Gmail with a bounded mail intent: search, inspect, summarize, draft or prepare a reply. The LLM must write its own natural user-facing sentence adapted to the user's request; the host must not synthesize that sentence. The LLM then activates this CodeAct with explicit slots, and the application renders the action event automatically. The LLM chooses the intent; the host only executes the explicit CodeAct.".to_string(),
        result_schema: BRAIN_GMAIL_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "intent".to_string(),
                required: true,
                default_value: "search".to_string(),
                allowed_values: vec![
                    "open".to_string(),
                    "search".to_string(),
                    "inspect".to_string(),
                    "summarize".to_string(),
                    "draft".to_string(),
                    "reply".to_string(),
                ],
                description: "Mail action requested by the LLM. Sending must remain gated by user approval; draft/reply prepares text only.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "query".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Gmail search query using official operators when useful: from:, to:, subject:, after:, before:, newer:, older:, has:attachment, filename:, label:, category:, is:unread/read.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "keywords".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Comma-separated natural keywords the LLM wants preserved in the Gmail search or draft context.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "recipient".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Email recipient for draft/reply intents; never send automatically.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "subject".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Subject filter for search or proposed subject for a draft.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "body".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Draft or reply body prepared by the LLM; host must keep it editable and user-approved before send.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "open_mode".to_string(),
                required: false,
                default_value: "split_webexplorer".to_string(),
                allowed_values: vec!["split_webexplorer".to_string()],
                description: "Open Gmail inside the contained WebExplorer split; never replace the global product shell.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_navigation".to_string(),
                allowed_values: vec![
                    "conversation_and_navigation".to_string(),
                    "navigation_only".to_string(),
                    "draft_manifest".to_string(),
                ],
                description: "Default preserves the LLM-authored natural sentence in the left transcript and navigates Gmail on the right; the app must not generate a replacement phrase.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_gmail_com_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_GMAIL_COM_COMMAND.to_string(),
        section: "webexplorer".to_string(),
        purpose: "Open the contained split-screen WebExplorer on the Gmail Google Accounts sign-in URL. The LLM must write its own natural user-facing sentence adapted to the user's request; the host must not synthesize that sentence. The LLM then activates this CodeAct, and the application renders the action event automatically.".to_string(),
        result_schema: BRAIN_GMAIL_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![BrainCodeActTemplateSlot {
            name: "open_mode".to_string(),
            required: false,
            default_value: "split_webexplorer".to_string(),
            allowed_values: vec!["split_webexplorer".to_string()],
            description: "Open Gmail inside the contained WebExplorer split; never replace the global product shell and never generate a fixed user-facing phrase in the app.".to_string(),
        }],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_airbnb_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_AIRBNB_COMMAND.to_string(),
        section: "webexplorer".to_string(),
        purpose: "Open the contained split-screen WebExplorer on Airbnb.com after /maps_. Use this command when a geographic place is detected together with travel/vacation/stay language: voyage, vacances, partir, visiter, tourisme, sejour, destination, dates, guests, lodging, accommodation, hotel-like stay, home/apartment rental, vacation rental, booking, budget for stays, or short-term stay search. Do not use it for city facts, local weather, geography, maps, routes, or ordinary geographic context without travel/vacation/stay intent; those belong to /maps_ only. The LLM must write its own natural user-facing sentence adapted to the user's request before activating the command, and must also copy that same LLM-authored sentence into the say slot so command-only render paths still show a normal answer. The host must not synthesize that sentence. The LLM then activates this CodeAct, and the application renders the action event automatically.".to_string(),
        result_schema: BRAIN_AIRBNB_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "intent".to_string(),
                required: false,
                default_value: "open".to_string(),
                allowed_values: vec!["open".to_string(), "search".to_string(), "inspect".to_string()],
                description: "Airbnb action requested by the LLM. Open navigates to Airbnb.com; search preserves a destination/query for the Airbnb surface.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "say".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Required LLM-authored natural sentence shown to the user. It must be specific to the request and must never be generated by the host app.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "query".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Optional Airbnb destination, stay query, listing hint or travel context to preserve in the navigation proof.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "keywords".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Comma-separated travel keywords the LLM wants preserved for Airbnb context, such as city, dates, guests or stay type.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "open_mode".to_string(),
                required: false,
                default_value: "split_webexplorer".to_string(),
                allowed_values: vec!["split_webexplorer".to_string()],
                description: "Open Airbnb inside the contained WebExplorer split; never replace the global product shell and never generate a fixed user-facing phrase in the app.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_navigation".to_string(),
                allowed_values: vec!["conversation_and_navigation".to_string(), "navigation_only".to_string()],
                description: "Default preserves the LLM-authored natural sentence in the left transcript and navigates Airbnb on the right; the app must not generate a replacement phrase.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_newimage_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_NEWIMAGE_COMMAND.to_string(),
        section: "image".to_string(),
        purpose: "Generate a new image when the user asks to create, generate, draw, render or imagine an image. The LLM must write its own natural user-facing sentence adapted to the request before activating the command, and must also copy that same sentence into the say slot. The host must not synthesize that sentence. Use the user's natural prompt as the image prompt; do not ask for confirmation unless the request is ambiguous or unsafe.".to_string(),
        result_schema: BRAIN_IMAGE_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "say".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Required LLM-authored natural sentence shown to the user before image generation.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "prompt".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Detailed image prompt written by the LLM from the user's request.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "size".to_string(),
                required: false,
                default_value: "auto".to_string(),
                allowed_values: vec!["auto".to_string(), "1024x1024".to_string(), "1024x1536".to_string(), "1536x1024".to_string()],
                description: "Requested output size or auto when the user did not specify format.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "quality".to_string(),
                required: false,
                default_value: "auto".to_string(),
                allowed_values: vec!["auto".to_string(), "low".to_string(), "medium".to_string(), "high".to_string()],
                description: "Image quality target. Keep auto unless the user asks for fast draft or high quality.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "background".to_string(),
                required: false,
                default_value: "auto".to_string(),
                allowed_values: vec!["auto".to_string(), "opaque".to_string(), "transparent".to_string()],
                description: "Background mode; transparent only when the user asks for a cutout, sticker, logo or transparent asset.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_image".to_string(),
                allowed_values: vec!["conversation_and_image".to_string(), "image_only".to_string()],
                description: "Default preserves the LLM-authored natural sentence and renders the generated image artifact.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_editimage_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_EDITIMAGE_COMMAND.to_string(),
        section: "image".to_string(),
        purpose: "Edit, retouch, replace, transform, restyle, remove or add details to an image when the user asks to modify an attached image, a selected image, or the last visible image in the conversation. Follow-up requests such as remove an object, change the background, clean up, upscale, crop, recolor or restyle must activate this command when any image was attached/selected earlier in the session. The LLM must write its own natural user-facing sentence adapted to the requested edit before activating the command, and must also copy that same sentence into the say slot. The host must not synthesize that sentence. Do not answer with a prompt for an external tool. Do not emit /workspace_ for image editing. If no editable image is available, ask which image to modify instead of activating the command.".to_string(),
        result_schema: BRAIN_IMAGE_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "say".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Required LLM-authored natural sentence shown to the user before editing.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "instruction".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Precise edit instruction derived from the user's text.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "image_ref".to_string(),
                required: true,
                default_value: "attached_or_selected_image".to_string(),
                allowed_values: Vec::new(),
                description: "ID, filename, or natural reference for the image already attached/selected in the composer or conversation.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "mask_ref".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Optional mask image or region reference when the user limits the edit to part of the image.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "size".to_string(),
                required: false,
                default_value: "auto".to_string(),
                allowed_values: vec!["auto".to_string(), "1024x1024".to_string(), "1024x1536".to_string(), "1536x1024".to_string()],
                description: "Requested output size or auto when the user wants to preserve the input format.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "conversation_and_image".to_string(),
                allowed_values: vec!["conversation_and_image".to_string(), "image_only".to_string()],
                description: "Default preserves the LLM-authored natural sentence and renders the edited image artifact.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_questionnaire_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_QUESTIONNAIRE_COMMAND.to_string(),
        section: "conversation".to_string(),
        purpose: "Open a paginated questionnaire surface above the composer when the LLM needs several clarifying questions before acting. Keep the Canvas answer short, then activate this CodeAct with one question per q1..q5 slot, five questions maximum, and three expert option cards per question in q1_options..q5_options, separated by |. Each card must be context-specific and documented: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`. Use concise tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when they clarify the choice. For color-choice questions, add a bounded color preview token inside the option label, for example `color:#38bdf8` for one universal CSS color or `colors:#38bdf8,#a855f7` for a diffuse gradient; never put arbitrary CSS or JS in a questionnaire option. Mark the strongest default with `(Recommended)` when appropriate, and include one quality/ambition-first option when useful. Never write vague meta choices like Option 1, compare several pistes, I do not know yet, or generic one-word answers. The host always renders a fourth Other option with a free-text field on every page. Use this for planning, engineering, coding, travel, image work or any task where a long question list would otherwise pollute the transcript.".to_string(),
        result_schema: BRAIN_QUESTIONNAIRE_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "title".to_string(),
                required: false,
                default_value: "Questions".to_string(),
                allowed_values: Vec::new(),
                description: "Short title for the questionnaire surface, for example \"Cadrage du prototype\".".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "intro".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Two or three concise sentences framing the user's project goal, current uncertainty, and why these answers matter. Make the purpose of the questionnaire understandable without writing a long checklist.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q1".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "First clarifying question shown as page 1 of the questionnaire. Do not put the choices in this slot; put them in q1_options.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q1_options".to_string(),
                required: true,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Exactly three context-specific expert option cards for q1, separated by |. Format: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`; use tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when helpful. For color choices, add `color:#hex` or `colors:#hex,#hex` inside the option label so the host renders a diffuse color preview. Mark the best default with `(Recommended)` when appropriate, and include one quality/ambition-first option when useful. Never use Option 1/2/3, vague meta choices, or generic one-word answers. Do not include Other; the host adds it.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q2".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Second clarifying question shown as page 2 when needed. Do not put the choices in this slot; put them in q2_options.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q2_options".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Exactly three context-specific expert option cards for q2, separated by |. Format: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`; use tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when helpful. Mark the best default with `(Recommended)` when appropriate. Never use Option 1/2/3, vague meta choices, or generic one-word answers. Do not include Other.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q3".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Third clarifying question shown as page 3 when needed. Do not put the choices in this slot; put them in q3_options.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q3_options".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Exactly three context-specific expert option cards for q3, separated by |. Format: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`; use tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when helpful. Include one clearly superior-quality or more ambitious option when useful. Never use Option 1/2/3, vague meta choices, or generic one-word answers. Do not include Other.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q4".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Fourth clarifying question shown as page 4 when truly needed; avoid more unless the task requires it. Do not put the choices in this slot; put them in q4_options.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q4_options".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Exactly three context-specific expert option cards for q4, separated by |. Format: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`; use tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when helpful. Avoid vague or one-word answers. Never use Option 1/2/3. Do not include Other.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q5".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Fifth and final clarifying question shown as page 5 only when truly necessary. Never ask more than five questions. Do not put the choices in this slot; put them in q5_options.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "q5_options".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Exactly three context-specific expert option cards for q5, separated by |. Format: `Label (Tag) - 1-2 useful sentences explaining benefit, tradeoff, and when to choose it`; use tags such as Recommended, Fast, Quality, Ambitious, Cheaper, Safer or Riskier when helpful. Avoid vague or one-word answers. Never use Option 1/2/3. Do not include Other.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "mode".to_string(),
                required: false,
                default_value: "paginated_options".to_string(),
                allowed_values: vec!["paginated_options".to_string()],
                description: "Render one question per page with three LLM-provided options and a fourth Other free-text option.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "composer_questionnaire".to_string(),
                allowed_values: vec!["composer_questionnaire".to_string()],
                description: "The host renders a discreet questionnaire above the composer and hides CodeAct metadata from the Canvas.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_science_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_SCIENCE_COMMAND.to_string(),
        section: "brain_segment".to_string(),
        purpose: "Activate /sciencebrain_ when the LLM understands that the user's task belongs to the Science/Engineering/3D Brain: science, engineering, mathematics, biology, chemistry, physics, cryptography, optimization, formal analysis, physical product design, electronics, mechanics, robotics, CAD/3D, Banger, Monster, /newcompute_ or future Banger 3D work. The LLM activates this before choosing specialized science, engineering, compute or 3D commands. The host injects only this segment catalog after the LLM has activated the command; the app must not perform semantic routing before the LLM.".to_string(),
        result_schema: BRAIN_SEGMENT_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "segment".to_string(),
                required: true,
                default_value: "science".to_string(),
                allowed_values: vec!["science".to_string()],
                description: "Target Brain segment to activate for the rest of the session.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "reason".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Short LLM-authored reason for activating /sciencebrain_, derived from the user request.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "inject_brain_catalog".to_string(),
                allowed_values: vec!["inject_brain_catalog".to_string()],
                description: "The host injects the Science/Engineering/3D Brain visible catalog and renders a special /sciencebrain_ event.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
}

pub fn brain_coding_codeact_template() -> BrainGeneralCodeActTemplate {
    let mut template = BrainGeneralCodeActTemplate {
        command: BRAIN_CODING_COMMAND.to_string(),
        section: "brain_segment".to_string(),
        purpose: "Activate /codingbrain_ when the LLM understands that the user's task belongs to the Coding Brain: software engineering, coding, websites, applications, repository work, debugging, refactoring, tests, architecture, scripts, Rust, TypeScript, Electron, APIs, build systems, or developer tooling. The LLM activates this before choosing coding-specific commands such as workspace, archive recall, module creation, adapters or state stores. The host injects only this segment catalog after the LLM has activated the command; the app must not perform semantic routing before the LLM.".to_string(),
        result_schema: BRAIN_SEGMENT_RESULT_SCHEMA.to_string(),
        proof_hash: String::new(),
        slots: vec![
            BrainCodeActTemplateSlot {
                name: "segment".to_string(),
                required: true,
                default_value: "coding".to_string(),
                allowed_values: vec!["coding".to_string()],
                description: "Target Brain segment to activate for the rest of the session.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "reason".to_string(),
                required: false,
                default_value: String::new(),
                allowed_values: Vec::new(),
                description: "Short LLM-authored reason for activating /codingbrain_, derived from the user request.".to_string(),
            },
            BrainCodeActTemplateSlot {
                name: "output".to_string(),
                required: false,
                default_value: "inject_brain_catalog".to_string(),
                allowed_values: vec!["inject_brain_catalog".to_string()],
                description: "The host injects the Coding Brain visible catalog and renders a special /codingbrain_ event.".to_string(),
            },
        ],
    };
    template.proof_hash = Hash::for_blob(canonical_brain_general_codeact_template(&template).as_bytes()).as_hex();
    template
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
            let substitution_candidate_hash = substitution_candidate_hash(from, &[]);
            let verifier_hash = substitution_verifier_hash(
                BrainAction::AlreadyTight,
                from,
                None,
                substitution_candidate_hash,
                self.samples,
                frame_before,
                frame_after,
                &[],
            );
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
                    substitution_candidate_hash,
                    verifier_hash,
                    reasons: Vec::new(),
                },
            );
        }

        let mut rejected_reasons = Vec::new();
        let mut attempted_candidates = Vec::new();
        for candidate in candidates {
            let to = node.store().store(candidate.bytes())?;
            attempted_candidates.push(to);
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
                    let substitution_candidate_hash = substitution_candidate_hash(from, &[to]);
                    let verifier_hash = substitution_verifier_hash(
                        BrainAction::AcceptedSubstitution,
                        from,
                        Some(to),
                        substitution_candidate_hash,
                        verification_samples,
                        frame_before,
                        frame_after,
                        &[],
                    );
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
                            substitution_candidate_hash,
                            verifier_hash,
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
        let substitution_candidate_hash = substitution_candidate_hash(from, &attempted_candidates);
        let verifier_hash = substitution_verifier_hash(
            BrainAction::RejectedCandidate,
            from,
            None,
            substitution_candidate_hash,
            strict_equiv_samples(self.samples),
            frame_before,
            frame_after,
            &rejected_reasons,
        );
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
                substitution_candidate_hash,
                verifier_hash,
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
            let substitution_candidate_hash = substitution_candidate_hash(from, &[to]);
            let verifier_hash = substitution_verifier_hash(
                BrainAction::AcceptedSubstitution,
                from,
                Some(to),
                substitution_candidate_hash,
                samples,
                frame_before,
                frame_after,
                &[],
            );
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
                substitution_candidate_hash,
                verifier_hash,
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

fn substitution_candidate_hash(from: Hash, candidates: &[Hash]) -> Hash {
    let mut canonical = format!(
        "forge-brain-substitution-candidate-v1\nfrom={}\ncandidate_count={}\n",
        from.as_hex(),
        candidates.len()
    );
    for (idx, candidate) in candidates.iter().enumerate() {
        canonical.push_str(&format!("candidate_{idx}={}\n", candidate.as_hex()));
    }
    Hash::for_blob(canonical.as_bytes())
}

fn substitution_verifier_hash(
    action: BrainAction,
    from: Hash,
    to: Option<Hash>,
    substitution_candidate_hash: Hash,
    samples: usize,
    frame_before: [u8; 32],
    frame_after: [u8; 32],
    reasons: &[String],
) -> Hash {
    let mut canonical = format!(
        "forge-brain-substitution-verifier-v1\naction={}\nfrom={}\nto={}\nsubstitution_candidate_hash={}\nsamples={}\nframe_before={}\nframe_after={}\n",
        action.as_str(),
        from.as_hex(),
        to.map(|hash| hash.as_hex()).unwrap_or_default(),
        substitution_candidate_hash.as_hex(),
        samples,
        hex_bytes(&frame_before),
        hex_bytes(&frame_after),
    );
    if reasons.is_empty() {
        canonical.push_str("reasons=\n");
    } else {
        canonical.push_str("reasons=");
        canonical.push_str(
            &reasons
                .iter()
                .map(|reason| sanitize_line(reason))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        canonical.push('\n');
    }
    Hash::for_blob(canonical.as_bytes())
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
    push_hash_line(
        &mut out,
        "substitution_candidate_hash",
        Some(memory.substitution_candidate_hash),
    );
    push_hash_line(&mut out, "verifier_hash", Some(memory.verifier_hash));
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

fn canonical_skill_candidate(candidate: &BrainSkillCandidate) -> String {
    let mut out = String::new();
    out.push_str("forge-brain-codeact-skill-candidate-v1\n");
    out.push_str(&format!("skill_id={}\n", sanitize_line(&candidate.skill_id)));
    out.push_str(&format!("activation={}\n", sanitize_line(&candidate.activation)));
    out.push_str(&format!(
        "codeact_command={}\n",
        sanitize_line(&candidate.codeact_command)
    ));
    push_sorted_lines(&mut out, "step", &candidate.procedure_steps);
    push_sorted_lines(&mut out, "avoid", &candidate.avoid_patterns);
    push_sorted_lines(
        &mut out,
        "evidence",
        &candidate.required_evidence_hashes,
    );
    push_sorted_lines(&mut out, "source_session", &candidate.source_session_ids);
    out.push_str(&format!(
        "previous_skill_hash={}\n",
        candidate
            .previous_skill_hash
            .as_deref()
            .map(sanitize_line)
            .unwrap_or_default()
    ));
    out.push_str(&format!("rollback_ref={}\n", sanitize_line(&candidate.rollback_ref)));
    out.push_str(&format!("user_note={}\n", sanitize_line(&candidate.user_note)));
    out
}

fn canonical_brain_general_codeact_template(template: &BrainGeneralCodeActTemplate) -> String {
    let mut out = String::new();
    out.push_str("forge-brain-general-codeact-template-v1\n");
    out.push_str(&format!("command={}\n", sanitize_line(&template.command)));
    out.push_str(&format!("section={}\n", sanitize_line(&template.section)));
    out.push_str(&format!("purpose={}\n", sanitize_line(&template.purpose)));
    out.push_str(&format!(
        "result_schema={}\n",
        sanitize_line(&template.result_schema)
    ));
    for slot in &template.slots {
        let mut allowed_values = slot
            .allowed_values
            .iter()
            .map(|value| sanitize_line(value))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        allowed_values.sort();
        allowed_values.dedup();
        out.push_str(&format!(
            "slot={} required={} default={} allowed={} description={}\n",
            sanitize_line(&slot.name),
            slot.required,
            sanitize_line(&slot.default_value),
            allowed_values.join("|"),
            sanitize_line(&slot.description)
        ));
    }
    out
}

fn canonical_skill_promotion_verification(
    accepted: bool,
    candidate_hash: &str,
    session_index_hash: &str,
    retrieval_keys: &[String],
    reasons: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("forge-brain-codeact-skill-promotion-proof-v1\n");
    out.push_str(&format!("accepted={accepted}\n"));
    out.push_str(&format!("candidate_hash={}\n", sanitize_line(candidate_hash)));
    out.push_str(&format!(
        "session_index_hash={}\n",
        sanitize_line(session_index_hash)
    ));
    push_sorted_lines(&mut out, "retrieval_key", retrieval_keys);
    push_sorted_lines(&mut out, "reason", reasons);
    out
}

fn encode_brain_skill_promotion(
    candidate: &BrainSkillCandidate,
    sessions: &[BrainExperienceDigest],
    proof: &BrainSkillPromotionProof,
) -> String {
    let mut out = String::new();
    out.push_str("forge-brain-codeact-skill-promotion-manifest-v1\n");
    out.push_str(&format!("accepted={}\n", proof.accepted));
    out.push_str(&format!("skill_id={}\n", sanitize_line(&proof.skill_id)));
    out.push_str(&format!("candidate_hash={}\n", proof.candidate_hash));
    out.push_str(&format!("proof_hash={}\n", proof.proof_hash));
    out.push_str(&format!("session_index_hash={}\n", proof.session_index_hash));
    out.push_str(&format!("rollback_ref={}\n", sanitize_line(&proof.rollback_ref)));
    if let Some(skill_ref) = &proof.promoted_skill_ref {
        out.push_str(&format!("promoted_skill_ref={}\n", sanitize_line(skill_ref)));
    } else {
        out.push_str("promoted_skill_ref=\n");
    }
    out.push_str("---candidate---\n");
    out.push_str(&canonical_skill_candidate(candidate));
    out.push_str("---sessions---\n");
    for session in sessions {
        out.push_str(&format!(
            "session={} title={} created_at={} archived={} outcome={} evidence_hash={}\n",
            sanitize_line(&session.session_id),
            sanitize_line(&session.title),
            sanitize_line(&session.created_at),
            session.archived,
            sanitize_line(&session.outcome),
            sanitize_line(&session.evidence_hash),
        ));
    }
    out.push_str("---retrieval---\n");
    push_sorted_lines(&mut out, "retrieval_key", &proof.retrieval_keys);
    out.push_str("---reasons---\n");
    push_sorted_lines(&mut out, "reason", &proof.reasons);
    out
}

fn brain_session_index_hash(sessions: &[BrainExperienceDigest]) -> String {
    let mut rows = sessions
        .iter()
        .map(|session| {
            let mut refs = session
                .codeact_refs
                .iter()
                .map(|value| sanitize_line(value))
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>();
            refs.sort();
            refs.dedup();
            format!(
                "session={} title={} created_at={} archived={} outcome={} evidence_hash={} refs={}",
                sanitize_line(&session.session_id),
                sanitize_line(&session.title),
                sanitize_line(&session.created_at),
                session.archived,
                sanitize_line(&session.outcome),
                sanitize_line(&session.evidence_hash),
                refs.join(",")
            )
        })
        .collect::<Vec<_>>();
    rows.sort();
    Hash::for_blob(
        format!(
            "forge-brain-session-ledger-v1\ncount={}\n{}\n",
            rows.len(),
            rows.join("\n")
        )
        .as_bytes(),
    )
    .as_hex()
}

fn brain_skill_retrieval_keys(candidate: &BrainSkillCandidate) -> Vec<String> {
    let mut keys = Vec::new();
    keys.push(sanitize_line(&candidate.codeact_command));
    for word in candidate
        .activation
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
    {
        let word = word.trim().to_ascii_lowercase();
        if word.len() >= 4 {
            keys.push(word);
        }
    }
    for step in &candidate.procedure_steps {
        for word in step
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        {
            let word = word.trim().to_ascii_lowercase();
            if word.len() >= 7 {
                keys.push(word);
            }
        }
    }
    keys.sort();
    keys.dedup();
    keys.truncate(16);
    keys
}

fn push_sorted_lines(out: &mut String, key: &str, values: &[String]) {
    let mut values = values
        .iter()
        .map(|value| sanitize_line(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    for (idx, value) in values.iter().enumerate() {
        out.push_str(&format!("{key}_{idx}={value}\n"));
    }
}

fn sanitize_ref_segment(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else if c.is_whitespace() {
            out.push('-');
        }
    }
    out.trim_matches('-').to_ascii_lowercase()
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
    fn brain_exposes_searcharchive_as_general_codeact_template() {
        let template = brain_searcharchive_codeact_template();

        assert_eq!(template.command, BRAIN_SEARCHARCHIVE_COMMAND);
        assert_eq!(template.section, "general");
        assert_eq!(template.result_schema, BRAIN_SEARCHARCHIVE_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("bounded snippets"));
        assert!(template.slots.iter().any(|slot| slot.name == "query" && slot.required));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "context_window" && slot.description.contains("token")
        }));
        assert!(brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_SEARCHARCHIVE_COMMAND));
    }

    #[test]
    fn brain_exposes_googleweb_as_general_codeact_template() {
        let template = brain_googleweb_codeact_template();

        assert_eq!(template.command, BRAIN_GOOGLEWEB_COMMAND);
        assert_eq!(template.section, "webexplorer");
        assert_eq!(template.result_schema, BRAIN_GOOGLEWEB_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("native WebExplorer"));
        assert!(template.slots.iter().any(|slot| slot.name == "query" && slot.required));
        assert!(template.slots.iter().any(|slot| slot.name == "keywords" && slot.required));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "open_mode" && slot.allowed_values.contains(&"split_webexplorer".to_string())
        }));
        assert!(brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_GOOGLEWEB_COMMAND));
    }

    #[test]
    fn brain_exposes_maps_as_general_codeact_template() {
        let template = brain_maps_codeact_template();

        assert_eq!(template.command, BRAIN_MAPS_COMMAND);
        assert_eq!(template.section, "webexplorer");
        assert_eq!(template.result_schema, BRAIN_MAPS_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("Google Earth"));
        assert!(template.purpose.contains("Device location must never be read silently"));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "latitude" && slot.description.contains("WGS84")
        }));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "open_mode" && slot.allowed_values.contains(&"split_webexplorer".to_string())
        }));
        assert!(brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_MAPS_COMMAND));
    }

    #[test]
    fn brain_exposes_gmail_as_general_codeact_template() {
        let template = brain_gmail_codeact_template();

        assert_eq!(template.command, BRAIN_GMAIL_COMMAND);
        assert_eq!(template.section, "webexplorer");
        assert_eq!(template.result_schema, BRAIN_GMAIL_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("Gmail"));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "intent"
                && slot.required
                && slot.allowed_values.contains(&"search".to_string())
                && slot.allowed_values.contains(&"draft".to_string())
        }));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "query" && slot.description.contains("from:")
        }));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "body" && slot.description.contains("user-approved")
        }));
        assert!(brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_GMAIL_COMMAND));
    }

    #[test]
    fn brain_exposes_airbnb_as_general_codeact_template() {
        let template = brain_airbnb_codeact_template();

        assert_eq!(template.command, BRAIN_AIRBNB_COMMAND);
        assert_eq!(template.section, "webexplorer");
        assert_eq!(template.result_schema, BRAIN_AIRBNB_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("Airbnb.com"));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "intent"
                && slot.allowed_values.contains(&"open".to_string())
                && slot.allowed_values.contains(&"search".to_string())
        }));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "open_mode" && slot.allowed_values.contains(&"split_webexplorer".to_string())
        }));
        assert!(brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_AIRBNB_COMMAND));
    }

    #[test]
    fn brain_exposes_questionnaire_as_specialized_codeact_template() {
        let template = brain_questionnaire_codeact_template();

        assert_eq!(template.command, BRAIN_QUESTIONNAIRE_COMMAND);
        assert_eq!(template.section, "conversation");
        assert_eq!(template.result_schema, BRAIN_QUESTIONNAIRE_RESULT_SCHEMA);
        assert_eq!(template.proof_hash.len(), 40);
        assert!(template.purpose.contains("above the composer"));
        assert!(template.purpose.contains("long question list"));
        assert!(template.purpose.contains("color:#38bdf8"));
        assert!(template.purpose.contains("never put arbitrary CSS or JS"));
        assert!(template.slots.iter().any(|slot| slot.name == "q1" && slot.required));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "q1_options" && slot.description.contains("colors:#hex,#hex")
        }));
        assert!(template.slots.iter().any(|slot| {
            slot.name == "output" && slot.allowed_values.contains(&"composer_questionnaire".to_string())
        }));
        assert!(!brain_general_codeact_templates()
            .iter()
            .any(|candidate| candidate.command == BRAIN_QUESTIONNAIRE_COMMAND));
        assert!(BRAIN_SCIENCE_VISIBLE_CATALOG.contains(BRAIN_QUESTIONNAIRE_COMMAND));
        assert!(BRAIN_CODING_VISIBLE_CATALOG.contains(BRAIN_QUESTIONNAIRE_COMMAND));
    }

    #[test]
    fn brain_exposes_segment_switches_as_general_codeact_templates() {
        let science = brain_science_codeact_template();
        let coding = brain_coding_codeact_template();

        assert_eq!(science.command, BRAIN_SCIENCE_COMMAND);
        assert_eq!(coding.command, BRAIN_CODING_COMMAND);
        assert_eq!(science.section, "brain_segment");
        assert_eq!(coding.section, "brain_segment");
        assert_eq!(science.result_schema, BRAIN_SEGMENT_RESULT_SCHEMA);
        assert_eq!(coding.result_schema, BRAIN_SEGMENT_RESULT_SCHEMA);
        assert_eq!(science.proof_hash.len(), 40);
        assert_eq!(coding.proof_hash.len(), 40);
        assert!(science.purpose.contains("Science/Engineering/3D Brain"));
        assert!(science.purpose.contains("mathematics"));
        assert!(science.purpose.contains("biology"));
        assert!(science.purpose.contains("cryptography"));
        assert!(science.purpose.contains("/newcompute_"));
        assert!(science.purpose.contains("future Banger 3D"));
        assert!(coding.purpose.contains("Coding Brain"));
        assert!(science.slots.iter().any(|slot| {
            slot.name == "output" && slot.allowed_values.contains(&"inject_brain_catalog".to_string())
        }));
        assert!(coding.slots.iter().any(|slot| {
            slot.name == "output" && slot.allowed_values.contains(&"inject_brain_catalog".to_string())
        }));

        let templates = brain_general_codeact_templates();
        assert!(templates.iter().any(|candidate| candidate.command == BRAIN_SCIENCE_COMMAND));
        assert!(templates.iter().any(|candidate| candidate.command == BRAIN_CODING_COMMAND));
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
        assert!(memory_text.contains("substitution_candidate_hash="));
        assert!(memory_text.contains("verifier_hash="));
        assert_eq!(memory.substitution_candidate_hash.as_hex().len(), 40);
        assert_eq!(memory.verifier_hash.as_hex().len(), 40);

        let projection = memory.proof_projection();
        assert_eq!(projection.action, "accepted_substitution");
        assert!(projection.accepted);
        assert_eq!(projection.from, from.as_hex());
        assert_eq!(projection.to, Some(to.as_hex()));
        assert_eq!(projection.memory_hash, Some(memory_hash.as_hex()));
        assert_eq!(projection.node_delta, -2);
        assert_eq!(projection.projection_hash.len(), 40);
        assert_eq!(
            projection.substitution_candidate_hash,
            memory.substitution_candidate_hash.as_hex()
        );
        assert_eq!(projection.verifier_hash, memory.verifier_hash.as_hex());
        assert_eq!(projection, memory.proof_projection());
    }

    #[test]
    fn semantic_note_smoke_proof_accepts_recalled_anchored_note() {
        let recalled = vec!["note-1".to_string(), "note-2".to_string()];
        let proof = semantic_note_smoke_proof(
            "note-1",
            "evidence-1",
            "recall-1",
            &recalled,
        );
        assert!(proof.accepted);
        assert_eq!(proof.note_hash, "note-1");
        assert_eq!(proof.evidence_hash, "evidence-1");
        assert_eq!(proof.recall_hash, "recall-1");
        assert_eq!(proof.reasons.len(), 0);
        assert_eq!(proof.verification_hash.len(), 40);
        assert_eq!(proof.proof_hash.len(), 40);
        assert_eq!(proof, semantic_note_smoke_proof("note-1", "evidence-1", "recall-1", &recalled));
    }

    #[test]
    fn reusable_semantic_memory_proof_manifest_is_stable_for_duplicate_recall_entries() {
        let first = semantic_note_reusable_proof_manifest(
            "ops",
            "semantic",
            "text-1",
            "evidence-1",
            &[
                "other".to_string(),
                Hash::for_blob(
                    b"forge-brain-semantic-note-identity-v1\nscope=ops\nmemory_layer=semantic\ntext_hash=text-1\nevidence_hash=evidence-1\n",
                )
                .as_hex(),
                Hash::for_blob(
                    b"forge-brain-semantic-note-identity-v1\nscope=ops\nmemory_layer=semantic\ntext_hash=text-1\nevidence_hash=evidence-1\n",
                )
                .as_hex(),
            ],
        );
        let second = semantic_note_reusable_proof_manifest(
            "ops",
            "semantic",
            "text-1",
            "evidence-1",
            &[
                Hash::for_blob(
                    b"forge-brain-semantic-note-identity-v1\nscope=ops\nmemory_layer=semantic\ntext_hash=text-1\nevidence_hash=evidence-1\n",
                )
                .as_hex(),
                "other".to_string(),
            ],
        );

        assert!(first.accepted);
        assert_eq!(first, second);
        assert_eq!(first.proof_hash.len(), 40);
        assert_eq!(first.verification_hash.len(), 40);
    }

    #[test]
    fn codeact_skill_promotion_accepts_verified_session_evidence() {
        let (node, _path) = fresh_node("skill-promotion");
        let sessions = vec![
            BrainExperienceDigest {
                session_id: "recent-cron-scraper".to_string(),
                title: "Cron scraper repair".to_string(),
                created_at: "2026-06-08".to_string(),
                archived: false,
                outcome: "verified passed with retry policy".to_string(),
                evidence_hash: "evidence-cron-1".to_string(),
                codeact_refs: vec!["/newcompute_".to_string(), "/brain_tune_".to_string()],
            },
            BrainExperienceDigest {
                session_id: "archived-scraper-failure".to_string(),
                title: "Archived scraper failure".to_string(),
                created_at: "2026-06-03".to_string(),
                archived: true,
                outcome: "failed before backoff was added".to_string(),
                evidence_hash: "evidence-cron-0".to_string(),
                codeact_refs: vec!["/newcompute_".to_string()],
            },
        ];
        let candidate = BrainSkillCandidate {
            skill_id: "cron_scraper_setup".to_string(),
            activation: "When the user asks for a cron scraping job".to_string(),
            codeact_command: "/newcompute_".to_string(),
            procedure_steps: vec![
                "Start from a tiny idempotent fetch step with timeout and user-agent".to_string(),
                "Add scheduler only after the fetch proof and backoff sample pass".to_string(),
                "Persist a rollback command next to the generated cron entry".to_string(),
            ],
            avoid_patterns: vec!["Do not add cron before validating the scraper once".to_string()],
            required_evidence_hashes: vec!["evidence-cron-1".to_string()],
            source_session_ids: vec![
                "recent-cron-scraper".to_string(),
                "archived-scraper-failure".to_string(),
            ],
            previous_skill_hash: None,
            rollback_ref: "refs/brain/skill/cron_scraper_setup@previous".to_string(),
            user_note: "Prefer backoff before cron wiring.".to_string(),
        };

        let proof = publish_brain_skill_promotion(&node, &candidate, &sessions).unwrap();

        assert!(proof.accepted);
        assert!(proof.reasons.is_empty());
        assert_eq!(proof.proof_hash.len(), 40);
        assert_eq!(proof.candidate_hash.len(), 40);
        assert_eq!(proof.session_index_hash.len(), 40);
        let manifest_hash = proof.manifest_hash.as_deref().expect("manifest hash");
        assert_eq!(manifest_hash.len(), 40);
        assert!(proof.retrieval_keys.contains(&"/newcompute_".to_string()));
        assert_eq!(
            node.store()
                .lookup_ref("refs/brain/skill/cron_scraper_setup")
                .map(|hash| hash.as_hex()),
            Some(manifest_hash.to_string())
        );
    }

    #[test]
    fn codeact_skill_promotion_rejects_unproven_or_unsafe_memory() {
        let sessions = vec![BrainExperienceDigest {
            session_id: "recent-unsafe".to_string(),
            title: "Unsafe attempt".to_string(),
            created_at: "2026-06-08".to_string(),
            archived: false,
            outcome: "failed".to_string(),
            evidence_hash: "evidence-unsafe".to_string(),
            codeact_refs: vec![],
        }];
        let candidate = BrainSkillCandidate {
            skill_id: "bad".to_string(),
            activation: "cron".to_string(),
            codeact_command: "newcompute".to_string(),
            procedure_steps: vec!["Store token=abc in memory".to_string()],
            avoid_patterns: Vec::new(),
            required_evidence_hashes: vec!["missing-evidence".to_string()],
            source_session_ids: vec!["missing-session".to_string()],
            previous_skill_hash: None,
            rollback_ref: "".to_string(),
            user_note: "disable godel for speed".to_string(),
        };

        let proof = verify_brain_skill_promotion(&candidate, &sessions);

        assert!(!proof.accepted);
        assert!(proof.promoted_skill_ref.is_none());
        assert!(proof.reasons.iter().any(|reason| reason.contains("CodeAct command")));
        assert!(proof.reasons.iter().any(|reason| reason.contains("rollback")));
        assert!(proof.reasons.iter().any(|reason| reason.contains("unsafe")));
        assert_eq!(proof.proof_hash.len(), 40);
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
    fn invalid_substitution_fails_closed_with_stable_verifier_hash() {
        let (node, _path) = fresh_node("reject-proof");
        let from_program = add_self_program();
        let to_program = id_program();
        let from = node.store().store(from_program.bytes()).expect("from store");
        let to = node.store().store(to_program.bytes()).expect("to store");
        let rewrite = RewriteV2::ProgramSubstitution { from, to };
        let samples = strict_equiv_samples(1);

        let first = verify_program_substitution_strict(&node, &rewrite, &from_program, &to_program, samples);
        let second =
            verify_program_substitution_strict(&node, &rewrite, &from_program, &to_program, samples);
        let (first_reasons, second_reasons) = match (first, second) {
            (
                VerificationOutcomeV2::Reject { reasons: first_reasons },
                VerificationOutcomeV2::Reject {
                    reasons: second_reasons,
                },
            ) => (first_reasons, second_reasons),
            other => panic!("expected rejection, got {other:?}"),
        };
        let frame_before = frame_hash(&capture(&node));
        let frame_after = frame_hash(&capture(&node));
        let candidate_hash = substitution_candidate_hash(from, &[to]);
        let first_verifier_hash = substitution_verifier_hash(
            BrainAction::RejectedCandidate,
            from,
            Some(to),
            candidate_hash,
            samples,
            frame_before,
            frame_after,
            &first_reasons,
        );
        let second_verifier_hash = substitution_verifier_hash(
            BrainAction::RejectedCandidate,
            from,
            Some(to),
            candidate_hash,
            samples,
            frame_before,
            frame_after,
            &second_reasons,
        );

        assert_eq!(candidate_hash, substitution_candidate_hash(from, &[to]));
        assert_eq!(first_reasons, second_reasons);
        assert_eq!(first_verifier_hash, second_verifier_hash);
        assert!(!first_reasons.is_empty());
        assert_eq!(
            publish_program_substitution(&node, from, &from_program, &to_program, 1).unwrap(),
            None
        );
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
