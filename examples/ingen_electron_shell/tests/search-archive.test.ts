import { describe, expect, it } from "vitest";
import { BRAIN_SEARCHARCHIVE_RESULT_SCHEMA } from "../src/shared/ipc-contract";
import {
  archiveSessionProofHash,
  parseSearchArchiveCodeAct,
  readSearchArchiveCodeAct,
  renderSearchArchiveResult,
  renderSearchArchiveTemplateResult,
  searchArchiveSessions,
  upsertArchiveMessage,
  type ChatArchiveSession,
  type ChatArchiveSessionMeta
} from "../src/main/search-archive";
import type { TranscriptMessage } from "../src/shared/ipc-contract";

function sessionMeta(overrides: Partial<ChatArchiveSessionMeta> = {}): ChatArchiveSessionMeta {
  return {
    sessionId: "chat-gingerbread",
    title: "Recette Noel",
    date: "2026-06-10",
    section: "forge",
    workspaceLabel: "Forge",
    archived: false,
    ...overrides
  };
}

function message(id: string, role: TranscriptMessage["role"], text: string): TranscriptMessage {
  return {
    id,
    role,
    text,
    attachments: [],
    proofHash: `proof-${id}`
  };
}

describe("search archive CodeAct", () => {
  it("requires the two-phase template before archive execution", () => {
    const templateStep = readSearchArchiveCodeAct("/searcharchive_");
    expect(templateStep?.kind).toBe("template");
    expect(templateStep?.kind === "template" ? renderSearchArchiveTemplateResult(templateStep.result) : "").toContain("SEARCHARCHIVE_TEMPLATE_RESULT");
    const template = templateStep?.kind === "template" ? templateStep.result.template : "";
    expect(template).toContain('content_scope="messages|all"');
    expect(template).not.toContain("file_origin");
    expect(template).not.toContain("created_in_app_sources");
    expect(template).not.toContain("file_types");
    expect(template).not.toContain("include_file_previews");

    const directFilled = readSearchArchiveCodeAct('/searcharchive_ query="pain d epices" session_scope=archived');
    const proseThenTemplate = readSearchArchiveCodeAct("Je cherche dans les archives.\n/searcharchive_");
    expect(directFilled?.kind).toBe("template");
    expect(directFilled?.kind === "template" ? directFilled.result.reason : "").toBe("template_required");
    expect(proseThenTemplate?.kind).toBe("template");
  });

  it("parses the bounded /searcharchive_ template slots after proof handoff", () => {
    const templateStep = readSearchArchiveCodeAct("/searcharchive_");
    const proofHash = templateStep?.kind === "template" ? templateStep.result.template.match(/template_proof_hash="sha256:([^"]+)"/)?.[1] : "";
    const request = parseSearchArchiveCodeAct(`/searcharchive_
template_proof_hash="sha256:${proofHash}"
query="pain d epices"
keywords=["orange","gingembre"]
date_from="2026-06-01"
date_to="2026-06-30"
session_scope=archived
content_scope=messages
top_k=3
context_turns=2`);
    const executable = readSearchArchiveCodeAct(`/searcharchive_ template_proof_hash="sha256:${proofHash}" query="pain d epices" session_scope=recent top_k=2 context_turns=1`);

    expect(request).toMatchObject({
      query: "pain d epices",
      keywords: ["orange", "gingembre"],
      dateFrom: "2026-06-01",
      dateTo: "2026-06-30",
      scope: "archived",
      sessionScope: "archived",
      contentScope: "messages",
      topK: 3,
      contextTurns: 2
    });
    expect(executable?.kind === "request" ? executable.request : undefined).toMatchObject({
      query: "pain d epices",
      scope: "recent",
      topK: 2,
      contextTurns: 1
    });
  });

  it("returns snippets, neighbor turns and refs instead of full session context", () => {
    const sessions = new Map<string, ChatArchiveSession>();
    const meta = sessionMeta();
    upsertArchiveMessage(sessions, meta, message("turn-1", "user", "Je veux une recette de Noel."), "2026-06-10T10:00:00Z");
    upsertArchiveMessage(sessions, meta, message("turn-2", "assistant", "On peut partir sur une base moelleuse."), "2026-06-10T10:01:00Z");
    upsertArchiveMessage(
      sessions,
      meta,
      message("turn-3", "user", "Retrouve le contexte sur le pain d epices avec orange confite."),
      "2026-06-10T10:02:00Z"
    );
    upsertArchiveMessage(sessions, meta, message("turn-4", "assistant", "Le dosage precedent utilisait cannelle et gingembre."), "2026-06-10T10:03:00Z");

    const result = searchArchiveSessions(Array.from(sessions.values()), {
      query: "pain d epices",
      scope: "all",
      topK: 1,
      contextTurns: 1
    });

    expect(result.schema).toBe(BRAIN_SEARCHARCHIVE_RESULT_SCHEMA);
    expect(result.returnedCount).toBe(1);
    expect(result.hits[0]?.snippet).toContain("pain d epices");
    expect(result.hits[0]?.contextBefore).toHaveLength(1);
    expect(result.hits[0]?.contextAfter).toHaveLength(1);
    expect(result.hits[0]?.openRef).toContain("forge://archive/session/chat-gingerbread");
    expect(result.hits[0]?.fetchMoreRef).toMatch(/^archive_ctx_/);

    const rendered = renderSearchArchiveResult(result);
    expect(rendered).toContain("SEARCHARCHIVE_RESULT");
    expect(rendered).toContain("snippet:");
    expect(rendered).not.toContain("visual_summary");
  });

  it("returns attachment refs on matched message turns without searching by file", () => {
    const sessions = new Map<string, ChatArchiveSession>();
    upsertArchiveMessage(
      sessions,
      sessionMeta({ sessionId: "chat-file", title: "Documents" }),
      {
        ...message("turn-file", "user", "Voici le contexte pain d epices."),
        attachments: [
          {
            id: "file-1",
            name: "liste-pain-epices.txt",
            kind: "text",
            url: "ingen://attachment/file-1",
            textPreview: "farine miel cannelle pain d epices",
            tablePreview: []
          }
        ]
      },
      "2026-06-10T11:00:00Z"
    );

    const result = searchArchiveSessions(Array.from(sessions.values()), {
      query: "contexte pain",
      scope: "all",
      topK: 2,
      contextTurns: 0
    });

    expect(result.hits[0]?.matchedField).toBe("message_text");
    expect(result.hits[0]?.attachments[0]?.name).toBe("liste-pain-epices.txt");
    expect(renderSearchArchiveResult(result)).not.toContain("personne sur scooter");
  });

  it("centers long snippets on matching query terms when the full query phrase is absent", () => {
    const sessions = new Map<string, ChatArchiveSession>();
    const distantIntro = Array.from({ length: 42 }, (_, index) => `detail neutre ${index}`).join(" ");
    upsertArchiveMessage(
      sessions,
      sessionMeta({ sessionId: "chat-witcher", title: "Heart Stone Witcher" }),
      message(
        "turn-witcher",
        "assistant",
        `${distantIntro}. Le passage important parle de The Witcher 3 et de Hearts of Stone, avec Geralt et Olgierd au centre du recit.`
      ),
      "2026-06-10T12:00:00Z"
    );

    const result = searchArchiveSessions(Array.from(sessions.values()), {
      query: "witcher heart stone",
      scope: "all",
      topK: 1,
      contextTurns: 0
    });

    expect(result.returnedCount).toBe(1);
    expect(result.hits[0]?.snippet.toLowerCase()).toContain("witcher");
    expect(result.hits[0]?.snippet.toLowerCase()).toContain("stone");
  });

  it("keeps legacy archived sessions without attachments searchable", () => {
    const legacySession = {
      schema: "forge.brain.chat_session_archive.v1",
      sessionId: "chat-legacy-gmail",
      title: "ouvre gmail",
      date: "2026-06-11",
      section: "forge",
      workspaceLabel: "Forge",
      createdAt: "2026-06-11T09:00:00Z",
      updatedAt: "2026-06-11T09:00:00Z",
      archived: false,
      messages: [
        {
          turnId: "turn-legacy",
          role: "assistant",
          text: "Le LLM a repondu naturellement avant d'activer Gmail.",
          createdAt: "2026-06-11T09:00:00Z",
          proofHash: "proof-legacy"
        }
      ],
      proofHash: "proof-session"
    } as unknown as ChatArchiveSession;

    const result = searchArchiveSessions([legacySession], {
      query: "gmail",
      scope: "all",
      topK: 1,
      contextTurns: 0
    });

    expect(result.returnedCount).toBe(1);
    expect(result.hits[0]?.attachments).toEqual([]);
    expect(archiveSessionProofHash(legacySession)).toMatch(/^[a-f0-9]{64}$/);
  });
});
