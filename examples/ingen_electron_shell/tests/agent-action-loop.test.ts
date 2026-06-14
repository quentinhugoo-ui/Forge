import { describe, expect, it } from "vitest";
import {
  agentActionLiveVisibleText,
  extractAgentActionJsonRequest,
  removeAgentActionJsonFragment
} from "../src/main/agent-action-loop";

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
});
