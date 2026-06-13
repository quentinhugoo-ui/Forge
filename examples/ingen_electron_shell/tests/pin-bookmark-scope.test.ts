import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const rendererSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const generatedIpcSource = readFileSync(join(process.cwd(), "src", "shared", "generated", "forge-ipc.generated.ts"), "utf8");
const storeSource = readFileSync(join(process.cwd(), "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");

describe("PIN bookmark session scope", () => {
  it("carries the active chat session id through the typed panels snapshot", () => {
    expect(generatedIpcSource).toContain("activeSessionId: string;");
    expect(storeSource).toContain('activeSessionId: ""');
    expect(mainSource).toContain("activeSessionId: panelsChatBottomState.activeSessionId");
  });

  it("stores pinned transcript chapters by active session, not one global key", () => {
    expect(rendererSource).toContain('const LEGACY_PINS_STORAGE_KEY = "ingen.chat.pins.v1"');
    expect(rendererSource).toContain('const SESSION_PINS_STORAGE_KEY_PREFIX = "ingen.chat.session-pins.v1"');
    expect(rendererSource).toContain("function pinsStorageKey(activeSessionId: string)");
    expect(rendererSource).toContain("storage?.removeItem(LEGACY_PINS_STORAGE_KEY)");
    expect(rendererSource).toContain('key={snapshot.activeSessionId || "draft-session"}');
    expect(rendererSource).not.toContain("getItem(PINS_STORAGE_KEY)");
    expect(rendererSource).not.toContain("setItem(PINS_STORAGE_KEY");
  });
});
