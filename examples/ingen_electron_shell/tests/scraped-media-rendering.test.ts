import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const rendererSource = readFileSync(join(process.cwd(), "src", "renderer", "PanelsChatBottomSlice.tsx"), "utf8");
const appSource = readFileSync(join(process.cwd(), "src", "renderer", "App.tsx"), "utf8");
const canvasSource = readFileSync(join(process.cwd(), "src", "renderer", "CanvasSurfacesSlice.tsx"), "utf8");
const stylesSource = readFileSync(join(process.cwd(), "src", "renderer", "styles.css"), "utf8");
const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const scrapersSource = readFileSync(join(process.cwd(), "src", "main", "scrapers-codeact.ts"), "utf8");

describe("scraped media transcript rendering", () => {
  it("attaches scraper image and video results to assistant messages", () => {
    expect(mainSource).toContain("scrapersVisualAttachments(result)");
    expect(mainSource).toContain("visualAttachments.length > 0");
    expect(mainSource).toContain("attachments: visualAttachments.length > 0");
  });

  it("keeps scraped media URLs in the common archive and session file history", () => {
    expect(mainSource).toContain("url: cached ? uploadPreviewUrl(attachment.id, attachment.name) : attachment.url");
    expect(mainSource).toContain("url: attachment.localPath ? uploadPreviewUrl(preview.id, preview.name) : preview.url");
    expect(mainSource).toContain("filesFromArchiveSession(session)");
    expect(mainSource).toContain("(message.attachments ?? []).map(publicArchiveAttachmentPreview)");
  });

  it("injects scraped media references into the agent conversation context", () => {
    expect(mainSource).toContain("function isRemoteAttachmentUrl");
    expect(mainSource).toContain("media_url=${attachment.url}");
    expect(mainSource).toContain("media_url=${remoteUrl}");
    expect(mainSource).toContain("remote_media=true");
  });

  it("opens assistant scraped media in the split canvas instead of the transcript body", () => {
    expect(appSource).toContain("function isWebSearchVisualAttachment");
    expect(appSource).toContain('file.kind !== "image" && file.kind !== "video"');
    expect(appSource).toContain('value.includes("web_search=true")');
    expect(appSource).toContain("const [canvasWebMediaOpen, setCanvasWebMediaOpen]");
    expect(appSource).toContain("const latestWebMediaAttachments");
    expect(appSource).toContain("setCanvasWebMediaOpen(latestWebMediaKey.length > 0)");
    expect(appSource).toContain("webMediaOpen={canvasWebMediaOpen}");
    expect(appSource).toContain("webMediaFiles={latestWebMediaAttachments}");
    expect(canvasSource).toContain("function WebMediaCanvas");
    expect(canvasSource).toContain("const webMediaCanvasOpen = split && webMediaOpen");
    expect(canvasSource).toContain('className="parallelCanvasGrid parallelCanvasGrid--count2 webMediaCanvasGrid"');
    expect(canvasSource).toContain('className="parallelCanvasPane webMediaCanvasPane"');
    expect(canvasSource).toContain("<WebMediaCanvas files={webMediaFiles} />");
    expect(rendererSource).not.toContain("transcriptVisualMediaAnchor");
    expect(rendererSource).not.toContain("assistantText__body--withAnchoredMedia");
    expect(rendererSource).toContain("<TranscriptAttachmentPreview preview={preview} onMeasure={handleMeasure} />");
    expect(rendererSource).toContain('referrerPolicy="no-referrer"');
    expect(rendererSource).toContain("const followsVisualMessage");
    expect(rendererSource).toContain('(previousMessage.attachments ?? []).some(isTranscriptVisualAttachment)');
    expect(rendererSource).not.toContain("blobPathForAttachment");
    expect(rendererSource).not.toContain("TranscriptImageDreamPreview");
  });

  it("keeps scraped media in the right split canvas with a balanced bento", () => {
    expect(stylesSource).toContain(".canvasSurfaces--webMediaOpen ~ .panelsChatBottom .chatCanvas");
    expect(stylesSource).toContain(".canvasSurfaces--webMediaOpen .parallelCanvasGrid");
    expect(stylesSource).toContain(".canvasSurfaces--webMediaOpen .parallelCanvasGrid--count2 .parallelCanvasPane:first-child");
    expect(stylesSource).toContain(".webMediaCanvasPane");
    expect(stylesSource).toContain("place-items: center");
    expect(stylesSource).toContain(".webMediaBento");
    expect(stylesSource).toContain("width: min(100%, 760px)");
    expect(stylesSource).toContain("height: min(100%, calc(100vh - var(--chat-canvas-bottom) - 150px))");
    expect(stylesSource).toContain(".webMediaBento--count3");
    expect(stylesSource).toContain(".webMediaBento__item .canvasFileTile__media");
    expect(stylesSource).toContain("object-fit: contain");
    expect(stylesSource).not.toContain(".transcriptVisualMediaAnchor");
    expect(stylesSource).not.toContain(".assistantText__body--withAnchoredMedia");
    expect(stylesSource).toContain(".transcriptAttachmentStack--bento");
    expect(stylesSource).toContain("grid-template-columns: repeat(2, minmax(0, 1fr))");
    expect(stylesSource).toContain("grid-template-rows: repeat(2, minmax(0, 1fr))");
    expect(stylesSource).toContain("height: min(44vh, 500px)");
    expect(stylesSource).toContain(".transcriptAttachmentStack--count3 .transcriptAttachment:first-child");
    expect(stylesSource).toContain("filter: none");
    expect(stylesSource).toContain("object-fit: contain");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-landscape):not(:has(.transcriptFloatMedia)) .transcriptTextFrame");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-nineSixteen):not(:has(.transcriptFloatMedia)) .transcriptTextFrame");
    expect(stylesSource).toContain(".transcriptItem:has(.transcriptAttachment--frame-square):not(:has(.transcriptFloatMedia)) .transcriptTextFrame");
  });

  it("groups scraped media under Web search in the Files pane", () => {
    expect(scrapersSource).toContain('"web_search=true"');
    expect(canvasSource).toContain('type FileKindFilter = "all" | "web_search"');
    expect(canvasSource).toContain('{ id: "web_search", label: "Web search" }');
    expect(canvasSource).toContain("function isWebSearchFile");
    expect(canvasSource).toContain('kindFilter === "web_search"');
    expect(canvasSource).toContain("canvasFileTile--webSearch");
    expect(stylesSource).toContain(".canvasFileTile__sourceTag");
  });
});
