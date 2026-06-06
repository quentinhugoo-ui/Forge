use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NativeSection {
    Shell,
    Alpha,
    Forge,
    WebExplorer,
    RealEstate,
    Trading,
    Banger,
}

impl Default for NativeSection {
    fn default() -> Self {
        Self::Forge
    }
}

impl NativeSection {
    const ORDER: [Self; 7] = [
        Self::Shell,
        Self::Alpha,
        Self::Forge,
        Self::WebExplorer,
        Self::Trading,
        Self::Banger,
        Self::RealEstate,
    ];

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "shell" => Some(Self::Shell),
            "alpha" => Some(Self::Alpha),
            "forge" => Some(Self::Forge),
            "webexplorer" => Some(Self::WebExplorer),
            "real-estate" | "real-estate-main" => Some(Self::RealEstate),
            "trading" => Some(Self::Trading),
            "banger" => Some(Self::Banger),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Alpha => "alpha",
            Self::Forge => "forge",
            Self::WebExplorer => "webexplorer",
            Self::RealEstate => "real-estate",
            Self::Trading => "trading",
            Self::Banger => "banger",
        }
    }

    pub fn next(self) -> Self {
        let index = Self::ORDER
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0);
        Self::ORDER[(index + 1) % Self::ORDER.len()]
    }

    fn title(self) -> &'static str {
        match self {
            Self::Shell => "New session",
            Self::Alpha => "Alpha canvas",
            Self::Forge => "New session",
            Self::WebExplorer => "RAM DOM Atlas",
            Self::RealEstate => "Nouvelle session immo",
            Self::Trading => "NATGASUSD",
            Self::Banger => "New object",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Shell => "Heavy compute in any domain - data, code, medical imaging, genomics, anything. The LLM stays out of files and math, saving massive tokens.",
            Self::Alpha => "Canvas placeholder; current Tauri behavior remains the reference.",
            Self::Forge => "Heavy compute in any domain - data, code, medical imaging, genomics, anything. The LLM stays out of files and math, saving massive tokens.",
            Self::WebExplorer => "WRY/WebView2 remains a contained web peripheral, not the app shell.",
            Self::RealEstate => "Vertical surface placeholder; runtime parity still pending.",
            Self::Trading => "Trading dashboard placeholder; Bloomberg live web stays peripheral.",
            Self::Banger => "wgpu viewport placeholder; DOM canvas is not the target.",
        }
    }

    fn canvas_title(self) -> &'static str {
        match self {
            Self::Shell | Self::Forge => "Drop any file",
            Self::Alpha => "Alpha canvas",
            Self::WebExplorer => "Open a page",
            Self::RealEstate => "Initialisation de Forge",
            Self::Trading => "Market workspace",
            Self::Banger => "Native viewport",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NativeJobStatus {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeJob {
    pub id: String,
    pub label: String,
    pub status: NativeJobStatus,
    pub proof_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeModal {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub provider: String,
    pub model: String,
    pub online: bool,
}

impl Default for ProviderStatus {
    fn default() -> Self {
        Self {
            provider: "local-placeholder".to_string(),
            model: "unbound".to_string(),
            online: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeSessionSummary {
    pub id: String,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NativeMessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeChatMessage {
    pub id: String,
    pub role: NativeMessageRole,
    pub body: String,
    pub proof_hash: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NativeAgentCardKind {
    Plan,
    Questionnaire,
    Proof,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeAgentCard {
    pub id: String,
    pub kind: NativeAgentCardKind,
    pub title: String,
    pub body: String,
    pub proof_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisiblePanels {
    pub proof: bool,
    pub left: bool,
    pub modal: bool,
}

impl Default for VisiblePanels {
    fn default() -> Self {
        Self {
            proof: true,
            left: true,
            modal: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebExplorerState {
    pub desired_visible: bool,
    pub focused: bool,
    pub url: Option<String>,
}

impl Default for WebExplorerState {
    fn default() -> Self {
        Self {
            desired_visible: false,
            focused: false,
            url: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BangerViewportState {
    pub desired_visible: bool,
    pub fixture_scene: String,
    pub frame_hash: Option<String>,
}

impl Default for BangerViewportState {
    fn default() -> Self {
        Self {
            desired_visible: false,
            fixture_scene: "stage0-placeholder".to_string(),
            frame_hash: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeUiState {
    active_section: NativeSection,
    chat_draft: String,
    selected_session: String,
    sessions: Vec<NativeSessionSummary>,
    transcript: Vec<NativeChatMessage>,
    agent_cards: Vec<NativeAgentCard>,
    hardware_status: String,
    provider: ProviderStatus,
    jobs: Vec<NativeJob>,
    proof_badges: Vec<String>,
    brain_status: String,
    monster_status: String,
    trading_status: String,
    real_estate_status: String,
    panels: VisiblePanels,
    webexplorer: WebExplorerState,
    banger: BangerViewportState,
    modal: Option<NativeModal>,
    event_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeStateCheckpoint {
    pub schema: String,
    pub events: Vec<NativeUiEvent>,
    pub projection: NativeUiProjection,
    pub event_log_hash: String,
    pub state_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeStateKernel {
    state: NativeUiState,
    events: Vec<NativeUiEvent>,
}

impl Default for NativeUiState {
    fn default() -> Self {
        Self {
            active_section: NativeSection::Forge,
            chat_draft: String::new(),
            selected_session: "native-front-migration".to_string(),
            sessions: vec![NativeSessionSummary {
                id: "native-front-migration".to_string(),
                title: "New session".to_string(),
            }],
            transcript: Vec::new(),
            agent_cards: vec![
                NativeAgentCard {
                    id: "stage8-plan".to_string(),
                    kind: NativeAgentCardKind::Plan,
                    title: "Migration Front Stage 8".to_string(),
                    body: "Native transcript, session list, model status and proof cards are rendered by Slint/Rust only.".to_string(),
                    proof_hash: None,
                },
                NativeAgentCard {
                    id: "stage8-questionnaire".to_string(),
                    kind: NativeAgentCardKind::Questionnaire,
                    title: "Questionnaire boundary".to_string(),
                    body: "Question panels are native cards; LLM CodeAct remains deferred until the frontend migration is complete.".to_string(),
                    proof_hash: None,
                },
            ],
            hardware_status: "hardware=unbound".to_string(),
            provider: ProviderStatus::default(),
            jobs: Vec::new(),
            proof_badges: vec!["design-parity=false".to_string()],
            brain_status: "brain=unbound".to_string(),
            monster_status: "monster=unbound".to_string(),
            trading_status: "trading=unbound".to_string(),
            real_estate_status: "real-estate=unbound".to_string(),
            panels: VisiblePanels::default(),
            webexplorer: WebExplorerState::default(),
            banger: BangerViewportState::default(),
            modal: None,
            event_count: 0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NativeUiEvent {
    Navigate { section: String },
    NavigateNext,
    ChatDraftChanged { draft: String },
    SendChat,
    SessionsReplaced { sessions: Vec<NativeSessionSummary> },
    SelectSession { session_id: String },
    AgentMessageAppended { message: NativeChatMessage },
    AgentCardUpserted { card: NativeAgentCard },
    HardwareUpdated { status: String },
    ProviderUpdated { provider: String, model: String, online: bool },
    JobUpserted { job: NativeJob },
    ProofBadgeRaised { badge: String },
    BrainUpdated { status: String },
    MonsterUpdated { status: String },
    TogglePanel { panel: String },
    WebExplorerUpdated { visible: bool, focused: bool, url: Option<String> },
    BangerViewportUpdated { visible: bool, frame_hash: Option<String> },
    BangerServiceUpdated { status: String },
    TradingUpdated { status: String },
    RealEstateUpdated { status: String },
    OpenModal { title: String, body: String },
    CloseModal,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NativeUiProjection {
    pub active_section: String,
    pub section_title: String,
    pub canvas_title: String,
    pub canvas_hint: String,
    pub selected_session: String,
    pub session_lines: String,
    pub transcript_lines: String,
    pub plan_cards: String,
    pub questionnaire_cards: String,
    pub proof_cards: String,
    pub agent_surface_status: String,
    pub hardware_status: String,
    pub provider_status: String,
    pub job_status: String,
    pub proof_badge_status: String,
    pub brain_status: String,
    pub monster_status: String,
    pub panel_status: String,
    pub webexplorer_status: String,
    pub banger_status: String,
    pub trading_status: String,
    pub real_estate_status: String,
    pub modal_visible: bool,
    pub modal_title: String,
    pub modal_body: String,
    pub event_count: u64,
    pub projection_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionHashEnvelope<'a> {
    active_section: &'a str,
    section_title: &'a str,
    canvas_title: &'a str,
    canvas_hint: &'a str,
    selected_session: &'a str,
    session_lines: &'a str,
    transcript_lines: &'a str,
    plan_cards: &'a str,
    questionnaire_cards: &'a str,
    proof_cards: &'a str,
    agent_surface_status: &'a str,
    hardware_status: &'a str,
    provider_status: &'a str,
    job_status: &'a str,
    proof_badge_status: &'a str,
    brain_status: &'a str,
    monster_status: &'a str,
    panel_status: &'a str,
    webexplorer_status: &'a str,
    banger_status: &'a str,
    trading_status: &'a str,
    real_estate_status: &'a str,
    modal_visible: bool,
    modal_title: &'a str,
    modal_body: &'a str,
    event_count: u64,
}

impl NativeUiState {
    pub fn chat_draft(&self) -> &str {
        &self.chat_draft
    }

    pub fn apply(&mut self, event: NativeUiEvent) {
        match event {
            NativeUiEvent::Navigate { section } => {
                if let Some(section) = NativeSection::from_id(&section) {
                    self.set_section(section);
                    self.modal = None;
                } else {
                    self.modal = Some(NativeModal {
                        title: "Navigation blocked".to_string(),
                        body: format!("Unknown native section '{section}'"),
                    });
                    self.panels.modal = true;
                }
            }
            NativeUiEvent::NavigateNext => {
                self.set_section(self.active_section.next());
                self.modal = None;
            }
            NativeUiEvent::ChatDraftChanged { draft } => {
                self.chat_draft = draft;
            }
            NativeUiEvent::SendChat => {
                let draft = self.chat_draft.trim().to_string();
                if draft.is_empty() {
                    self.modal = Some(NativeModal {
                        title: "Empty command".to_string(),
                        body: "The native shell kept the empty chat command local.".to_string(),
                    });
                    self.panels.modal = true;
                } else {
                    let user_message_id = format!("msg-user-{}", self.event_count + 1);
                    self.transcript.push(NativeChatMessage {
                        id: user_message_id,
                        role: NativeMessageRole::User,
                        body: draft.clone(),
                        proof_hash: None,
                    });
                    let native_reply_proof = stable_hash(&(
                        "stage8-native-agent-surface",
                        draft.as_str(),
                        self.event_count,
                    ));
                    self.transcript.push(NativeChatMessage {
                        id: format!("msg-native-{}", self.event_count + 1),
                        role: NativeMessageRole::System,
                        body: "Captured locally by the Slint/Rust agent surface; no browser IPC or CodeAct dispatch.".to_string(),
                        proof_hash: Some(native_reply_proof.clone()),
                    });
                    self.upsert_agent_card(NativeAgentCard {
                        id: format!("proof-chat-{}", self.event_count + 1),
                        kind: NativeAgentCardKind::Proof,
                        title: "Native chat capture".to_string(),
                        body: format!("draft='{}' stored in the replayable Rust state kernel", draft),
                        proof_hash: Some(native_reply_proof),
                    });
                    self.jobs.push(NativeJob {
                        id: format!("native-chat-{}", self.event_count + 1),
                        label: draft.clone(),
                        status: NativeJobStatus::Queued,
                        proof_hash: None,
                    });
                    self.modal = Some(NativeModal {
                        title: "Command captured".to_string(),
                        body: format!("Native state recorded '{draft}' without browser IPC."),
                    });
                    self.panels.modal = true;
                    self.chat_draft.clear();
                }
            }
            NativeUiEvent::SessionsReplaced { sessions } => {
                self.sessions = if sessions.is_empty() {
                    vec![NativeSessionSummary {
                        id: "native-front-migration".to_string(),
                        title: "New session".to_string(),
                    }]
                } else {
                    sessions
                };
                if !self
                    .sessions
                    .iter()
                    .any(|session| session.id == self.selected_session)
                {
                    self.selected_session = self.sessions[0].id.clone();
                }
            }
            NativeUiEvent::SelectSession { session_id } => {
                if self.sessions.iter().any(|session| session.id == session_id) {
                    self.selected_session = session_id;
                } else {
                    self.modal = Some(NativeModal {
                        title: "Session blocked".to_string(),
                        body: format!("Unknown native session '{session_id}'"),
                    });
                    self.panels.modal = true;
                }
            }
            NativeUiEvent::AgentMessageAppended { message } => {
                self.transcript.push(message);
            }
            NativeUiEvent::AgentCardUpserted { card } => {
                self.upsert_agent_card(card);
            }
            NativeUiEvent::HardwareUpdated { status } => {
                self.hardware_status = status;
            }
            NativeUiEvent::ProviderUpdated {
                provider,
                model,
                online,
            } => {
                self.provider = ProviderStatus {
                    provider,
                    model,
                    online,
                };
            }
            NativeUiEvent::JobUpserted { job } => {
                if let Some(existing) = self.jobs.iter_mut().find(|item| item.id == job.id) {
                    *existing = job;
                } else {
                    self.jobs.push(job);
                }
            }
            NativeUiEvent::ProofBadgeRaised { badge } => {
                if !self.proof_badges.iter().any(|item| item == &badge) {
                    self.proof_badges.push(badge);
                }
            }
            NativeUiEvent::BrainUpdated { status } => {
                self.brain_status = status;
            }
            NativeUiEvent::MonsterUpdated { status } => {
                self.monster_status = status;
            }
            NativeUiEvent::TogglePanel { panel } => match panel.as_str() {
                "proof" => self.panels.proof = !self.panels.proof,
                "left" => self.panels.left = !self.panels.left,
                "modal" => self.panels.modal = !self.panels.modal,
                _ => {
                    self.modal = Some(NativeModal {
                        title: "Panel blocked".to_string(),
                        body: format!("Unknown native panel '{panel}'"),
                    });
                    self.panels.modal = true;
                }
            },
            NativeUiEvent::WebExplorerUpdated {
                visible,
                focused,
                url,
            } => {
                self.webexplorer = WebExplorerState {
                    desired_visible: visible,
                    focused,
                    url,
                };
            }
            NativeUiEvent::BangerViewportUpdated {
                visible,
                frame_hash,
            } => {
                self.banger.desired_visible = visible;
                self.banger.frame_hash = frame_hash;
            }
            NativeUiEvent::BangerServiceUpdated { status } => {
                self.banger.fixture_scene = status;
            }
            NativeUiEvent::TradingUpdated { status } => {
                self.trading_status = status;
            }
            NativeUiEvent::RealEstateUpdated { status } => {
                self.real_estate_status = status;
            }
            NativeUiEvent::OpenModal { title, body } => {
                self.modal = Some(NativeModal { title, body });
                self.panels.modal = true;
            }
            NativeUiEvent::CloseModal => {
                self.modal = None;
                self.panels.modal = false;
            }
        }
        self.event_count += 1;
    }

    pub fn projection(&self) -> NativeUiProjection {
        let modal_title = self
            .modal
            .as_ref()
            .map(|modal| modal.title.clone())
            .unwrap_or_default();
        let modal_body = self
            .modal
            .as_ref()
            .map(|modal| modal.body.clone())
            .unwrap_or_default();
        let mut projection = NativeUiProjection {
            active_section: self.active_section.id().to_string(),
            section_title: self.active_section.title().to_string(),
            canvas_title: self.active_section.canvas_title().to_string(),
            canvas_hint: self.active_section.hint().to_string(),
            selected_session: self.selected_session.clone(),
            session_lines: session_lines(&self.sessions, &self.selected_session),
            transcript_lines: transcript_lines(&self.transcript),
            plan_cards: card_lines(&self.agent_cards, NativeAgentCardKind::Plan),
            questionnaire_cards: card_lines(&self.agent_cards, NativeAgentCardKind::Questionnaire),
            proof_cards: card_lines(&self.agent_cards, NativeAgentCardKind::Proof),
            agent_surface_status: agent_surface_status(
                &self.sessions,
                &self.transcript,
                &self.agent_cards,
            ),
            hardware_status: self.hardware_status.clone(),
            provider_status: provider_status(&self.provider),
            job_status: job_status(&self.jobs),
            proof_badge_status: self.proof_badges.join(", "),
            brain_status: self.brain_status.clone(),
            monster_status: self.monster_status.clone(),
            panel_status: panel_status(&self.panels),
            webexplorer_status: webexplorer_status(&self.webexplorer),
            banger_status: banger_status(&self.banger),
            trading_status: self.trading_status.clone(),
            real_estate_status: self.real_estate_status.clone(),
            modal_visible: self.modal.is_some() && self.panels.modal,
            modal_title,
            modal_body,
            event_count: self.event_count,
            projection_hash: String::new(),
        };
        projection.projection_hash = projection_hash(&projection);
        projection
    }

    fn set_section(&mut self, section: NativeSection) {
        self.active_section = section;
        self.webexplorer.desired_visible = section == NativeSection::WebExplorer;
        self.banger.desired_visible = section == NativeSection::Banger;
    }

    fn upsert_agent_card(&mut self, card: NativeAgentCard) {
        if let Some(existing) = self.agent_cards.iter_mut().find(|item| item.id == card.id) {
            *existing = card;
        } else {
            self.agent_cards.push(card);
        }
    }
}

impl Default for NativeStateKernel {
    fn default() -> Self {
        Self {
            state: NativeUiState::default(),
            events: Vec::new(),
        }
    }
}

impl NativeStateKernel {
    pub fn dispatch(&mut self, event: NativeUiEvent) {
        self.state.apply(event.clone());
        self.events.push(event);
    }

    pub fn dispatch_many<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = NativeUiEvent>,
    {
        for event in events {
            self.dispatch(event);
        }
    }

    pub fn projection(&self) -> NativeUiProjection {
        self.state.projection()
    }

    pub fn chat_draft(&self) -> &str {
        self.state.chat_draft()
    }

    pub fn events(&self) -> &[NativeUiEvent] {
        &self.events
    }

    pub fn checkpoint(&self) -> NativeStateCheckpoint {
        NativeStateCheckpoint {
            schema: "ingen.native_front.state_checkpoint.v1".to_string(),
            events: self.events.clone(),
            projection: self.projection(),
            event_log_hash: event_log_hash(&self.events),
            state_hash: state_hash(&self.state),
        }
    }

    pub fn restore(checkpoint: &NativeStateCheckpoint) -> Self {
        let mut kernel = Self::default();
        kernel.dispatch_many(checkpoint.events.clone());
        kernel
    }
}

pub fn replay_projection(events: &[NativeUiEvent]) -> NativeUiProjection {
    replay_checkpoint(events).projection
}

pub fn replay_checkpoint(events: &[NativeUiEvent]) -> NativeStateCheckpoint {
    let mut kernel = NativeStateKernel::default();
    kernel.dispatch_many(events.to_vec());
    kernel.checkpoint()
}

pub fn checkpoint_json(checkpoint: &NativeStateCheckpoint) -> String {
    serde_json::to_string_pretty(checkpoint).expect("serialize native state checkpoint")
}

fn provider_status(provider: &ProviderStatus) -> String {
    let state = if provider.online { "online" } else { "offline" };
    format!("{} / {} / {}", provider.provider, provider.model, state)
}

fn session_lines(sessions: &[NativeSessionSummary], selected_session: &str) -> String {
    sessions
        .iter()
        .take(9)
        .map(|session| {
            let marker = if session.id == selected_session { "*" } else { "o" };
            format!("{marker} {}", session.title)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn transcript_lines(messages: &[NativeChatMessage]) -> String {
    if messages.is_empty() {
        return "No native transcript yet.".to_string();
    }
    let total = messages.len();
    let start = total.saturating_sub(8);
    let mut lines = messages[start..]
        .iter()
        .map(|message| {
            let role = match message.role {
                NativeMessageRole::User => "user",
                NativeMessageRole::Assistant => "assistant",
                NativeMessageRole::System => "system",
            };
            let proof = message
                .proof_hash
                .as_deref()
                .map(|hash| format!(" proof={}", short_hash(hash)))
                .unwrap_or_default();
            format!("{role}{proof}: {}", message.body)
        })
        .collect::<Vec<_>>();
    if start > 0 {
        lines.insert(0, format!("... {} older native messages", start));
    }
    lines.join("\n\n")
}

fn card_lines(cards: &[NativeAgentCard], kind: NativeAgentCardKind) -> String {
    let lines = cards
        .iter()
        .filter(|card| card.kind == kind)
        .take(6)
        .map(|card| {
            let proof = card
                .proof_hash
                .as_deref()
                .map(|hash| format!(" proof={}", short_hash(hash)))
                .unwrap_or_default();
            format!("{}{}\n{}", card.title, proof, card.body)
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "none".to_string()
    } else {
        lines.join("\n\n")
    }
}

fn agent_surface_status(
    sessions: &[NativeSessionSummary],
    messages: &[NativeChatMessage],
    cards: &[NativeAgentCard],
) -> String {
    format!(
        "sessions={} transcript={} cards={} showing={} browser_ipc=false codeact=false",
        sessions.len(),
        messages.len(),
        cards.len(),
        messages.len().min(8)
    )
}

fn job_status(jobs: &[NativeJob]) -> String {
    if jobs.is_empty() {
        return "jobs=0".to_string();
    }
    let running = jobs
        .iter()
        .filter(|job| job.status == NativeJobStatus::Running)
        .count();
    let queued = jobs
        .iter()
        .filter(|job| job.status == NativeJobStatus::Queued)
        .count();
    let done = jobs
        .iter()
        .filter(|job| job.status == NativeJobStatus::Done)
        .count();
    let failed = jobs
        .iter()
        .filter(|job| job.status == NativeJobStatus::Failed)
        .count();
    format!(
        "jobs={} queued={} running={} done={} failed={}",
        jobs.len(),
        queued,
        running,
        done,
        failed
    )
}

fn panel_status(panels: &VisiblePanels) -> String {
    format!(
        "left={} proof={} modal={}",
        panels.left, panels.proof, panels.modal
    )
}

fn webexplorer_status(state: &WebExplorerState) -> String {
    format!(
        "webexplorer visible={} focused={} url={}",
        state.desired_visible,
        state.focused,
        state.url.as_deref().unwrap_or("none")
    )
}

fn banger_status(state: &BangerViewportState) -> String {
    format!(
        "banger visible={} scene={} frame={}",
        state.desired_visible,
        state.fixture_scene,
        state.frame_hash.as_deref().unwrap_or("none")
    )
}

fn projection_hash(projection: &NativeUiProjection) -> String {
    let envelope = ProjectionHashEnvelope {
        active_section: &projection.active_section,
        section_title: &projection.section_title,
        canvas_title: &projection.canvas_title,
        canvas_hint: &projection.canvas_hint,
        selected_session: &projection.selected_session,
        session_lines: &projection.session_lines,
        transcript_lines: &projection.transcript_lines,
        plan_cards: &projection.plan_cards,
        questionnaire_cards: &projection.questionnaire_cards,
        proof_cards: &projection.proof_cards,
        agent_surface_status: &projection.agent_surface_status,
        hardware_status: &projection.hardware_status,
        provider_status: &projection.provider_status,
        job_status: &projection.job_status,
        proof_badge_status: &projection.proof_badge_status,
        brain_status: &projection.brain_status,
        monster_status: &projection.monster_status,
        panel_status: &projection.panel_status,
        webexplorer_status: &projection.webexplorer_status,
        banger_status: &projection.banger_status,
        trading_status: &projection.trading_status,
        real_estate_status: &projection.real_estate_status,
        modal_visible: projection.modal_visible,
        modal_title: &projection.modal_title,
        modal_body: &projection.modal_body,
        event_count: projection.event_count,
    };
    let bytes = serde_json::to_vec(&envelope).expect("serialize native ui projection");
    format!("{:x}", Sha256::digest(bytes))
}

fn event_log_hash(events: &[NativeUiEvent]) -> String {
    let bytes = serde_json::to_vec(events).expect("serialize native event log");
    format!("{:x}", Sha256::digest(bytes))
}

fn state_hash(state: &NativeUiState) -> String {
    let bytes = serde_json::to_vec(state).expect("serialize native ui state");
    format!("{:x}", Sha256::digest(bytes))
}

fn stable_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize stable hash input");
    format!("{:x}", Sha256::digest(bytes))
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_is_deterministic() {
        let events = vec![
            NativeUiEvent::Navigate {
                section: "banger".to_string(),
            },
            NativeUiEvent::ChatDraftChanged {
                draft: "/newcompute_ sdf".to_string(),
            },
            NativeUiEvent::SendChat,
        ];

        let first = replay_projection(&events);
        let second = replay_projection(&events);

        assert_eq!(first, second);
        assert_eq!(first.active_section, "banger");
        assert!(first.banger_status.contains("visible=true"));
        assert!(first.job_status.contains("queued=1"));
        assert!(first.modal_visible);
        assert!(first.modal_body.contains("without browser IPC"));
    }

    #[test]
    fn invalid_section_is_blocked_in_state() {
        let projection = replay_projection(&[NativeUiEvent::Navigate {
            section: "tauri-hidden-shell".to_string(),
        }]);

        assert_eq!(projection.active_section, "forge");
        assert!(projection.modal_visible);
        assert_eq!(projection.modal_title, "Navigation blocked");
    }

    #[test]
    fn provider_jobs_and_proofs_project_without_browser_ipc() {
        let projection = replay_projection(&[
            NativeUiEvent::ProviderUpdated {
                provider: "openai".to_string(),
                model: "gpt-5".to_string(),
                online: true,
            },
            NativeUiEvent::JobUpserted {
                job: NativeJob {
                    id: "job-1".to_string(),
                    label: "fixture render".to_string(),
                    status: NativeJobStatus::Done,
                    proof_hash: Some("abc123".to_string()),
                },
            },
            NativeUiEvent::ProofBadgeRaised {
                badge: "frame-hash=abc123".to_string(),
            },
        ]);

        assert!(projection.provider_status.contains("openai"));
        assert!(projection.job_status.contains("done=1"));
        assert!(projection.proof_badge_status.contains("frame-hash=abc123"));
    }

    #[test]
    fn native_agent_surface_records_chat_without_codeact() {
        let projection = replay_projection(&[
            NativeUiEvent::ChatDraftChanged {
                draft: "Mine this 50 GB metagenome".to_string(),
            },
            NativeUiEvent::SendChat,
        ]);

        assert!(projection.transcript_lines.contains("user: Mine this 50 GB metagenome"));
        assert!(projection.transcript_lines.contains("browser IPC"));
        assert!(projection.proof_cards.contains("Native chat capture"));
        assert!(projection.agent_surface_status.contains("browser_ipc=false"));
        assert!(projection.agent_surface_status.contains("codeact=false"));
        assert!(projection.job_status.contains("queued=1"));
    }

    #[test]
    fn native_agent_surface_virtualizes_long_transcripts() {
        let events = (0..14)
            .map(|index| NativeUiEvent::AgentMessageAppended {
                message: NativeChatMessage {
                    id: format!("m-{index}"),
                    role: NativeMessageRole::Assistant,
                    body: format!("message {index}"),
                    proof_hash: None,
                },
            })
            .collect::<Vec<_>>();
        let projection = replay_projection(&events);

        assert!(projection.transcript_lines.contains("6 older native messages"));
        assert!(!projection.transcript_lines.contains("message 0"));
        assert!(projection.transcript_lines.contains("message 13"));
        assert!(projection.agent_surface_status.contains("showing=8"));
    }

    #[test]
    fn native_sessions_and_cards_project_to_slint_strings() {
        let projection = replay_projection(&[
            NativeUiEvent::SessionsReplaced {
                sessions: vec![
                    NativeSessionSummary {
                        id: "s1".to_string(),
                        title: "Migration Front".to_string(),
                    },
                    NativeSessionSummary {
                        id: "s2".to_string(),
                        title: "Banger viewport".to_string(),
                    },
                ],
            },
            NativeUiEvent::SelectSession {
                session_id: "s2".to_string(),
            },
            NativeUiEvent::AgentCardUpserted {
                card: NativeAgentCard {
                    id: "plan-1".to_string(),
                    kind: NativeAgentCardKind::Plan,
                    title: "Native plan".to_string(),
                    body: "Rendered in Slint.".to_string(),
                    proof_hash: Some("abcdef1234567890".to_string()),
                },
            },
        ]);

        assert!(projection.session_lines.contains("* Banger viewport"));
        assert!(projection.plan_cards.contains("Native plan proof=abcdef123456"));
        assert!(projection.agent_surface_status.contains("sessions=2"));
    }

    #[test]
    fn navigate_next_cycles_sections() {
        let projection = replay_projection(&[
            NativeUiEvent::NavigateNext,
            NativeUiEvent::NavigateNext,
            NativeUiEvent::NavigateNext,
        ]);

        assert_eq!(projection.active_section, "banger");
        assert!(projection.banger_status.contains("visible=true"));
    }

    #[test]
    fn checkpoint_restores_from_event_log() {
        let events = vec![
            NativeUiEvent::Navigate {
                section: "webexplorer".to_string(),
            },
            NativeUiEvent::ProviderUpdated {
                provider: "openrouter".to_string(),
                model: "gpt-5.3".to_string(),
                online: true,
            },
            NativeUiEvent::ChatDraftChanged {
                draft: "/web_ inspect".to_string(),
            },
        ];
        let checkpoint = replay_checkpoint(&events);
        let restored = NativeStateKernel::restore(&checkpoint).checkpoint();

        assert_eq!(checkpoint, restored);
        assert_eq!(checkpoint.events.len(), 3);
        assert!(!checkpoint.event_log_hash.is_empty());
        assert!(!checkpoint.state_hash.is_empty());
        assert_eq!(checkpoint.projection.active_section, "webexplorer");
        assert!(checkpoint.projection.provider_status.contains("openrouter"));
    }

    #[test]
    fn checkpoint_json_is_stable_and_schema_tagged() {
        let checkpoint = replay_checkpoint(&[NativeUiEvent::Navigate {
            section: "real-estate".to_string(),
        }]);
        let first = checkpoint_json(&checkpoint);
        let second = checkpoint_json(&checkpoint);

        assert_eq!(first, second);
        assert!(first.contains("ingen.native_front.state_checkpoint.v1"));
        assert!(first.contains("real-estate"));
    }
}
