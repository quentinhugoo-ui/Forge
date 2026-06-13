import { createHash } from "node:crypto";
import type {
  ComposerUploadPreview,
  NativeSection,
  SearchArchiveAttachmentRef,
  SearchArchiveContextLine,
  SearchArchiveHit,
  SearchArchiveRequest,
  SearchArchiveResult,
  SearchArchiveScope,
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
}

function archiveMessageAttachments(message: { attachments?: ChatArchiveAttachment[] | null }): ChatArchiveAttachment[] {
  return Array.isArray(message.attachments) ? message.attachments : [];
}

export function stableSearchArchiveHash(value: unknown): string {
  return createHash("sha256").update(stableJson(value)).digest("hex");
}

export function parseSearchArchiveCodeAct(input: string): SearchArchiveRequest | undefined {
  const trimmed = input.trim();
  if (!trimmed.startsWith(SEARCHARCHIVE_COMMAND)) {
    return undefined;
  }
  const body = trimmed.slice(SEARCHARCHIVE_COMMAND.length).trim();
  const fields = parseTemplateFields(body);
  const freeform = fields.size === 0 ? body : "";
  const query = clampText((fields.get("query") ?? fields.get("q") ?? fields.get("keyword") ?? freeform).trim(), MAX_QUERY_CHARS);
  if (!query) {
    return {
      query: "",
      scope: readScope(fields.get("scope")),
      topK: readBoundedInteger(fields.get("top_k") ?? fields.get("topK"), DEFAULT_TOP_K, 1, MAX_TOP_K),
      contextTurns: readContextTurns(fields.get("context_window") ?? fields.get("contextTurns"))
    };
  }
  return {
    query,
    scope: readScope(fields.get("scope")),
    topK: readBoundedInteger(fields.get("top_k") ?? fields.get("topK"), DEFAULT_TOP_K, 1, MAX_TOP_K),
    contextTurns: readContextTurns(fields.get("context_window") ?? fields.get("contextTurns")),
    targets: splitTargets(fields.get("targets"))
  };
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
  const scope = readScope(request.scope);
  const topK = clampNumber(request.topK ?? DEFAULT_TOP_K, 1, MAX_TOP_K);
  const contextTurns = clampNumber(request.contextTurns ?? DEFAULT_CONTEXT_TURNS, 0, MAX_CONTEXT_TURNS);
  const queryTerms = tokenizeQuery(query);
  const candidates: SearchCandidate[] = [];
  const searchableSessions = sessions.filter((session) => {
    if (scope === "recent") return !session.archived;
    if (scope === "archived") return session.archived;
    return true;
  });

  for (const session of searchableSessions) {
    session.messages.forEach((message, messageIndex) => {
      const messageScore = scoreText(message.text, query, queryTerms);
      if (messageScore > 0) {
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
      for (const attachment of archiveMessageAttachments(message)) {
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
            score: nameScore + 8 + recencyBonus(message.createdAt)
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
            score: attachmentTextScore + 4 + recencyBonus(message.createdAt)
          });
        }
      }
    });
  }

  const ranked = candidates
    .sort((left, right) => right.score - left.score || right.message.createdAt.localeCompare(left.message.createdAt))
    .slice(0, topK);
  const hits = ranked.map((candidate, index) => searchHit(candidate, query, contextTurns, index + 1));
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
  lines.push("next_action=Use the snippets directly, or call search again with a narrower query/context_window if more context is needed.");
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

function searchHit(candidate: SearchCandidate, query: string, contextTurns: number, rank: number): SearchArchiveHit {
  const { session, message, messageIndex } = candidate;
  const contextBefore = session.messages
    .slice(Math.max(0, messageIndex - contextTurns), messageIndex)
    .map(contextLine);
  const contextAfter = session.messages
    .slice(messageIndex + 1, Math.min(session.messages.length, messageIndex + 1 + contextTurns))
    .map(contextLine);
  const attachments = archiveMessageAttachments(message).map((attachment) => attachmentRef(session.sessionId, attachment));
  const evidenceHash = stableSearchArchiveHash({
    sessionId: session.sessionId,
    turnId: message.turnId,
    matchedField: candidate.matchedField,
    snippet: snippetAround(candidate.matchedText, query),
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
    snippet: snippetAround(candidate.matchedText, query),
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

function attachmentRef(sessionId: string, attachment: ComposerUploadPreview): SearchArchiveAttachmentRef {
  return {
    id: attachment.id,
    name: attachment.name ?? "",
    kind: attachment.kind,
    textPreview: clampText(attachment.textPreview ?? "", MAX_CONTEXT_CHARS),
    proofHash: stableSearchArchiveHash({
      id: attachment.id,
      name: attachment.name ?? "",
      kind: attachment.kind,
      textPreview: attachment.textPreview ?? "",
      tablePreview: attachment.tablePreview ?? []
    }),
    openRef: `forge://archive/file/${encodeURIComponent(sessionId)}/${encodeURIComponent(attachment.id)}`
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
    lines.push(`        text_preview: ${JSON.stringify(attachment.textPreview)}`);
    lines.push(`        proof_hash: sha256:${attachment.proofHash}`);
    lines.push(`        open_ref: ${JSON.stringify(attachment.openRef)}`);
  }
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
  if (value === "recent" || value === "archived" || value === "all") {
    return value;
  }
  return "all";
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

function snippetAround(text: string | undefined | null, query: string): string {
  const clean = String(text ?? "").replace(/\s+/g, " ").trim();
  if (clean.length <= MAX_SNIPPET_CHARS) return clean;
  const lowerClean = clean.toLocaleLowerCase();
  const lowerQuery = query.toLocaleLowerCase();
  const index = lowerQuery ? lowerClean.indexOf(lowerQuery) : -1;
  const anchor = index >= 0 ? index : Math.floor(clean.length / 2);
  const start = Math.max(0, anchor - Math.floor(MAX_SNIPPET_CHARS / 2));
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
