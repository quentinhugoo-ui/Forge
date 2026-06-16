import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const stylesSource = readFileSync(join(root, "src", "renderer", "styles.css"), "utf8");

describe("parallel transcript spacing", () => {
  it("keeps parallel panes at the same bottom distance from the chat bar as the single canvas", () => {
    const parallelGridBlock = stylesSource.match(/^\.parallelTranscriptGrid\s*\{[\s\S]*?\n\}/m)?.[0] ?? "";
    const parallelMessagesBlock = stylesSource.match(/^\.chatCanvas--parallelPane \.chatCanvas__messages\s*\{[\s\S]*?\n\}/m)?.[0] ?? "";

    expect(parallelGridBlock).toContain("bottom: var(--chat-canvas-bottom);");
    expect(parallelMessagesBlock).toContain("padding-bottom: var(--transcript-bottom-breathing-room);");
    expect(parallelMessagesBlock).toContain("scroll-padding-bottom: var(--transcript-bottom-breathing-room);");
    expect(parallelMessagesBlock).not.toContain("var(--chat-canvas-bottom) + var(--transcript-bottom-breathing-room)");
  });
});
