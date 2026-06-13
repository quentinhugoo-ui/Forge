import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const mainSource = readFileSync(join(root, "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(root, "src", "preload", "preload.ts"), "utf8");
const storeSource = readFileSync(join(root, "src", "renderer", "panels-chat-bottom-store.ts"), "utf8");
const ipcContractSource = readFileSync(join(root, "src", "shared", "ipc-contract.ts"), "utf8");
const animationSource = readFileSync(join(root, "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");

describe("assistant progressive response feed", () => {
  it("pushes transcript snapshot events while a chat command is still running", () => {
    expect(ipcContractSource).toContain("export interface PanelsChatBottomSnapshotEvent");
    expect(preloadSource).toContain('ipcRenderer.on("forge:panels-chat-bottom-snapshot-event"');
    expect(storeSource).toContain("api?.onPanelsChatBottomSnapshotEvent?.");
    expect(mainSource).toContain('webContents.send("forge:panels-chat-bottom-snapshot-event"');
  });

  it("seeds large assistant messages before committing the full final response", () => {
    expect(mainSource).toContain("ASSISTANT_PROGRESSIVE_SEED_MIN_CHARS");
    expect(mainSource).toContain("commitAssistantMessageWithProgressiveSeed");
    expect(mainSource).toContain('emitPanelsChatBottomSnapshotEvent("assistant_progressive_seed"');
    expect(storeSource).toContain("incomingHasFreshAssistantResponse");
  });

  it("keeps the existing assistant writing animation implementation untouched", () => {
    expect(animationSource).toContain("function AnimatedAssistantText");
    expect(animationSource).toContain("const animationSource = useMemo(() => assistantVisibleAnimationSource(message.text), [message.text]);");
    expect(animationSource).toContain("const visibleText = animationSource.slice(0, visibleCharacters);");
  });
});
