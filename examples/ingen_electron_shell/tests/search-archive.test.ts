import { describe, expect, it } from "vitest";
import { BRAIN_SEARCHARCHIVE_RESULT_SCHEMA } from "../src/shared/ipc-contract";
import {
  archiveSessionProofHash,
  parseSearchArchiveCodeAct,
  renderSearchArchiveResult,
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
  it("parses the bounded /searcharchive_ template slots", () => {
    const request = parseSearchArchiveCodeAct('/searcharchive_\nquery="pain d epices"\nscope=archived\ntop_k=3\ncontext_window=turns:2');
    const singleLineRequest = parseSearchArchiveCodeAct('/searcharchive_ query="pain d epices" scope=recent top_k=2 context_window=turns:1');

    expect(request).toMatchObject({
      query: "pain d epices",
      scope: "archived",
      topK: 3,
      contextTurns: 2
    });
    expect(singleLineRequest).toMatchObject({
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

  it("matches attachment names and text previews without generating semantic captions", () => {
    const sessions = new Map<string, ChatArchiveSession>();
    upsertArchiveMessage(
      sessions,
      sessionMeta({ sessionId: "chat-file", title: "Documents" }),
      {
        ...message("turn-file", "user", "Voici le fichier."),
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
      query: "pain epices",
      scope: "all",
      topK: 2,
      contextTurns: 0
    });

    expect(result.hits.some((hit) => hit.matchedField === "attachment_name" || hit.matchedField === "attachment_text")).toBe(true);
    expect(result.hits[0]?.attachments[0]?.name).toBe("liste-pain-epices.txt");
    expect(renderSearchArchiveResult(result)).not.toContain("personne sur scooter");
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
