import { createHash } from "node:crypto";
import type {
  ComposerUploadPreview,
  NativeSection,
  SearchArchiveAttachmentRef,
  SearchArchiveContextLine,
  SearchArchiveContentScope,
  SearchArchiveCreatedInAppSource,
  SearchArchiveFileOrigin,
  SearchArchiveFileType,
  SearchArchiveHit,
  SearchArchiveRequest,
  SearchArchiveResult,
  SearchArchiveScope,
  SearchArchiveSessionScope,
  TranscriptMessage
} from "../shared/ipc-contract.js";
import {
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SEARCHARCHIVE_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const SEARCHARCHIVE_COMMAND = BRAIN_SEARCHARCHIVE_COMMAND;
export const SEARCHARCHIVE_RESULT_SCHEMA = BRAIN_SEARCHARCHIVE_RESULT_SCHEMA;

const DEFAULT_TOP_K = 5;
const MAX_TOP_K = 10;
const DEFAULT_CONTEXT_TURNS = 1;
const MAX_CONTEXT_TURNS = 3;
const MAX_QUERY_CHARS = 240;
const MAX_SNIPPET_CHARS = 420;
const MAX_CONTEXT_CHARS = 520;
const SEARCHARCHIVE_TEMPLATE_RESULT_SCHEMA = "forge.brain.searcharchive.template_result.v1";

export interface SearchArchiveTemplateResult {
  schema: typeof SEARCHARCHIVE_TEMPLATE_RESULT_SCHEMA;
  command: typeof SEARCHARCHIVE_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    sessionScope: SearchArchiveSessionScope[];
    contentScope: SearchArchiveContentScope[];
    fileOrigin: SearchArchiveFileOrigin[];
    createdInAppSources: SearchArchiveCreatedInAppSource[];
    fileTypes: SearchArchiveFileType[];
  };
  proofHash: string;
}

export type SearchArchiveCodeAct =
  | { kind: "template"; result: SearchArchiveTemplateResult }
  | { kind: "request"; request: SearchArchiveRequest };

export interface ChatArchiveSessionMeta {
  sessionId: string;
  title: string;
  section: NativeSection;
  workspaceLabel: string;
  date: string;
  archived: boolean;
}

export interface ChatArchiveAttachment extends ComposerUploadPreview {
  localPath?: string;
  mimeType?: string;
}

export interface ChatArchiveMessage {
  turnId: string;
  role: "user" | "assistant" | "system";
  text: string;
  createdAt: string;
  attachments: ChatArchiveAttachment[];
  proofHash: string;
}

export interface ChatArchiveSession {
  schema: "forge.brain.chat_session_archive.v1";
  sessionId: string;
  title: string;
  section: NativeSection;
  workspaceLabel: string;
  date: string;
  createdAt: string;
  updatedAt: string;
  archived: boolean;
  messages: ChatArchiveMessage[];
  proofHash: string;
}

interface SearchCandidate {
  session: ChatArchiveSession;
  message: ChatArchiveMessage;
  messageIndex: number;
  sourceType: SearchArchiveHit["sourceType"];
  matchedField: SearchArchiveHit["matchedField"];
  matchedText: string;
  score: number;
  attachment?: ChatArchiveAttachment;
}

const SEARCHARCHIVE_SESSION_SCOPES: SearchArchiveSessionScope[] = ["current", "recent", "archived", "all"];
const SEARCHARCHIVE_CONTENT_SCOPES: SearchArchiveContentScope[] = ["messages", "files", "artifacts", "all"];
const SEARCHARCHIVE_FILE_ORIGINS: SearchArchiveFileOrigin[] = ["uploaded", "created_in_app", "all"];
const SEARCHARCHIVE_CREATED_IN_APP_SOURCES: SearchArchiveCreatedInAppSource[] = [
  "agent",
  "scrapers",
  "image_generation",
  "image_edit",
  "compute",
  "banger_3d",
  "other"
];
const SEARCHARCHIVE_FILE_TYPES: SearchArchiveFileType[] = [
  "image",
  "pdf",
  "text",
  "code",
  "markdown",
  "csv",
  "json",
  "html",
  "audio",
  "video",
  "model3d",
  "other"
];

function archiveMessageAttachments(message: { attachments?: ChatArchiveAttachment[] | null }): ChatArchiveAttachment[] {
  return Array.isArray(message.attachments) ? message.attachments : [];
}

function sessionTitleMessage(session: ChatArchiveSession): ChatArchiveMessage {
  const createdAt = session.updatedAt || session.createdAt || `${session.date}T00:00:00.000Z`;
  return {
    turnId: `session-title-${stableSearchArchiveHash({ sessionId: session.sessionId, title: session.title }).slice(0, 16)}`,
    role: "system",
    text: session.title,
    createdAt,
    attachments: [],
    proofHash: stableSearchArchiveHash({ sessionId: session.sessionId, title: session.title, createdAt })
  };
}

export function stableSearchArchiveHash(value: unknown): string {
  return createHash("sha256").update(stableJson(value)).digest("hex");
}

export function searchArchiveTemplateResult(reason: SearchArchiveTemplateResult["reason"] = "empty_command"): SearchArchiveTemplateResult {
  const templateProofHash = searchArchiveTemplateProofHash();
  const template = [
    `${SEARCHARCHIVE_COMMAND}`,
    `template_proof_hash="sha256:${templateProofHash}"`,
    'query=""',
    "keywords=[]",
    'date_from=""',
    'date_to=""',
    'session_scope="current|recent|archived|all"',
    'content_scope="messages|files|artifacts|all"',
    'file_origin="uploaded|created_in_app|all"',
    'created_in_app_sources=["agent","scrapers","image_generation","image_edit","compute","banger_3d","other"]',
    'file_types=["image","pdf","text","code","markdown","csv","json","html","audio","video","model3d","other"]',
    "top_k=10",
    "context_turns=3",
    "include_file_previews=true",
    "include_artifact_refs=true"
  ].join("\n");
  const result: SearchArchiveTemplateResult = {
    schema: SEARCHARCHIVE_TEMPLATE_RESULT_SCHEMA,
    command: SEARCHARCHIVE_COMMAND,
    status: "template",
    reason,
    template,
    allowedValues: {
      sessionScope: SEARCHARCHIVE_SESSION_SCOPES,
      contentScope: SEARCHARCHIVE_CONTENT_SCOPES,
      fileOrigin: SEARCHARCHIVE_FILE_ORIGINS,
      createdInAppSources: SEARCHARCHIVE_CREATED_IN_APP_SOURCES,
      fileTypes: SEARCHARCHIVE_FILE_TYPES
    },
    proofHash: ""
  };
  result.proofHash = stableSearchArchiveHash({ ...result, proofHash: "" });
  return result;
}

export function renderSearchArchiveTemplateResult(result: SearchArchiveTemplateResult): string {
  return [
    "SEARCHARCHIVE_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${searchArchiveTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function readSearchArchiveCodeAct(input: string): SearchArchiveCodeAct | undefined {
  const trimmed = searchArchiveCodeActText(input).trim();
  if (!trimmed) {
    return undefined;
  }
  const body = trimmed.slice(SEARCHARCHIVE_COMMAND.length).trim();
  if (!body) {
    return { kind: "template", result: searchArchiveTemplateResult("empty_command") };
  }
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: searchArchiveTemplateResult("template_required") };
  }
  const request = parseSearchArchiveCodeAct(input);
  if (!request || !request.query.trim()) {
    return { kind: "template", result: searchArchiveTemplateResult("template_required") };
  }
  return { kind: "request", request };
}

export function parseSearchArchiveCodeAct(input: string): SearchArchiveRequest | undefined {
  const trimmed = searchArchiveCodeActText(input).trim();
  if (!trimmed) {
    return undefined;
  }
  const body = trimmed.slice(SEARCHARCHIVE_COMMAND.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? body : "";
  const keywords = readStringList(fields.get("keywords") ?? fields.get("keyword"));
  const rawQuery = fields.get("query") ?? fields.get("q") ?? (keywords.length > 0 ? keywords.join(" ") : freeform);
  const query = clampText(rawQuery.trim(), MAX_QUERY_CHARS);
  return {
    query,
    keywords,
    dateFrom: readDateField(fields.get("date_from") ?? fields.get("dateFrom")),
    dateTo: readDateField(fields.get("date_to") ?? fields.get("dateTo")),
    scope: readScope(fields.get("session_scope") ?? fields.get("scope")),
    sessionScope: readScope(fields.get("session_scope") ?? fields.get("scope")),
    contentScope: readContentScope(fields.get("content_scope") ?? fields.get("contentScope")),
    fileOrigin: readFileOrigin(fields.get("file_origin") ?? fields.get("fileOrigin")),
    createdInAppSources: readChoiceList(fields.get("created_in_app_sources") ?? fields.get("createdInAppSources"), SEARCHARCHIVE_CREATED_IN_APP_SOURCES),
    fileTypes: readChoiceList(fields.get("file_types") ?? fields.get("fileTypes"), SEARCHARCHIVE_FILE_TYPES),
    topK: readBoundedInteger(fields.get("top_k") ?? fields.get("topK"), DEFAULT_TOP_K, 1, MAX_TOP_K),
    contextTurns: readContextTurns(fields.get("context_window") ?? fields.get("context_turns") ?? fields.get("contextTurns")),
    targets: splitTargets(fields.get("targets")),
    includeFilePreviews: readBoolean(fields.get("include_file_previews") ?? fields.get("includeFilePreviews"), true),
    includeArtifactRefs: readBoolean(fields.get("include_artifact_refs") ?? fields.get("includeArtifactRefs"), true),
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))
  };
}

function searchArchiveCodeActText(input: string): string {
  const commandIndex = input.indexOf(SEARCHARCHIVE_COMMAND);
  return commandIndex >= 0 ? input.slice(commandIndex) : "";
}

export function upsertArchiveMessage(
  sessions: Map<string, ChatArchiveSession>,
  meta: ChatArchiveSessionMeta,
  message: TranscriptMessage,
  createdAt: string
): ChatArchiveSession {
  const existing = sessions.get(meta.sessionId);
  const attachments = (message.attachments ?? []).map((attachment) => {
    const source = attachment as ComposerUploadPreview & { localPath?: unknown; mimeType?: unknown };
    const archived: ChatArchiveAttachment = { ...attachment };
    if (typeof source.localPath === "string" && source.localPath.trim()) {
      archived.localPath = source.localPath;
    }
    if (typeof source.mimeType === "string" && source.mimeType.trim()) {
      archived.mimeType = source.mimeType;
    }
    return archived;
  });
  const archivedMessage: ChatArchiveMessage = {
    turnId: message.id,
    role: message.role,
    text: message.text,
    createdAt,
    attachments,
    proofHash: message.proofHash
  };
  const session: ChatArchiveSession = existing ?? {
    schema: "forge.brain.chat_session_archive.v1",
    sessionId: meta.sessionId,
    title: meta.title,
    section: meta.section,
    workspaceLabel: meta.workspaceLabel,
    date: meta.date,
    createdAt,
    updatedAt: createdAt,
    archived: meta.archived,
    messages: [],
    proofHash: ""
  };
  session.title = meta.title || session.title;
  session.section = meta.section;
  session.workspaceLabel = meta.workspaceLabel;
  session.date = meta.date || session.date;
  session.archived = meta.archived;
  session.updatedAt = createdAt;
  const existingMessageIndex = session.messages.findIndex((item) => item.turnId === message.id);
  if (existingMessageIndex >= 0) {
    session.messages[existingMessageIndex] = archivedMessage;
  } else {
    session.messages.push(archivedMessage);
  }
  session.proofHash = archiveSessionProofHash(session);
  sessions.set(meta.sessionId, session);
  return session;
}

export function markArchiveSessionArchived(
  sessions: Map<string, ChatArchiveSession>,
  sessionId: string,
  archived: boolean,
  updatedAt: string
): boolean {
  const session = sessions.get(sessionId);
  if (!session) return false;
  session.archived = archived;
  session.updatedAt = updatedAt;
  session.proofHash = archiveSessionProofHash(session);
  return true;
}

export function searchArchiveSessions(
  sessions: ChatArchiveSession[],
  request: SearchArchiveRequest
): SearchArchiveResult {
  const query = clampText((request.query ?? "").trim(), MAX_QUERY_CHARS);
  const scope = readScope(request.sessionScope ?? request.scope);
  const contentScope = request.contentScope ?? "all";
  const fileOrigin = request.fileOrigin ?? "all";
  const createdInAppSources = new Set(request.createdInAppSources ?? []);
  const fileTypes = new Set(request.fileTypes ?? []);
  const topK = clampNumber(request.topK ?? DEFAULT_TOP_K, 1, MAX_TOP_K);
  const contextTurns = clampNumber(request.contextTurns ?? DEFAULT_CONTEXT_TURNS, 0, MAX_CONTEXT_TURNS);
  const queryTerms = tokenizeQuery([query, ...(request.keywords ?? [])].join(" "));
  const candidates: SearchCandidate[] = [];
  const searchableSessions = sessions.filter((session) => {
    if (scope === "current") return Boolean(request.currentSessionId) && session.sessionId === request.currentSessionId;
    if (scope === "recent") return !session.archived;
    if (scope === "archived") return session.archived;
    return true;
  }).filter((session) => sessionInDateRange(session, request.dateFrom, request.dateTo));

  for (const session of searchableSessions) {
    const includeTitles = contentScope === "messages" || contentScope === "all";
    const titleScore = includeTitles ? scoreText(session.title, query, queryTerms) : 0;
    if (titleScore > 0) {
      candidates.push({
        session,
        message: sessionTitleMessage(session),
        messageIndex: -1,
        sourceType: "session_title",
        matchedField: "session_title",
        matchedText: session.title,
        score: titleScore + recencyBonus(session.updatedAt)
      });
    }
    session.messages.forEach((message, messageIndex) => {
      const includeMessages = contentScope === "messages" || contentScope === "all";
      const includeFiles = contentScope === "files" || contentScope === "artifacts" || contentScope === "all";
      const messageScore = includeMessages ? scoreText(message.text, query, queryTerms) : 0;
      if (messageScore > 0 && messageInDateRange(message, request.dateFrom, request.dateTo)) {
        candidates.push({
          session,
          message,
          messageIndex,
          sourceType: "session_message",
          matchedField: "message_text",
          matchedText: message.text,
          score: messageScore + recencyBonus(message.createdAt)
        });
      }
      if (!includeFiles || !messageInDateRange(message, request.dateFrom, request.dateTo)) {
        return;
      }
      for (const attachment of archiveMessageAttachments(message)) {
        if (!attachmentMatchesFilters(attachment, fileOrigin, createdInAppSources, fileTypes)) {
          continue;
        }
        const attachmentName = attachment.name ?? "";
        const nameScore = scoreText(attachmentName, query, queryTerms);
        if (nameScore > 0) {
          candidates.push({
            session,
            message,
            messageIndex,
            sourceType: "attachment",
            matchedField: "attachment_name",
            matchedText: attachmentName,
            score: nameScore + 8 + recencyBonus(message.createdAt),
            attachment
          });
        }
        const attachmentText = attachmentTextForSearch(attachment);
        const attachmentTextScore = scoreText(attachmentText, query, queryTerms);
        if (attachmentTextScore > 0) {
          candidates.push({
            session,
            message,
            messageIndex,
            sourceType: "attachment",
            matchedField: "attachment_text",
            matchedText: attachmentText,
            score: attachmentTextScore + 4 + recencyBonus(message.createdAt),
            attachment
          });
        }
      }
    });
  }

  const ranked = candidates
    .sort((left, right) => right.score - left.score || right.message.createdAt.localeCompare(left.message.createdAt))
    .slice(0, topK);
  const hits = ranked.map((candidate, index) => searchHit(candidate, query, contextTurns, index + 1, request));
  const indexSnapshotHash = stableSearchArchiveHash(
    searchableSessions.map((session) => ({
      sessionId: session.sessionId,
      updatedAt: session.updatedAt,
      archived: session.archived,
      proofHash: session.proofHash,
      messages: session.messages.length
    }))
  );
  const result: SearchArchiveResult = {
    schema: SEARCHARCHIVE_RESULT_SCHEMA,
    query,
    scope,
    matchCount: candidates.length,
    returnedCount: hits.length,
    truncated: candidates.length > hits.length,
    tokenBudgetUsedEstimate: estimateTokens(JSON.stringify(hits)),
    indexSnapshotHash,
    hits,
    proofHash: ""
  };
  result.proofHash = stableSearchArchiveHash({ ...result, proofHash: "" });
  return result;
}

export function renderSearchArchiveResult(result: SearchArchiveResult): string {
  const lines = [
    "SEARCHARCHIVE_RESULT",
    `schema=${result.schema}`,
    `query=${JSON.stringify(result.query)}`,
    `scope=${result.scope}`,
    `match_count=${result.matchCount}`,
    `returned_count=${result.returnedCount}`,
    `truncated=${result.truncated}`,
    `token_budget_used_estimate=${result.tokenBudgetUsedEstimate}`,
    `index_snapshot_hash=sha256:${result.indexSnapshotHash}`,
    `proof_hash=sha256:${result.proofHash}`,
    "hits:"
  ];
  if (result.hits.length === 0) {
    lines.push("  []");
    lines.push("note=No matching archive context was found. Try a broader query or scope=all.");
    return lines.join("\n");
  }
  for (const hit of result.hits) {
    lines.push(`  - rank: ${hit.rank}`);
    lines.push(`    source_type: ${hit.sourceType}`);
    lines.push(`    session_id: ${JSON.stringify(hit.sessionId)}`);
    lines.push(`    session_title: ${JSON.stringify(hit.sessionTitle)}`);
    lines.push(`    turn_id: ${JSON.stringify(hit.turnId)}`);
    lines.push(`    role: ${hit.role}`);
    lines.push(`    created_at: ${JSON.stringify(hit.createdAt)}`);
    lines.push(`    matched_field: ${hit.matchedField}`);
    lines.push(`    score: ${hit.score.toFixed(3)}`);
    lines.push(`    snippet: ${JSON.stringify(hit.snippet)}`);
    renderContextLines(lines, "context_before", hit.contextBefore);
    renderContextLines(lines, "context_after", hit.contextAfter);
    renderAttachmentRefs(lines, hit.attachments);
    lines.push(`    evidence_hash: sha256:${hit.evidenceHash}`);
    lines.push(`    open_ref: ${JSON.stringify(hit.openRef)}`);
    lines.push(`    fetch_more_ref: ${JSON.stringify(hit.fetchMoreRef)}`);
  }
  lines.push("next_action=Use the snippets directly, or call search again with a narrower query/context_turns if more context is needed.");
  return lines.join("\n");
}

export function archiveSessionProofHash(session: ChatArchiveSession): string {
  return stableSearchArchiveHash({
    schema: session.schema,
    sessionId: session.sessionId,
    title: session.title,
    section: session.section,
    workspaceLabel: session.workspaceLabel,
    date: session.date,
    createdAt: session.createdAt,
    updatedAt: session.updatedAt,
    archived: session.archived,
    messages: session.messages.map((message) => ({
      turnId: message.turnId,
      role: message.role,
      textHash: stableSearchArchiveHash(message.text ?? ""),
      createdAt: message.createdAt,
      attachments: archiveMessageAttachments(message).map((attachment) => ({
        id: attachment.id,
        name: attachment.name ?? "",
        kind: attachment.kind,
        textPreviewHash: stableSearchArchiveHash(attachment.textPreview ?? ""),
        tablePreviewHash: stableSearchArchiveHash(attachment.tablePreview ?? [])
      })),
      proofHash: message.proofHash
    }))
  });
}

function searchHit(candidate: SearchCandidate, query: string, contextTurns: number, rank: number, request: SearchArchiveRequest): SearchArchiveHit {
  const { session, message, messageIndex } = candidate;
  const snippetNeedle = [query, ...(request.keywords ?? [])].join(" ");
  const snippet = snippetAround(candidate.matchedText, snippetNeedle);
  const contextBefore = messageIndex >= 0
    ? session.messages
        .slice(Math.max(0, messageIndex - contextTurns), messageIndex)
        .map(contextLine)
    : [];
  const contextAfter = messageIndex >= 0
    ? session.messages
        .slice(messageIndex + 1, Math.min(session.messages.length, messageIndex + 1 + contextTurns))
        .map(contextLine)
    : [];
  const hitAttachments = candidate.attachment ? [candidate.attachment] : archiveMessageAttachments(message);
  const attachments = hitAttachments.map((attachment) => attachmentRef(session.sessionId, attachment, request));
  const evidenceHash = stableSearchArchiveHash({
    sessionId: session.sessionId,
    turnId: message.turnId,
    matchedField: candidate.matchedField,
    snippet,
    contextBefore,
    contextAfter,
    attachments: attachments.map((attachment) => attachment.proofHash)
  });
  return {
    rank,
    sourceType: candidate.sourceType,
    sessionId: session.sessionId,
    sessionTitle: session.title,
    turnId: message.turnId,
    role: message.role,
    createdAt: message.createdAt,
    matchedField: candidate.matchedField,
    snippet,
    contextBefore,
    contextAfter,
    attachments,
    score: candidate.score,
    evidenceHash,
    openRef: `forge://archive/session/${encodeURIComponent(session.sessionId)}?turn=${encodeURIComponent(message.turnId)}`,
    fetchMoreRef: `archive_ctx_${stableSearchArchiveHash({ sessionId: session.sessionId, turnId: message.turnId }).slice(0, 16)}`
  };
}

function contextLine(message: ChatArchiveMessage): SearchArchiveContextLine {
  return {
    role: message.role,
    turnId: message.turnId,
    text: clampText(message.text ?? "", MAX_CONTEXT_CHARS),
    proofHash: message.proofHash
  };
}

function attachmentRef(sessionId: string, attachment: ComposerUploadPreview, request: SearchArchiveRequest): SearchArchiveAttachmentRef {
  const origin = attachmentOrigin(attachment);
  const createdInAppSource = attachmentCreatedInAppSource(attachment);
  const fileType = attachmentFileType(attachment);
  return {
    id: attachment.id,
    name: attachment.name ?? "",
    kind: attachment.kind,
    fileType,
    origin,
    createdInAppSource,
    textPreview: request.includeFilePreviews === false ? "" : clampText(attachment.textPreview ?? "", MAX_CONTEXT_CHARS),
    artifactRefIncluded: request.includeArtifactRefs !== false,
    proofHash: stableSearchArchiveHash({
      id: attachment.id,
      name: attachment.name ?? "",
      kind: attachment.kind,
      fileType,
      origin,
      createdInAppSource,
      textPreview: attachment.textPreview ?? "",
      tablePreview: attachment.tablePreview ?? []
    }),
    openRef: request.includeArtifactRefs === false ? "" : `forge://archive/file/${encodeURIComponent(sessionId)}/${encodeURIComponent(attachment.id)}`
  };
}

function renderContextLines(lines: string[], label: string, context: SearchArchiveContextLine[]): void {
  lines.push(`    ${label}:`);
  if (context.length === 0) {
    lines.push("      []");
    return;
  }
  for (const item of context) {
    lines.push(`      - role: ${item.role}`);
    lines.push(`        turn_id: ${JSON.stringify(item.turnId)}`);
    lines.push(`        text: ${JSON.stringify(item.text)}`);
    lines.push(`        proof_hash: sha256:${item.proofHash}`);
  }
}

function renderAttachmentRefs(lines: string[], attachments: SearchArchiveAttachmentRef[]): void {
  lines.push("    attachments:");
  if (attachments.length === 0) {
    lines.push("      []");
    return;
  }
  for (const attachment of attachments) {
    lines.push(`      - id: ${JSON.stringify(attachment.id)}`);
    lines.push(`        name: ${JSON.stringify(attachment.name)}`);
    lines.push(`        kind: ${attachment.kind}`);
    lines.push(`        file_type: ${attachment.fileType}`);
    lines.push(`        origin: ${attachment.origin}`);
    lines.push(`        created_in_app_source: ${attachment.createdInAppSource}`);
    lines.push(`        text_preview: ${JSON.stringify(attachment.textPreview)}`);
    lines.push(`        artifact_ref_included: ${attachment.artifactRefIncluded}`);
    lines.push(`        proof_hash: sha256:${attachment.proofHash}`);
    lines.push(`        open_ref: ${JSON.stringify(attachment.openRef)}`);
  }
}

function searchArchiveTemplateProofHash(): string {
  return stableSearchArchiveHash({
    command: SEARCHARCHIVE_COMMAND,
    schema: SEARCHARCHIVE_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "query",
      "keywords",
      "date_from",
      "date_to",
      "session_scope",
      "content_scope",
      "file_origin",
      "created_in_app_sources",
      "file_types",
      "top_k",
      "context_turns",
      "include_file_previews",
      "include_artifact_refs"
    ],
    allowedValues: {
      sessionScope: SEARCHARCHIVE_SESSION_SCOPES,
      contentScope: SEARCHARCHIVE_CONTENT_SCOPES,
      fileOrigin: SEARCHARCHIVE_FILE_ORIGINS,
      createdInAppSources: SEARCHARCHIVE_CREATED_IN_APP_SOURCES,
      fileTypes: SEARCHARCHIVE_FILE_TYPES
    }
  });
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === searchArchiveTemplateProofHash();
}

function normalizeProofHash(value: unknown): string {
  return String(value ?? "").trim().replace(/^sha256:/i, "");
}

function indentBlock(value: string, prefix: string): string {
  return value.split(/\r?\n/).map((line) => `${prefix}${line}`).join("\n");
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    const value = (match[2] ?? match[3] ?? match[4] ?? "").trim();
    fields.set(key, value);
  }
  return fields;
}

function readScope(value: unknown): SearchArchiveScope {
  if (value === "current" || value === "recent" || value === "archived" || value === "all") {
    return value;
  }
  return "all";
}

function readContentScope(value: unknown): SearchArchiveContentScope {
  return readChoice(value, SEARCHARCHIVE_CONTENT_SCOPES, "all");
}

function readFileOrigin(value: unknown): SearchArchiveFileOrigin {
  return readChoice(value, SEARCHARCHIVE_FILE_ORIGINS, "all");
}

function readChoice<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  const normalized = String(value ?? "").trim();
  return (allowed as readonly string[]).includes(normalized) ? normalized as T : fallback;
}

function readChoiceList<T extends string>(value: unknown, allowed: readonly T[]): T[] {
  const allowedSet = new Set<string>(allowed);
  return readStringList(value).filter((item): item is T => allowedSet.has(item));
}

function readStringList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map((item) => String(item).trim()).filter(Boolean).slice(0, 32);
  }
  const raw = String(value ?? "").trim();
  if (!raw) return [];
  const unbracketed = raw.replace(/^\[/, "").replace(/\]$/, "");
  return unbracketed
    .split(/[,|;\n]+/)
    .map((item) => item.trim().replace(/^["']|["']$/g, ""))
    .filter(Boolean)
    .slice(0, 32);
}

function readDateField(value: unknown): string | undefined {
  const raw = String(value ?? "").trim();
  return /^\d{4}-\d{2}-\d{2}$/.test(raw) ? raw : undefined;
}

function readBoolean(value: unknown, fallback: boolean): boolean {
  const raw = String(value ?? "").trim().toLowerCase();
  if (raw === "true" || raw === "yes" || raw === "1") return true;
  if (raw === "false" || raw === "no" || raw === "0") return false;
  return fallback;
}

function readBoundedInteger(value: unknown, fallback: number, min: number, max: number): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return clampNumber(parsed, min, max);
}

function readContextTurns(value: unknown): number {
  const raw = String(value ?? "");
  const match = raw.match(/(?:turns?|messages?)\s*:?\s*(\d+)/i) ?? raw.match(/^\s*(\d+)\s*$/);
  if (!match) return DEFAULT_CONTEXT_TURNS;
  return clampNumber(Number.parseInt(match[1] ?? "", 10), 0, MAX_CONTEXT_TURNS);
}

function splitTargets(value: unknown): string[] {
  if (typeof value !== "string" || !value.trim()) return [];
  return value
    .split(/[,\s|]+/)
    .map((item) => item.trim())
    .filter(Boolean)
    .slice(0, 12);
}

function sessionInDateRange(session: ChatArchiveSession, dateFrom: string | undefined, dateTo: string | undefined): boolean {
  const day = session.date || session.updatedAt.slice(0, 10) || session.createdAt.slice(0, 10);
  return dateInRange(day, dateFrom, dateTo);
}

function messageInDateRange(message: ChatArchiveMessage, dateFrom: string | undefined, dateTo: string | undefined): boolean {
  return dateInRange(message.createdAt.slice(0, 10), dateFrom, dateTo);
}

function dateInRange(day: string, dateFrom: string | undefined, dateTo: string | undefined): boolean {
  if (dateFrom && day < dateFrom) return false;
  if (dateTo && day > dateTo) return false;
  return true;
}

function attachmentMatchesFilters(
  attachment: ComposerUploadPreview,
  fileOrigin: SearchArchiveFileOrigin,
  createdInAppSources: Set<SearchArchiveCreatedInAppSource>,
  fileTypes: Set<SearchArchiveFileType>
): boolean {
  const origin = attachmentOrigin(attachment);
  if (fileOrigin !== "all" && origin !== fileOrigin) {
    return false;
  }
  const source = attachmentCreatedInAppSource(attachment);
  if (origin === "created_in_app" && createdInAppSources.size > 0 && !createdInAppSources.has(source)) {
    return false;
  }
  const fileType = attachmentFileType(attachment);
  if (fileTypes.size > 0 && !fileTypes.has(fileType)) {
    return false;
  }
  return true;
}

function scoreText(text: string, query: string, queryTerms: string[]): number {
  const normalizedText = normalizeSearchText(text);
  const normalizedQuery = normalizeSearchText(query);
  if (!normalizedQuery || !normalizedText) return 0;
  let score = 0;
  if (normalizedText.includes(normalizedQuery)) {
    score += 100;
  }
  let matchedTerms = 0;
  for (const term of queryTerms) {
    if (normalizedText.includes(term)) {
      matchedTerms += 1;
      score += 14;
    }
  }
  if (queryTerms.length > 0 && matchedTerms === queryTerms.length) {
    score += 28;
  }
  return score;
}

function tokenizeQuery(query: string): string[] {
  return Array.from(
    new Set(
      normalizeSearchText(query)
        .split(/[^a-z0-9_]+/i)
        .map((term) => term.trim())
        .filter((term) => term.length >= 2)
    )
  );
}

function normalizeSearchText(text: string | undefined | null): string {
  return String(text ?? "")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase()
    .replace(/\s+/g, " ")
    .trim();
}

function foldSnippetSearchText(text: string): { folded: string; indexMap: number[] } {
  const foldedParts: string[] = [];
  const indexMap: number[] = [];
  for (let index = 0; index < text.length; index += 1) {
    const folded = text[index]
      ?.normalize("NFD")
      .replace(/\p{Diacritic}/gu, "")
      .toLocaleLowerCase() ?? "";
    for (const char of folded) {
      foldedParts.push(char);
      indexMap.push(index);
    }
  }
  return { folded: foldedParts.join(""), indexMap };
}

function snippetAnchorIndex(clean: string, query: string): number {
  const { folded, indexMap } = foldSnippetSearchText(clean);
  const foldedQuery = foldSnippetSearchText(query).folded.replace(/\s+/g, " ").trim();
  if (foldedQuery) {
    const exactIndex = folded.indexOf(foldedQuery);
    if (exactIndex >= 0) {
      return indexMap[exactIndex] ?? exactIndex;
    }
  }

  const terms = tokenizeQuery(query);
  let bestFoldedIndex = -1;
  let bestScore = -1;
  const halfWindow = Math.floor(MAX_SNIPPET_CHARS / 2);
  for (const term of terms) {
    let index = folded.indexOf(term);
    while (index >= 0) {
      const windowStart = Math.max(0, index - halfWindow);
      const windowEnd = Math.min(folded.length, index + halfWindow);
      const score = terms.reduce((count, candidate) => {
        const candidateIndex = folded.indexOf(candidate, windowStart);
        return candidateIndex >= 0 && candidateIndex <= windowEnd ? count + 1 : count;
      }, 0);
      if (score > bestScore || (score === bestScore && (bestFoldedIndex < 0 || index < bestFoldedIndex))) {
        bestScore = score;
        bestFoldedIndex = index;
      }
      index = folded.indexOf(term, index + Math.max(1, term.length));
    }
  }

  return bestFoldedIndex >= 0 ? indexMap[bestFoldedIndex] ?? bestFoldedIndex : -1;
}

function snippetAround(text: string | undefined | null, query: string): string {
  const clean = String(text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= MAX_SNIPPET_CHARS) return clean;
  const index = snippetAnchorIndex(clean, query);
  const anchor = index >= 0 ? index : Math.floor(clean.length / 2);
  const start = Math.min(Math.max(0, anchor - Math.floor(MAX_SNIPPET_CHARS / 2)), Math.max(0, clean.length - MAX_SNIPPET_CHARS));
  const end = Math.min(clean.length, start + MAX_SNIPPET_CHARS);
  return `${start > 0 ? "..." : ""}${clean.slice(start, end).trim()}${end < clean.length ? "..." : ""}`;
}

function attachmentTextForSearch(attachment: ComposerUploadPreview): string {
  const tablePreview = Array.isArray(attachment.tablePreview) ? attachment.tablePreview : [];
  return [
    attachment.kind,
    attachment.name ?? "",
    attachment.textPreview ?? "",
    tablePreview.map((row) => row.join(" ")).join("\n")
  ].filter(Boolean).join("\n");
}

function attachmentOrigin(attachment: ComposerUploadPreview): SearchArchiveFileOrigin {
  const url = String(attachment.url ?? "").toLowerCase();
  if (url.startsWith("ingen://upload-preview/")) {
    return "uploaded";
  }
  return "created_in_app";
}

function attachmentCreatedInAppSource(attachment: ComposerUploadPreview): SearchArchiveCreatedInAppSource {
  if (attachmentOrigin(attachment) === "uploaded") {
    return "other";
  }
  const haystack = `${attachment.id} ${attachment.name ?? ""} ${attachment.url ?? ""}`.toLowerCase();
  if (haystack.includes("scraper") || haystack.includes("crawl4ai") || haystack.includes("scrapling")) return "scrapers";
  if (haystack.includes("editimage") || haystack.includes("image_edit")) return "image_edit";
  if (haystack.includes("newimage") || haystack.includes("image_generation") || haystack.includes("generated-image")) return "image_generation";
  if (haystack.includes("compute") || haystack.includes("monster")) return "compute";
  if (attachment.kind === "model3d" || haystack.includes("banger") || haystack.includes("3d")) return "banger_3d";
  return "agent";
}

function attachmentFileType(attachment: ComposerUploadPreview): SearchArchiveFileType {
  const name = String(attachment.name ?? "").toLowerCase();
  if (attachment.kind === "image") return "image";
  if (attachment.kind === "video") return "video";
  if (attachment.kind === "model3d") return "model3d";
  if (attachment.kind === "pdf") return "pdf";
  if (attachment.kind === "text") {
    if (/\.(ts|tsx|js|jsx|rs|py|go|java|c|cpp|h|hpp|cs|swift|kt)$/i.test(name)) return "code";
    if (/\.mdx?$/i.test(name)) return "markdown";
    if (/\.csv$/i.test(name)) return "csv";
    if (/\.json$/i.test(name)) return "json";
    if (/\.html?$/i.test(name)) return "html";
    return "text";
  }
  if (/\.(mp3|wav|flac|ogg|m4a)$/i.test(name)) return "audio";
  if (/\.(mp4|mov|mkv|webm|avi)$/i.test(name)) return "video";
  return "other";
}

function recencyBonus(createdAt: string): number {
  const timestamp = Date.parse(createdAt);
  if (!Number.isFinite(timestamp)) return 0;
  const ageDays = Math.max(0, (Date.now() - timestamp) / 86_400_000);
  return Math.max(0, 5 - Math.min(5, ageDays / 14));
}

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

function clampNumber(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, Math.trunc(value)));
}

function clampText(text: string | undefined | null, maxChars: number): string {
  const clean = String(text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`).join(",")}}`;
}
