import { mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import {
  agentActionEventCommandForRequest,
  executeAgentActionRequest,
  type AgentActionHostConfig
} from "../src/main/agent-action-host";
import {
  agentActionLiveVisibleText,
  extractAgentActionJsonRequest,
  removeAgentActionJsonFragment
} from "../src/main/agent-action-loop";
import type { AgentActionRequest } from "../src/shared/ipc-contract";

async function withTempWorkspace<T>(run: (config: AgentActionHostConfig) => Promise<T>): Promise<T> {
  const root = join(tmpdir(), `ingen-agent-loop-realistic-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  await mkdir(root, { recursive: true });
  const config: AgentActionHostConfig = {
    workspaceRoot: root,
    workspaceActive: true,
    cwd: root,
    platform: process.platform
  };
  try {
    return await run(config);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

describe("agent action loop parser", () => {
  it("extracts an action marker even when the model glues it to prose", () => {
    const text = "Je vais d'abord regarder le contenu de ton bureau sans supprimer quoi que ce soit.AGENT_ACTION_JSON\n{\"action\":\"list\",\"scope\":\"computer\",\"path\":\"C:\\\\Users\\\\quent\\\\Desktop\"}";

    const extracted = extractAgentActionJsonRequest(text);

    expect(extracted?.request).toEqual({
      action: "list",
      scope: "computer",
      path: "C:\\Users\\quent\\Desktop"
    });
    expect(removeAgentActionJsonFragment(text, extracted!)).toBe("Je vais d'abord regarder le contenu de ton bureau sans supprimer quoi que ce soit.");
  });

  it("hides complete and partial control markers from the live transcript", () => {
    const partial = "Je regarde d'abord le bureau.AGENT_ACTION_JSON";
    const complete = `${partial} {\"action\":\"list\",\"scope\":\"computer\",\"path\":\"C:\\\\Users\\\\quent\\\\Desktop\"}`;

    expect(agentActionLiveVisibleText(partial)).toBe("Je regarde d'abord le bureau.");
    expect(agentActionLiveVisibleText(complete)).toBe("Je regarde d'abord le bureau.");
  });

  it("executes a realistic paragraph-action-result loop without fake completion", async () => {
    await withTempWorkspace(async (config) => {
      const turns: Array<{ paragraph: string; request: AgentActionRequest }> = [
        {
          paragraph: "Je cree d'abord une note de controle dans l'espace de travail, avec verification de lecture juste apres.",
          request: {
            action: "document_write_text",
            path: "loop/proof.txt",
            content: "loop proof\n"
          }
        },
        {
          paragraph: "La note existe maintenant; je lis son artefact pour rattacher la suite a une preuve runtime.",
          request: {
            action: "document_inspect",
            path: "loop/proof.txt"
          }
        },
        {
          paragraph: "Cette derniere demande touche au systeme Windows: sans confirmation explicite, elle doit rester bloquee.",
          request: {
            action: "package_install_update",
            packageId: "Git.Git",
            command: "upgrade"
          }
        }
      ];
      const runtimeEvents: Array<{ kind: "tool_call_started" | "tool_result" | "tool_call_completed"; command: string; accepted?: boolean }> = [];
      let finalText = "";

      for (const turn of turns) {
        const streamed = `${turn.paragraph}\nAGENT_ACTION_JSON ${JSON.stringify(turn.request)}`;
        expect(agentActionLiveVisibleText(streamed)).toBe(turn.paragraph);
        const extracted = extractAgentActionJsonRequest(streamed);
        expect(extracted?.request).toEqual(turn.request);

        const command = agentActionEventCommandForRequest(turn.request);
        runtimeEvents.push({ kind: "tool_call_started", command });
        const result = await executeAgentActionRequest(config, turn.request);
        runtimeEvents.push({ kind: "tool_result", command, accepted: result.accepted });
        runtimeEvents.push({ kind: "tool_call_completed", command, accepted: result.accepted });

        expect(result.proofHash).toMatch(/^[a-f0-9]{64}$/);
        if (result.accepted) {
          expect(result.verification?.passed).toBe(true);
        } else {
          expect(result.userPresenceRequired || result.failureCategory === "denied").toBe(true);
        }
        finalText += `${turn.paragraph}\n\n${command}\n\nAGENT_ACTION_RESULT v1 ${JSON.stringify({
          accepted: result.accepted,
          action: result.action,
          proofHash: result.proofHash,
          verificationPassed: result.verification?.passed,
          error: result.error?.message
        })}\n\n`;
      }

      expect(runtimeEvents.map((event) => event.kind)).toEqual([
        "tool_call_started",
        "tool_result",
        "tool_call_completed",
        "tool_call_started",
        "tool_result",
        "tool_call_completed",
        "tool_call_started",
        "tool_result",
        "tool_call_completed"
      ]);
      expect(runtimeEvents.map((event) => event.command)).toEqual([
        "/agent_document_write_ path=\"loop/proof.txt\"",
        "/agent_document_write_ path=\"loop/proof.txt\"",
        "/agent_document_write_ path=\"loop/proof.txt\"",
        "/agent_document_inspect_ path=\"loop/proof.txt\"",
        "/agent_document_inspect_ path=\"loop/proof.txt\"",
        "/agent_document_inspect_ path=\"loop/proof.txt\"",
        "/agent_package_install_update_",
        "/agent_package_install_update_",
        "/agent_package_install_update_"
      ]);
      expect(runtimeEvents.at(-1)?.accepted).toBe(false);
      expect(finalText).toContain("requires confirmed:true");
      expect(finalText).not.toMatch(/\bdone\b/i);
    });
  });
});
