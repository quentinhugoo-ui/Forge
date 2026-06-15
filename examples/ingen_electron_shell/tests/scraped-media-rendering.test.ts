import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const rendererSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(process.cwd(), "src", "renderer", "styles.css"), "utf8");
const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");

describe("scraped media transcript rendering", () => {
  it("attaches scraper image and video results to assistant messages", () => {
    expect(mainSource).toContain("scrapersVisualAttachments(result)");
    expect(mainSource).toContain("visualAttachments.length > 0");
    expect(mainSource).toContain("attachments: visualAttachments.length > 0");
  });

  it("renders assistant scraped media with the existing transcript blob stack", () => {
    expect(rendererSource).toContain('role === "assistant"');
    expect(rendererSource).toContain('<div className="transcriptFloatMedia" aria-label="Attached visual media">');
    expect(rendererSource).toContain("<TranscriptAttachmentStack previews={visualAttachments} />");
  });

  it("compresses assistant text around scraped blobs with the existing media frame classes", () => {
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptFloatMedia) .transcriptTextFrame");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-landscape) .transcriptTextFrame");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-nineSixteen) .transcriptTextFrame");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-square) .transcriptTextFrame");
  });
});
