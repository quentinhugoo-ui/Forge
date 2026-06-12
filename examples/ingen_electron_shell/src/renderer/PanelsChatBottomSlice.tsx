import { Fragment, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import type { Camera, Object3D } from "three";
import type { BrainCodeActCommand, ComposerUploadPreview, TranscriptMessage } from "../shared/ipc-contract";
import {
  BRAIN_BRAIN_COMMAND,
  BRAIN_AIRBNB_COMMAND,
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_CODEACT_COMMANDS,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_GOOGLE_AGENDA_COMMAND,
  BRAIN_NAMED_COMPUTE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_CODING_COMMAND,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_NEWMODULE_COMMAND,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_RUST_PORT_ADAPTER_COMMAND,
  BRAIN_RUST_STATE_STORE_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_WORKSPACE_COMMAND,
  BRAIN_WEB_COMMAND
} from "../shared/ipc-contract";
import { ComposerSendBurst, type ComposerSendBurstHandle } from "./ComposerSendBurst";
import { panelsChatBottomStore, usePanelsChatBottomStore } from "./panels-chat-bottom-store";
import { ProviderLogo } from "./ProviderLogo";
import { MODULE_DRAG_ZONE_EVENT, type ModuleDragZoneDetail, type SidebarModuleId } from "./SidebarSlice";
import { ModuleLogo } from "./module-logos";
import { sidebarShadowStore } from "./sidebar-shadow-store";

const COMPOSER_MAX_INPUT_HEIGHT = 360;
const CHAT_KEY_COLOR_EVENT = "ingen:chat-key-color";
export const IMAGE_EDIT_STAGED_EVENT = "ingen:image-edit-staged";
const UPLOAD_PREVIEW_MAX_PDF_PAGES = 3;
type NativeDroppedFile = File & { path?: string };

function hasDraggedFiles(dataTransfer: DataTransfer | null): boolean {
  if (!dataTransfer) {
    return false;
  }
  return Array.from(dataTransfer.items ?? []).some((item) => item.kind === "file") || Array.from(dataTransfer.files ?? []).length > 0;
}

function droppedFilePaths(dataTransfer: DataTransfer | null): string[] {
  if (!dataTransfer) {
    return [];
  }
  return Array.from(dataTransfer.files ?? [])
    .map((file) => (file as NativeDroppedFile).path ?? "")
    .filter((filePath) => filePath.trim().length > 0);
}

function targetInsideComposer(target: EventTarget | null): boolean {
  return target instanceof Element && Boolean(target.closest(".composer"));
}

function emitChatKeyColor(event: KeyboardEvent<HTMLTextAreaElement>) {
  const isAltGraph = event.getModifierState("AltGraph");
  if (
    event.metaKey ||
    (event.ctrlKey && !isAltGraph) ||
    (event.altKey && !isAltGraph) ||
    event.key === "Backspace" ||
    event.key === "Delete"
  ) {
    return;
  }
  if (event.key.length !== 1 && event.key !== "Dead") {
    return;
  }
  window.dispatchEvent(new CustomEvent(CHAT_KEY_COLOR_EVENT, {
    detail: { code: event.code, key: event.key, location: event.location }
  }));
}

export function UploadPreview({ preview }: { preview?: ComposerUploadPreview }) {
  if (!preview) {
    return null;
  }
  if (preview.kind === "image") {
    return <img className="composerUploadPreview__image" src={preview.url} alt={preview.name} draggable={false} />;
  }
  if (preview.kind === "video") {
    return <VideoUploadPreview preview={preview} />;
  }
  if (preview.kind === "model3d") {
    return <ThreeUploadPreview preview={preview} />;
  }
  if (preview.kind === "pdf") {
    return <PdfUploadPreview preview={preview} />;
  }
  if (preview.kind === "spreadsheet") {
    return <SpreadsheetUploadPreview preview={preview} />;
  }
  if (preview.kind === "chart") {
    return <CandlestickUploadPreview preview={preview} />;
  }
  if (preview.kind === "text") {
    return <pre className="composerUploadPreview__text">{preview.textPreview}</pre>;
  }
  return <div className="composerUploadPreview__fallback">{preview.name}</div>;
}

function VideoUploadPreview({ preview }: { preview: ComposerUploadPreview }) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    const playPreview = () => {
      void video.play().catch(() => undefined);
    };
    playPreview();
    video.addEventListener("loadedmetadata", playPreview);
    video.addEventListener("canplay", playPreview);
    return () => {
      video.removeEventListener("loadedmetadata", playPreview);
      video.removeEventListener("canplay", playPreview);
      video.pause();
    };
  }, [preview.url]);

  return (
    <video
      ref={videoRef}
      className="composerUploadPreview__video"
      src={preview.url}
      autoPlay
      loop
      muted
      playsInline
      preload="auto"
    />
  );
}

export function UploadPreviewGrid({ previews }: { previews: ComposerUploadPreview[] }) {
  if (previews.length === 0) {
    return null;
  }
  const columns = Math.ceil(Math.sqrt(previews.length));
  const rows = Math.ceil(previews.length / columns);
  const style = {
    "--upload-grid-columns": columns,
    "--upload-grid-rows": rows
  } as CSSProperties;
  return (
    <div className="composerUploadPreviewGrid" style={style}>
      {previews.map((preview) => (
        <div className="composerUploadPreviewGrid__cell" key={preview.id} title={preview.name}>
          <UploadPreview preview={preview} />
        </div>
      ))}
    </div>
  );
}

type TranscriptMediaFrame = "portrait" | "landscape" | "nineSixteen" | "square";

type BlobPoint = {
  x: number;
  y: number;
};

type BlobBox = {
  width: number;
  height: number;
};

function transcriptMediaFrame(width: number, height: number): TranscriptMediaFrame {
  if (width <= 0 || height <= 0) {
    return "landscape";
  }
  const ratio = width / height;
  if (ratio <= 0.68) {
    return "nineSixteen";
  }
  if (ratio < 0.92) {
    return "portrait";
  }
  if (ratio > 1.12) {
    return "landscape";
  }
  return "square";
}

function seededUnit(seed: string): () => number {
  let state = 2166136261;
  for (let index = 0; index < seed.length; index += 1) {
    state ^= seed.charCodeAt(index);
    state = Math.imul(state, 16777619);
  }
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function blobBoxForFrame(frame: TranscriptMediaFrame): BlobBox {
  if (frame === "nineSixteen") {
    return { width: 72, height: 128 };
  }
  if (frame === "portrait") {
    return { width: 92, height: 100 };
  }
  if (frame === "landscape") {
    return { width: 172, height: 90 };
  }
  return { width: 112, height: 100 };
}

function dreamBlobBlur(box: BlobBox): number {
  return Math.max(4, Math.min(box.width, box.height) * 0.06);
}

function blobEdgeInset(box: BlobBox): BlobPoint {
  // The feather must finish dissolving before the frame edge: at 2.4 sigma the
  // residual alpha is under 1%, so the box never clips the fade flat.
  const inset = dreamBlobBlur(box) * 1.8;
  return { x: inset, y: inset };
}

function fitBlobPointsToBox(points: BlobPoint[], box: BlobBox): BlobPoint[] {
  const inset = blobEdgeInset(box);
  const minX = Math.min(...points.map((point) => point.x));
  const maxX = Math.max(...points.map((point) => point.x));
  const minY = Math.min(...points.map((point) => point.y));
  const maxY = Math.max(...points.map((point) => point.y));
  const spanX = Math.max(1, maxX - minX);
  const spanY = Math.max(1, maxY - minY);
  const scaleX = (box.width - inset.x * 2) / spanX;
  const scaleY = (box.height - inset.y * 2) / spanY;

  return points.map((point) => ({
    x: inset.x + (point.x - minX) * scaleX,
    y: inset.y + (point.y - minY) * scaleY
  }));
}

function blobPathForAttachment(seed: string, frame: TranscriptMediaFrame, box: BlobBox): string {
  const random = seededUnit(`${seed}:${frame}`);
  const count = 22;
  const phase = random() * Math.PI * 2;
  const radii = {
    x: box.width * (frame === "nineSixteen" ? 0.43 : frame === "portrait" ? 0.44 : 0.47),
    y: box.height * (frame === "landscape" ? 0.44 : frame === "nineSixteen" ? 0.48 : 0.47)
  };
  const points: BlobPoint[] = [];
  let wobble = random() * Math.PI * 2;

  for (let index = 0; index < count; index += 1) {
    const angle = phase + (index / count) * Math.PI * 2;
    wobble += 0.42 + random() * 0.68;
    const longWave = Math.sin(angle * 2.2 + phase) * 0.08;
    const shortWave = Math.sin(angle * 5.1 + wobble) * 0.055;
    const jitter = (random() - 0.5) * 0.11;
    const radius = 0.96 + longWave + shortWave + jitter;
    points.push({
      x: box.width / 2 + Math.cos(angle) * radii.x * radius,
      y: box.height / 2 + Math.sin(angle) * radii.y * radius
    });
  }

  const fittedPoints = fitBlobPointsToBox(points, box);
  const commands: string[] = [];
  for (let index = 0; index < fittedPoints.length; index += 1) {
    const previous = fittedPoints[(index - 1 + fittedPoints.length) % fittedPoints.length];
    const current = fittedPoints[index];
    const next = fittedPoints[(index + 1) % fittedPoints.length];
    const afterNext = fittedPoints[(index + 2) % fittedPoints.length];
    const controlA = {
      x: current.x + (next.x - previous.x) / 6,
      y: current.y + (next.y - previous.y) / 6
    };
    const controlB = {
      x: next.x - (afterNext.x - current.x) / 6,
      y: next.y - (afterNext.y - current.y) / 6
    };

    if (index === 0) {
      commands.push(`M ${current.x.toFixed(2)} ${current.y.toFixed(2)}`);
    }
    commands.push(
      `C ${controlA.x.toFixed(2)} ${controlA.y.toFixed(2)} ${controlB.x.toFixed(2)} ${controlB.y.toFixed(2)} ${next.x.toFixed(2)} ${next.y.toFixed(2)}`
    );
  }

  return `${commands.join(" ")} Z`;
}

function TranscriptImageDreamPreview({
  preview,
  maskId,
  blurId,
  blobPath,
  box,
  onMeasure
}: {
  preview: ComposerUploadPreview;
  maskId: string;
  blurId: string;
  blobPath: string;
  box: BlobBox;
  onMeasure: (width: number, height: number) => void;
}) {
  const blur = dreamBlobBlur(box);

  return (
    <>
      <img
        className="transcriptAttachment__measure"
        src={preview.url}
        alt=""
        aria-hidden="true"
        onLoad={(event) => onMeasure(event.currentTarget.naturalWidth, event.currentTarget.naturalHeight)}
      />
      <svg
        className="transcriptAttachment__dreamImage"
        viewBox={`0 0 ${box.width} ${box.height}`}
        preserveAspectRatio="none"
        aria-label={preview.name}
        role="img"
      >
        <defs>
          <filter id={blurId} x="-35%" y="-35%" width="170%" height="170%" colorInterpolationFilters="sRGB">
            <feGaussianBlur stdDeviation={blur.toFixed(2)} />
          </filter>
          <mask id={maskId} maskUnits="userSpaceOnUse" x="0" y="0" width={box.width} height={box.height} style={{ maskType: "alpha" }}>
            <path d={blobPath} fill="white" filter={`url(#${blurId})`} />
          </mask>
        </defs>
        <path className="transcriptAttachment__dreamAura" d={blobPath} filter={`url(#${blurId})`} />
        <image
          href={preview.url}
          x="0"
          y="0"
          width={box.width}
          height={box.height}
          preserveAspectRatio="xMidYMid slice"
          mask={`url(#${maskId})`}
        />
      </svg>
    </>
  );
}

function TranscriptAttachmentPreview({
  preview,
  onMeasure
}: {
  preview: ComposerUploadPreview;
  onMeasure?: (width: number, height: number) => void;
}) {
  if (preview.kind === "image") {
    return (
      <img
        className="transcriptAttachment__media"
        src={preview.url}
        alt={preview.name}
        draggable={false}
        onLoad={(event) => onMeasure?.(event.currentTarget.naturalWidth, event.currentTarget.naturalHeight)}
      />
    );
  }
  if (preview.kind === "video") {
    return <TranscriptVideoAttachmentPreview preview={preview} onMeasure={onMeasure} />;
  }
  if (preview.kind === "model3d") {
    return (
      <div className="transcriptAttachment__model">
        <ThreeUploadPreview preview={preview} />
      </div>
    );
  }
  return (
    <div className="transcriptAttachment__document">
      <UploadPreview preview={preview} />
    </div>
  );
}

function TranscriptVideoAttachmentPreview({
  preview,
  onMeasure
}: {
  preview: ComposerUploadPreview;
  onMeasure?: (width: number, height: number) => void;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return;
    }
    const playPreview = () => {
      void video.play().catch(() => undefined);
    };
    playPreview();
    video.addEventListener("loadedmetadata", playPreview);
    video.addEventListener("canplay", playPreview);
    return () => {
      video.removeEventListener("loadedmetadata", playPreview);
      video.removeEventListener("canplay", playPreview);
      video.pause();
    };
  }, [preview.url]);

  return (
    <video
      ref={videoRef}
      className="transcriptAttachment__media"
      src={preview.url}
      autoPlay
      loop
      muted
      playsInline
      preload="auto"
      onLoadedMetadata={(event) => onMeasure?.(event.currentTarget.videoWidth, event.currentTarget.videoHeight)}
    />
  );
}

function TranscriptAttachmentFigure({
  preview,
  fused = false,
  index = 0,
  onEditImage
}: {
  preview: ComposerUploadPreview;
  fused?: boolean;
  index?: number;
  onEditImage?: (preview: ComposerUploadPreview) => void;
}) {
  const [frame, setFrame] = useState<TranscriptMediaFrame>(preview.kind === "model3d" ? "square" : "landscape");
  const reactId = useId();
  const maskId = `transcript-dream-mask-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const blurId = `transcript-dream-blur-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const box = useMemo(() => blobBoxForFrame(frame), [frame]);
  const blobPath = useMemo(() => blobPathForAttachment(`${preview.id}:${preview.name}`, frame, box), [box, frame, preview.id, preview.name]);
  const className = `transcriptAttachment transcriptAttachment--${preview.kind} transcriptAttachment--frame-${frame}${fused ? " transcriptAttachment--fused" : ""}`;
  const style = fused ? ({ "--attachment-index": index } as CSSProperties) : undefined;

  if (preview.kind === "image") {
    return (
      <figure className={className} style={style} title={preview.name}>
        <TranscriptImageDreamPreview
          preview={preview}
          maskId={maskId}
          blurId={blurId}
          blobPath={blobPath}
          box={box}
          onMeasure={(width, height) => setFrame(transcriptMediaFrame(width, height))}
        />
        {onEditImage ? (
          <button
            type="button"
            className="imageEditButton imageEditButton--transcript"
            aria-label={`Modifier ${preview.name}`}
            title="Modifier l'image"
            onClick={(event) => {
              event.stopPropagation();
              onEditImage(preview);
            }}
          >
            <EditImageGlyph />
          </button>
        ) : null}
      </figure>
    );
  }

  if (preview.kind === "model3d") {
    return (
      <figure className={className} style={style} title={preview.name}>
        <div className="transcriptAttachment__model transcriptAttachment__model--direct">
          <ThreeUploadPreview preview={preview} rendererMode="webgpu" />
        </div>
      </figure>
    );
  }

  return (
    <figure className={className} style={style} title={preview.name}>
      <svg className="transcriptAttachment__dreamMask" viewBox={`0 0 ${box.width} ${box.height}`} preserveAspectRatio="none" aria-hidden="true">
        <defs>
          <filter id={blurId} x="-35%" y="-35%" width="170%" height="170%" colorInterpolationFilters="sRGB">
            <feGaussianBlur stdDeviation={dreamBlobBlur(box).toFixed(2)} />
          </filter>
          <mask
            id={maskId}
            maskUnits="userSpaceOnUse"
            x="0"
            y="0"
            width={box.width}
            height={box.height}
            style={{ maskType: "alpha" }}
          >
            <path d={blobPath} fill="white" filter={`url(#${blurId})`} />
          </mask>
        </defs>
        <foreignObject x="0" y="0" width={box.width} height={box.height} mask={`url(#${maskId})`}>
          <div className="transcriptAttachment__dreamContent">
            <TranscriptAttachmentPreview preview={preview} onMeasure={(width, height) => setFrame(transcriptMediaFrame(width, height))} />
          </div>
        </foreignObject>
      </svg>
    </figure>
  );
}

function TranscriptAttachmentStack({
  previews,
  onEditImage
}: {
  previews: ComposerUploadPreview[];
  onEditImage?: (preview: ComposerUploadPreview) => void;
}) {
  if (previews.length === 0) {
    return null;
  }
  const fused = previews.length > 1;
  const fusedCountClass = fused ? ` transcriptAttachmentStack--count${Math.min(previews.length, 4)}` : "";
  return (
    <div
      className={`transcriptAttachmentStack${fused ? " transcriptAttachmentStack--fused" : ""}${fusedCountClass}`}
      style={fused ? ({ "--attachment-count": previews.length } as CSSProperties) : undefined}
    >
      {previews.map((preview, index) => (
        <TranscriptAttachmentFigure key={preview.id} preview={preview} fused={fused} index={index} onEditImage={onEditImage} />
      ))}
    </div>
  );
}

function isTranscriptVisualAttachment(preview: ComposerUploadPreview): boolean {
  return preview.kind === "image" || preview.kind === "video" || preview.kind === "model3d";
}

export function TranscriptAttachmentEventIcon({ kind }: { kind: ComposerUploadPreview["kind"] }) {
  if (kind === "video") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="4" y="6" width="11" height="12" rx="2" />
        <path d="m15 10 5-3v10l-5-3z" />
      </svg>
    );
  }
  if (kind === "model3d") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9L12 3z" />
        <path d="M12 12 4 7.5" />
        <path d="m12 12 8-4.5" />
        <path d="M12 12v9" />
      </svg>
    );
  }
  if (kind === "pdf" || kind === "text") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3.8h6.8l3.2 3.2v13.2H7V3.8Z" />
        <path d="M13.8 3.8V7H17M9.5 10h5M9.5 12.7h5M9.5 15.4h3.8" />
      </svg>
    );
  }
  if (kind === "spreadsheet") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M5 5h14v14H5V5Z" />
        <path d="M5 10h14M5 14h14M10 5v14M15 5v14" />
      </svg>
    );
  }
  if (kind === "chart") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M5 18.5h14" />
        <path d="M7 15.5v3M12 10.5v8M17 6.5v12" />
      </svg>
    );
  }
  if (kind === "file") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3.8h6.5l3.5 3.5v12.9H7V3.8Z" />
        <path d="M13.5 3.8v3.5H17" />
      </svg>
    );
  }
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <path d="m7 16 3.3-3.3a1.6 1.6 0 0 1 2.3 0L16 16" />
      <path d="m14 14 1.2-1.2a1.6 1.6 0 0 1 2.3 0L20 15.3" />
      <circle cx="9" cy="9" r="1.2" />
    </svg>
  );
}

function visualAttachmentEventLabel(kind: ComposerUploadPreview["kind"]): string {
  if (kind === "video") return "video added";
  if (kind === "model3d") return "3D object added";
  return "image added";
}

function TranscriptVisualAttachmentEvents({ previews }: { previews: ComposerUploadPreview[] }) {
  if (previews.length === 0) {
    return null;
  }
  return (
    <div className="transcriptAttachmentEvents" aria-label="Attached visual files">
      {previews.map((preview) => (
        <div className="transcriptAttachmentEvent" key={`${preview.id}-event`}>
          <TranscriptAttachmentEventIcon kind={preview.kind} />
          <span>{visualAttachmentEventLabel(preview.kind)}</span>
          <code>{preview.name}</code>
        </div>
      ))}
    </div>
  );
}

function PdfUploadPreview({ preview }: { preview: ComposerUploadPreview }) {
  const frameRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame) {
      return;
    }
    let cancelled = false;
    let documentTask: { promise: Promise<{ numPages: number; getPage: (pageNumber: number) => Promise<any> }>; destroy?: () => void } | null = null;
    frame.replaceChildren();
    void (async () => {
      try {
        const [pdfjsLib, workerModule] = await Promise.all([
          import("pdfjs-dist"),
          import("pdfjs-dist/build/pdf.worker.mjs?url")
        ]);
        if (cancelled) {
          return;
        }
        pdfjsLib.GlobalWorkerOptions.workerSrc = workerModule.default;
        documentTask = pdfjsLib.getDocument({ url: preview.url });
        const pdf = await documentTask.promise;
        const pageCount = Math.min(pdf.numPages, UPLOAD_PREVIEW_MAX_PDF_PAGES);
        for (let pageIndex = 1; pageIndex <= pageCount && !cancelled; pageIndex += 1) {
          const page = await pdf.getPage(pageIndex);
          const baseViewport = page.getViewport({ scale: 1 });
          const scale = Math.max(0.24, frame.clientWidth / Math.max(baseViewport.width, 1));
          const viewport = page.getViewport({ scale });
          const canvas = document.createElement("canvas");
          canvas.className = "composerUploadPreview__pdfPage";
          canvas.width = Math.floor(viewport.width);
          canvas.height = Math.floor(viewport.height);
          frame.appendChild(canvas);
          const context = canvas.getContext("2d");
          if (context) {
            await page.render({ canvas, canvasContext: context, viewport }).promise;
          }
        }
      } catch (error) {
        if (!cancelled) {
          frame.textContent = "PDF preview unavailable";
        }
      }
    })();
    return () => {
      cancelled = true;
      documentTask?.destroy?.();
      frame.replaceChildren();
    };
  }, [preview.url]);

  return <div ref={frameRef} className="composerUploadPreview__pdf" aria-label={preview.name} />;
}

function CandlestickUploadPreview({ preview }: { preview: ComposerUploadPreview }) {
  const rows = preview.tablePreview.slice(1).map((row) => ({
    open: Number(row[1]),
    high: Number(row[2]),
    low: Number(row[3]),
    close: Number(row[4])
  })).filter((row) => [row.open, row.high, row.low, row.close].every(Number.isFinite));
  if (rows.length === 0) {
    return <SpreadsheetUploadPreview preview={preview} />;
  }
  const values = rows.flatMap((row) => [row.high, row.low]);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = Math.max(max - min, 0.000001);
  const xStep = 100 / rows.length;
  const y = (value: number) => 92 - ((value - min) / span) * 84;

  return (
    <svg className="composerUploadPreview__candles" viewBox="0 0 100 100" aria-label={preview.name}>
      {rows.map((row, index) => {
        const x = xStep * index + xStep / 2;
        const openY = y(row.open);
        const closeY = y(row.close);
        const highY = y(row.high);
        const lowY = y(row.low);
        const up = row.close >= row.open;
        const bodyTop = Math.min(openY, closeY);
        const bodyHeight = Math.max(1.2, Math.abs(openY - closeY));
        return (
          <g className={up ? "composerUploadPreview__candle composerUploadPreview__candle--up" : "composerUploadPreview__candle composerUploadPreview__candle--down"} key={`${preview.id}-candle-${index}`}>
            <line x1={x} x2={x} y1={highY} y2={lowY} />
            <rect x={x - Math.max(1.2, xStep * 0.24)} y={bodyTop} width={Math.max(2.2, xStep * 0.48)} height={bodyHeight} />
          </g>
        );
      })}
    </svg>
  );
}

function SpreadsheetUploadPreview({ preview }: { preview: ComposerUploadPreview }) {
  return (
    <table className="composerUploadPreview__sheet" aria-label={preview.name}>
      <tbody>
        {preview.tablePreview.map((row, rowIndex) => (
          <tr key={`${preview.id}-row-${rowIndex}`}>
            {row.map((cell, cellIndex) => (
              <td key={`${preview.id}-cell-${rowIndex}-${cellIndex}`}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

type ThreePreviewRendererMode = "webgl" | "webgpu";

type ThreePreviewRenderer = {
  domElement: HTMLCanvasElement;
  outputColorSpace?: string;
  setPixelRatio: (value: number) => void;
  setClearColor: (color: number, alpha?: number) => void;
  setSize: (width: number, height: number, updateStyle?: boolean) => void;
  render: (scene: Object3D, camera: Camera) => void;
  dispose: () => void;
  init?: () => Promise<unknown>;
};

function ThreeUploadPreview({
  preview,
  rendererMode = "webgl"
}: {
  preview: ComposerUploadPreview;
  rendererMode?: ThreePreviewRendererMode;
}) {
  const hostRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }

    let frameId = 0;
    let disposed = false;
    let cleanupRenderer: (() => void) | null = null;

    void (async () => {
      const showModelMessage = (text: string) => {
        const message = document.createElement("div");
        message.className = "composerUploadPreview__modelError";
        message.textContent = text;
        host.replaceChildren(message);
      };
      showModelMessage("Loading 3D");

      const [THREE, { GLTFLoader }, { OBJLoader }, { PLYLoader }, { STLLoader }] = await Promise.all([
        import("three"),
        import("three/examples/jsm/loaders/GLTFLoader.js"),
        import("three/examples/jsm/loaders/OBJLoader.js"),
        import("three/examples/jsm/loaders/PLYLoader.js"),
        import("three/examples/jsm/loaders/STLLoader.js")
      ]);
      if (disposed) {
        return;
      }

      const createWebGlRenderer = (): ThreePreviewRenderer => new THREE.WebGLRenderer({ alpha: true, antialias: true });
      const createRenderer = async (): Promise<ThreePreviewRenderer> => {
        if (rendererMode !== "webgpu") {
          return createWebGlRenderer();
        }
        try {
          const { WebGPURenderer } = await import("three/webgpu");
          const renderer = new WebGPURenderer({ alpha: true, antialias: true }) as unknown as ThreePreviewRenderer;
          await renderer.init?.();
          return renderer;
        } catch (error) {
          console.warn("3D WebGPU renderer unavailable; using WebGL fallback.", error);
          return createWebGlRenderer();
        }
      };
      const renderer = await createRenderer();
      if (disposed) {
        renderer.dispose();
        return;
      }
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x000000, 0);
      renderer.outputColorSpace = THREE.SRGBColorSpace;
      host.replaceChildren(renderer.domElement);

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 1000);
      camera.position.set(0, 0, 4.2);
      scene.add(new THREE.HemisphereLight(0xffffff, 0x707070, 1.8));
      const keyLight = new THREE.DirectionalLight(0xffffff, 2.2);
      keyLight.position.set(3, 4, 5);
      scene.add(keyLight);
      const fillLight = new THREE.DirectionalLight(0x9fc7ff, 0.9);
      fillLight.position.set(-3, 1.8, 2.4);
      scene.add(fillLight);

      const modelRoot = new THREE.Group();
      scene.add(modelRoot);

      const previewMaterial = () =>
        new THREE.MeshStandardMaterial({
          color: 0xd7d7d7,
          metalness: 0.16,
          roughness: 0.44
        });
      const resize = () => {
        const width = Math.max(1, Math.floor(host.clientWidth));
        const height = Math.max(1, Math.floor(host.clientHeight));
        renderer.setSize(width, height, false);
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
      };
      const renderScene = (time = 0) => {
        if (disposed) {
          return;
        }
        resize();
        modelRoot.rotation.y = 0.72 + time * 0.00034;
        modelRoot.rotation.x = -0.18;
        camera.lookAt(0, 0, 0);
        renderer.render(scene, camera);
        frameId = window.requestAnimationFrame(renderScene);
      };
      const resizeObserver = new ResizeObserver(resize);
      resizeObserver.observe(host);
      const frameModel = (object: Object3D) => {
        const box = new THREE.Box3().setFromObject(object);
        const center = box.getCenter(new THREE.Vector3());
        const sphere = box.getBoundingSphere(new THREE.Sphere());
        if (!Number.isFinite(sphere.radius) || sphere.radius <= 0 || box.isEmpty()) {
          object.position.set(0, 0, 0);
          object.scale.setScalar(rendererMode === "webgpu" ? 1 : 1.35);
          return;
        }
        object.position.sub(center);
        const targetSize = rendererMode === "webgpu" ? 1.25 : 1.85;
        object.scale.multiplyScalar(targetSize / Math.max(sphere.radius, 0.001));
      };
      const disposeObject = (object: Object3D) => {
        object.traverse((child) => {
          const disposable = child as Object3D & {
            geometry?: { dispose?: () => void };
            material?: { dispose?: () => void } | Array<{ dispose?: () => void }>;
          };
          disposable.geometry?.dispose?.();
          const materials = Array.isArray(disposable.material) ? disposable.material : disposable.material ? [disposable.material] : [];
          for (const material of materials) {
            material.dispose?.();
          }
        });
      };
      cleanupRenderer = () => {
        window.cancelAnimationFrame(frameId);
        resizeObserver.disconnect();
        disposeObject(scene);
        renderer.dispose();
      };
      const addObject = (object: Object3D) => {
        if (disposed) {
          return;
        }
        for (const child of [...modelRoot.children]) {
          disposeObject(child);
        }
        modelRoot.clear();
        frameModel(object);
        modelRoot.add(object);
      };

      const extension = preview.name.split(".").pop()?.toLowerCase() ?? "";

      try {
        const bytes = await fetch(preview.url).then((response) => {
          if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
          }
          return response.arrayBuffer();
        });

        if (disposed) {
          return;
        }

        if (extension === "glb" || extension === "gltf") {
          const loader = new GLTFLoader();
          const gltf = await new Promise<{ scene: Object3D }>((resolve, reject) => {
            loader.parse(bytes, preview.url, resolve, reject);
          });
          addObject(gltf.scene);
        } else if (extension === "obj") {
          const text = new TextDecoder("utf-8").decode(bytes);
          addObject(new OBJLoader().parse(text));
        } else if (extension === "stl") {
          const geometry = new STLLoader().parse(bytes);
          addObject(new THREE.Mesh(geometry, previewMaterial()));
        } else if (extension === "ply") {
          const geometry = new PLYLoader().parse(bytes);
          addObject(new THREE.Mesh(geometry, previewMaterial()));
        } else if (extension === "fbx") {
          const { FBXLoader } = await import("three/examples/jsm/loaders/FBXLoader.js");
          addObject(new FBXLoader().parse(bytes, ""));
        } else if (extension === "dae") {
          const { ColladaLoader } = await import("three/examples/jsm/loaders/ColladaLoader.js");
          const collada = new ColladaLoader().parse(new TextDecoder("utf-8").decode(bytes), "");
          if (!collada?.scene) {
            throw new Error("Collada scene missing");
          }
          addObject(collada.scene);
        } else if (extension === "3ds") {
          const { TDSLoader } = await import("three/examples/jsm/loaders/TDSLoader.js");
          addObject(new TDSLoader().parse(bytes, ""));
        } else if (extension === "3mf") {
          const { ThreeMFLoader } = await import("three/examples/jsm/loaders/3MFLoader.js");
          addObject(new ThreeMFLoader().parse(bytes));
        } else if (extension === "usd" || extension === "usdz") {
          const { USDLoader } = await import("three/examples/jsm/loaders/USDLoader.js");
          addObject(new USDLoader().parse(bytes));
        } else {
          throw new Error(`Unsupported model format: ${extension || "unknown"}`);
        }
      } catch (error) {
        renderer.domElement.remove();
        cleanupRenderer?.();
        cleanupRenderer = null;
        showModelMessage("3D preview unavailable");
        console.warn("3D preview unavailable", error);
        return;
      }

      frameId = window.requestAnimationFrame(renderScene);
    })();

    return () => {
      disposed = true;
      cleanupRenderer?.();
      host.replaceChildren();
    };
  }, [preview.name, preview.url, rendererMode]);

  return <div ref={hostRef} className="composerUploadPreview__model" aria-label={preview.name} />;
}

interface PinnedChapter {
  id: string;
  text: string;
}

const LEGACY_PINS_STORAGE_KEY = "ingen.chat.pins.v1";
const SESSION_PINS_STORAGE_KEY_PREFIX = "ingen.chat.session-pins.v1";

function pinsStorageKey(activeSessionId: string): string {
  return `${SESSION_PINS_STORAGE_KEY_PREFIX}:${encodeURIComponent(activeSessionId || "draft")}`;
}

function loadPins(storageKey: string): PinnedChapter[] {
  try {
    const storage = globalThis.localStorage;
    storage?.removeItem(LEGACY_PINS_STORAGE_KEY);
    const raw = storage?.getItem(storageKey);
    const parsed = raw ? JSON.parse(raw) : [];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(
      (entry): entry is PinnedChapter => typeof entry?.id === "string" && typeof entry?.text === "string"
    );
  } catch {
    return [];
  }
}

function savePins(storageKey: string, pins: PinnedChapter[]) {
  try {
    const storage = globalThis.localStorage;
    storage?.removeItem(LEGACY_PINS_STORAGE_KEY);
    if (pins.length === 0) {
      storage?.removeItem(storageKey);
      return;
    }
    storage?.setItem(storageKey, JSON.stringify(pins));
  } catch {
    // storage unavailable: pins stay in memory for this renderer session
  }
}

function CopyGlyph() {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
  );
}

function PinGlyph({ filled }: { filled: boolean }) {
  return (
    <svg width={14} height={14} viewBox="0 0 24 24" fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M9 4h6l-1 7 3.5 2.5V16H6.5v-2.5L10 11 9 4z" />
      <line x1="12" y1="16" x2="12" y2="21" />
    </svg>
  );
}

export function EditImageGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M14.5 4.5 19.5 9.5" />
      <path d="M4 20l4.5-1 10-10a2.8 2.8 0 0 0-4-4l-10 10L4 20z" />
      <path d="M4.5 14.5 9.5 19.5" />
      <path d="M3.8 6.2h5" />
      <path d="M6.3 3.7v5" />
    </svg>
  );
}

function prefersReducedMotion() {
  return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function followTranscriptLatest(container: HTMLElement | null, behavior: ScrollBehavior = "auto") {
  if (!container) {
    return;
  }
  requestAnimationFrame(() => {
    container.scrollTo({ top: container.scrollHeight - container.clientHeight, behavior });
  });
}

function latestTranscriptContainerFor(element: Element | null): HTMLElement | null {
  const item = element?.closest(".transcriptItem");
  const container = element?.closest<HTMLElement>(".chatCanvas__messages");
  if (!item || !container || container.lastElementChild !== item) {
    return null;
  }
  return container;
}

type AssistantMarkdownBlock =
  | { kind: "heading"; level: number; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "paragraph"; text: string }
  | { kind: "event"; event: TranscriptCodeActEvent };

type TranscriptCodeActCommand = BrainCodeActCommand | `/compute_${string}_`;

interface TranscriptCodeActEvent {
  command: TranscriptCodeActCommand;
  text: string;
}

const BRAIN_CODEACT_COMMAND_SET = new Set<string>(BRAIN_CODEACT_COMMANDS);
const BRAIN_CODEACT_COMMANDS_BY_LENGTH = [...BRAIN_CODEACT_COMMANDS].sort((left, right) => right.length - left.length);
const BRAIN_CODEACT_DESCRIPTION_BY_COMMAND = new Map<string, string>(
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS.map((entry) => [entry.command, entry.description])
);

const TRANSCRIPT_CODEACT_EVENT_TEXT = new Map<string, string>([
  [BRAIN_SEARCHARCHIVE_COMMAND, "archive memory search returned bounded context"],
  [BRAIN_GOOGLEWEB_COMMAND, "native Google WebExplorer search event created"],
  [BRAIN_GMAIL_COMMAND, "Gmail event prepared"],
  [BRAIN_GMAIL_COM_COMMAND, "Gmail surface opened"],
  [BRAIN_AIRBNB_COMMAND, "Airbnb surface opened"],
  [BRAIN_NEWIMAGE_COMMAND, "image generation prepared"],
  [BRAIN_EDITIMAGE_COMMAND, "image edit prepared"],
  [BRAIN_SCIENCE_COMMAND, "Science / Engineering / 3D Brain loaded"],
  [BRAIN_CODING_COMMAND, "Coding Brain loaded"],
  [BRAIN_WORKSPACE_COMMAND, "workspace folder required"],
  [BRAIN_NEWCOMPUTE_COMMAND, "opens the Monster template selector"],
  [BRAIN_SELECTCOMPUTE_COMMAND, "saved compute reused from the library"],
  [BRAIN_NAMED_COMPUTE_COMMAND, "named compute specialization invoked"],
  [BRAIN_NEWOBJECT_COMMAND, "Banger SDF object created"],
  [BRAIN_WEB_COMMAND, "native web research event created"],
  [BRAIN_FRONTDESIGN_COMMAND, "front design contract projected"],
  [BRAIN_GOOGLE_AGENDA_COMMAND, "Google Calendar event created"],
  [BRAIN_BRAIN_COMMAND, "Brain memory indexed"],
  [BRAIN_NEWMODULE_COMMAND, "module contract prepared"],
  [BRAIN_RUST_PORT_ADAPTER_COMMAND, "Rust adapter template prepared"],
  [BRAIN_RUST_STATE_STORE_COMMAND, "Rust state store contract prepared"]
]);

function dynamicComputeLabel(command: string): string {
  return command
    .replace(/^\/compute_/, "")
    .replace(/_$/, "")
    .replace(/_/g, " ")
    .trim();
}

function isDynamicComputeCommand(value: string): value is `/compute_${string}_` {
  return /^\/compute_[a-zA-Z0-9][a-zA-Z0-9_]*_$/.test(value) && value !== BRAIN_NAMED_COMPUTE_COMMAND;
}

function codeActEventText(command: TranscriptCodeActCommand): string {
  if (isDynamicComputeCommand(command)) {
    const label = dynamicComputeLabel(command);
    return `${label || "named"} compute executed`;
  }
  return TRANSCRIPT_CODEACT_EVENT_TEXT.get(command) || BRAIN_CODEACT_DESCRIPTION_BY_COMMAND.get(command) || "CodeAct command executed";
}

function readCodeActCommand(value: string): TranscriptCodeActCommand | undefined {
  const trimmed = value.trim().replace(/^["'`]+|["'`,;]+$/g, "");
  if (BRAIN_CODEACT_COMMAND_SET.has(trimmed)) {
    return trimmed as BrainCodeActCommand;
  }
  if (isDynamicComputeCommand(trimmed)) {
    return trimmed;
  }
  return undefined;
}

function codeActEventFromLine(line: string): TranscriptCodeActEvent | undefined {
  const trimmed = line.trim();
  const commandAssignment = /(?:^|\s)command\s*=\s*("([^"]+)"|'([^']+)'|([^\s]+))/.exec(trimmed);
  const assignedCommand = commandAssignment ? readCodeActCommand(commandAssignment[2] ?? commandAssignment[3] ?? commandAssignment[4] ?? "") : undefined;
  if (assignedCommand) {
    return { command: assignedCommand, text: codeActEventText(assignedCommand) };
  }
  const firstToken = trimmed.split(/\s+/, 1)[0] ?? "";
  const directCommand = readCodeActCommand(firstToken);
  if (directCommand) {
    return { command: directCommand, text: codeActEventText(directCommand) };
  }
  const containedCommand = BRAIN_CODEACT_COMMANDS_BY_LENGTH.find((command) => trimmed.includes(command));
  if (containedCommand) {
    return { command: containedCommand, text: codeActEventText(containedCommand) };
  }
  const dynamicCompute = /\/compute_[a-zA-Z0-9][a-zA-Z0-9_]*_/.exec(trimmed)?.[0];
  if (dynamicCompute && isDynamicComputeCommand(dynamicCompute)) {
    return { command: dynamicCompute, text: codeActEventText(dynamicCompute) };
  }
  return undefined;
}

function isCodeActResultHeader(line: string): boolean {
  return /^[A-Z][A-Z0-9_]*_RESULT\b/.test(line.trim());
}

function normalizeAssistantMarkdownText(text: string): string {
  return text
    .replace(/\r\n/g, "\n")
    .replace(/([.!?])\s+(-\s+)/g, "$1\n$2")
    .replace(/([.!?])\s+(\d+[.)]\s+)/g, "$1\n$2");
}

function assistantMarkdownBlocks(text: string): AssistantMarkdownBlock[] {
  const lines = normalizeAssistantMarkdownText(text).split("\n");
  const blocks: AssistantMarkdownBlock[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  let skippingCodeActMetadata = false;
  let sawCodeActMetadata = false;

  const flushParagraph = () => {
    const body = paragraph.join(" ").replace(/\s+/g, " ").trim();
    if (body) {
      blocks.push({ kind: "paragraph", text: body });
    }
    paragraph = [];
  };
  const flushList = () => {
    if (list && list.items.length > 0) {
      blocks.push({ kind: "list", ordered: list.ordered, items: list.items });
    }
    list = null;
  };

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      flushParagraph();
      flushList();
      skippingCodeActMetadata = false;
      continue;
    }
    if (isCodeActResultHeader(line)) {
      flushParagraph();
      flushList();
      skippingCodeActMetadata = true;
      sawCodeActMetadata = true;
      continue;
    }
    if (skippingCodeActMetadata) {
      continue;
    }
    const event = codeActEventFromLine(line);
    if (event) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "event", event });
      skippingCodeActMetadata = true;
      sawCodeActMetadata = true;
      continue;
    }
    const heading = /^(#{1,4})\s+(.+)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2].trim() });
      continue;
    }
    const bullet = /^[-*]\s+(.+)$/.exec(line);
    const ordered = /^(\d+)[.)]\s+(.+)$/.exec(line);
    if (bullet || ordered) {
      flushParagraph();
      const orderedItem = Boolean(ordered);
      if (!list || list.ordered !== orderedItem) {
        flushList();
        list = { ordered: orderedItem, items: [] };
      }
      list.items.push((ordered?.[2] ?? bullet?.[1] ?? "").trim());
      continue;
    }
    flushList();
    paragraph.push(line);
  }

  flushParagraph();
  flushList();
  const fallbackBlock: AssistantMarkdownBlock = { kind: "paragraph", text: text.trim() };
  return blocks.length > 0 ? blocks : sawCodeActMetadata ? [] : [fallbackBlock];
}

function assistantVisibleAnimationSource(text: string): string {
  const normalized = normalizeAssistantMarkdownText(text);
  const lines = normalized.split("\n");
  let offset = 0;
  let lastVisibleEnd = 0;
  let skippingCodeActMetadata = false;

  for (const rawLine of lines) {
    const lineStart = offset;
    const lineEnd = lineStart + rawLine.length;
    const line = rawLine.trim();
    offset = lineEnd + 1;

    if (!line) {
      skippingCodeActMetadata = false;
      continue;
    }
    if (isCodeActResultHeader(line)) {
      skippingCodeActMetadata = true;
      continue;
    }
    if (skippingCodeActMetadata) {
      continue;
    }
    if (codeActEventFromLine(line)) {
      lastVisibleEnd = lineEnd;
      skippingCodeActMetadata = true;
      continue;
    }
    lastVisibleEnd = lineEnd;
  }

  return normalized.slice(0, lastVisibleEnd || normalized.trimEnd().length);
}

function assistantInlineNodes(text: string, keyPrefix: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /(\*\*[^*]+?\*\*|`[^`]+?`)/g;
  let cursor = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(text.slice(cursor, match.index));
    }
    const token = match[0];
    if (token.startsWith("**")) {
      nodes.push(<strong key={`${keyPrefix}-strong-${match.index}`}>{token.slice(2, -2)}</strong>);
    } else {
      nodes.push(<code key={`${keyPrefix}-code-${match.index}`}>{token.slice(1, -1)}</code>);
    }
    cursor = match.index + token.length;
  }
  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }
  return nodes;
}

function GenericCodeActIcon({ done = true }: { done?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <circle cx="12" cy="12" r="9" />
      {done ? <path d="m8.2 12.2 2.5 2.5 5-5.2" /> : <path d="M9 12h6" />}
    </svg>
  );
}

function NewComputeCodeActIcon() {
  return (
    <svg className="boundedFill" viewBox="0 0 32 32" aria-hidden="true">
      <path d="M19 2c1.306 0 2.418.835 2.83 2h1.67A3.5 3.5 0 0 1 27 7.5v8.354a2.488 2.488 0 0 0-.843-.713l-.249-.13a4.575 4.575 0 0 0-.908-.356V7.5A1.5 1.5 0 0 0 23.5 6h-1.67A3.001 3.001 0 0 1 19 8h-6a3.001 3.001 0 0 1-2.83-2H8.5A1.5 1.5 0 0 0 7 7.5v19A1.5 1.5 0 0 0 8.5 28h7.61a2.506 2.506 0 0 0-.584 2H8.5A3.5 3.5 0 0 1 5 26.5v-19A3.5 3.5 0 0 1 8.5 4h1.67A3.001 3.001 0 0 1 13 2h6Zm-6 2a1 1 0 1 0 0 2h6a1 1 0 1 0 0-2h-6Zm17.703 20.707a1 1 0 1 0-1.414-1.414l-1.612 1.611l-.649-1.043a1.822 1.822 0 0 0-2.613-.516a1 1 0 0 0 1.006 1.719l.804 1.293l-1.932 1.932a1 1 0 0 0 1.415 1.414L27.31 28.1l.65 1.045a1.816 1.816 0 0 0 2.645.484a1 1 0 0 0-1.043-1.695l-.8-1.286l1.941-1.94Zm-9.865-5.92c.155-2.152 2.463-3.441 4.377-2.445l.249.13a1 1 0 1 1-.924 1.773l-.248-.129a1 1 0 0 0-1.46.815l-.076 1.07H24a1 1 0 0 1 0 2h-1.388l-.448 6.208c-.155 2.153-2.463 3.442-4.377 2.446l-.249-.13a1 1 0 1 1 .924-1.773l.248.129a1 1 0 0 0 1.46-.815L20.606 22H20a1 1 0 1 1 0-2h.75l.088-1.212Z" />
    </svg>
  );
}

function BrainCodeActIcon() {
  return (
    <svg className="brainIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path className="brainStem" d="M12 18V5" />
      <path className="brainSide" d="M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
      <path className="brainTop" d="M12 5A3 3 0 1 1 17.598 6.5" />
      <path className="brainTop" d="M12 5A3 3 0 1 0 6.402 6.5" />
      <path d="M17.997 5.125a4 4 0 0 1 2.526 5.77" />
      <path className="brainLow" d="M18 18a4 4 0 0 0 2-7.464" />
      <path d="M19.967 17.483A4 4 0 1 1 12 18a4 4 0 1 1-7.967-.517" />
      <path className="brainLow" d="M6 18a4 4 0 0 1-2-7.464" />
      <path d="M6.003 5.125a4 4 0 0 0-2.526 5.77" />
    </svg>
  );
}

function BrainSegmentCodeActIcon() {
  return (
    <svg className="brainSegmentIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path className="brainSegmentIcon__core" d="M12 18V5M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
      <path className="brainSegmentIcon__halo" d="M5.2 15.2c2.7 3.3 10.9 3.3 13.6 0" />
      <path className="brainSegmentIcon__halo" d="M5.2 8.8c2.7-3.3 10.9-3.3 13.6 0" />
      <circle className="brainSegmentIcon__dot brainSegmentIcon__dot--a" cx="5.2" cy="12" r="1.4" />
      <circle className="brainSegmentIcon__dot brainSegmentIcon__dot--b" cx="18.8" cy="12" r="1.4" />
    </svg>
  );
}

function SearchArchiveCodeActIcon() {
  return (
    <svg className="searchArchiveIcon" viewBox="0 0 24 24" aria-hidden="true">
      <path className="searchArchiveBox" d="M4.5 8.5h9.8v8.8a1.7 1.7 0 0 1-1.7 1.7H6.2a1.7 1.7 0 0 1-1.7-1.7V8.5Z" />
      <path className="searchArchiveLid" d="M3.8 6.1h11.2v2.4H3.8V6.1Z" />
      <path className="searchArchiveLine" d="M7.3 11.1h4.1" />
      <circle className="searchArchiveLens" cx="15.5" cy="14.5" r="3.2" />
      <path className="searchArchiveHandle" d="m17.9 16.9 2.5 2.5" />
    </svg>
  );
}

function NewObjectCodeActIcon() {
  return (
    <span className="cubeIconBox" aria-hidden="true">
      <span className="cubeSpinner">
        {Array.from({ length: 6 }, (_, index) => (
          <span key={index} />
        ))}
      </span>
    </span>
  );
}

function CalendarCodeActIcon() {
  return (
    <span className="calendarIconSwitch" aria-hidden="true">
      <svg className="calendarIcon calendarBusyIcon" viewBox="0 0 24 24">
        <path d="M8 2v4" />
        <path d="M16 2v4" />
        <rect height="18" rx="2" width="18" x="3" y="4" />
        <path d="M3 10h18" />
        {[8, 12, 16, 8, 12, 16].map((cx, index) => (
          <circle className="calendarDot" cx={cx} cy={index < 3 ? 14 : 18} key={`${cx}-${index}`} r="1" />
        ))}
      </svg>
      <svg className="calendarIcon calendarDoneIcon" viewBox="0 0 24 24">
        <path d="M8 2v4" />
        <path d="M16 2v4" />
        <rect height="18" rx="2" width="18" x="3" y="4" />
        <path d="M3 10h18" />
        <path className="calendarCheck" d="m9 16 2 2 4-4" />
      </svg>
    </span>
  );
}

function CodeActEventIcon({ command }: { command: TranscriptCodeActCommand }) {
  if (command === BRAIN_GMAIL_COMMAND || command === BRAIN_GMAIL_COM_COMMAND) return <ModuleLogo id="gmail" />;
  if (command === BRAIN_AIRBNB_COMMAND) return <ModuleLogo id="airbnb" />;
  if (command === BRAIN_SCIENCE_COMMAND || command === BRAIN_CODING_COMMAND) return <BrainSegmentCodeActIcon />;
  if (command === BRAIN_NEWIMAGE_COMMAND || command === BRAIN_EDITIMAGE_COMMAND) return <EditImageGlyph />;
  if (command === BRAIN_NEWCOMPUTE_COMMAND) return <NewComputeCodeActIcon />;
  if (command === BRAIN_BRAIN_COMMAND) return <BrainCodeActIcon />;
  if (command === BRAIN_SEARCHARCHIVE_COMMAND) return <SearchArchiveCodeActIcon />;
  if (command === BRAIN_NEWOBJECT_COMMAND) return <NewObjectCodeActIcon />;
  if (command === BRAIN_GOOGLE_AGENDA_COMMAND) return <CalendarCodeActIcon />;
  return <GenericCodeActIcon />;
}

function isBrainSegmentCommand(command: TranscriptCodeActCommand): boolean {
  return command === BRAIN_SCIENCE_COMMAND || command === BRAIN_CODING_COMMAND;
}

function TranscriptCodeActEventLine({ event }: { event: TranscriptCodeActEvent }) {
  return (
    <div className={isBrainSegmentCommand(event.command) ? "transcriptCodeActEvent transcriptCodeActEvent--brainSegment" : "transcriptCodeActEvent"}>
      <code className="transcriptCodeActEvent__command">{event.command}</code>
      <span className="transcriptCodeActEvent__icon" aria-hidden="true">
        <CodeActEventIcon command={event.command} />
      </span>
      <span className="transcriptCodeActEvent__text">{event.text}</span>
    </div>
  );
}

function AssistantMarkdownText({ text, messageId, writing }: { text: string; messageId: string; writing: boolean }) {
  const blocks = assistantMarkdownBlocks(text);
  return (
    <div className="assistantText__body">
      {blocks.map((block, index) => {
        if (block.kind === "event") {
          return <TranscriptCodeActEventLine event={block.event} key={`${messageId}-event-${index}-${block.event.command}`} />;
        }
        if (block.kind === "heading") {
          const Tag = block.level <= 2 ? "h3" : "h4";
          return <Tag className="assistantText__heading" key={`${messageId}-heading-${index}`}>{assistantInlineNodes(block.text, `${messageId}-heading-${index}`)}</Tag>;
        }
        if (block.kind === "list") {
          const Tag = block.ordered ? "ol" : "ul";
          return (
            <Tag className="assistantText__list" key={`${messageId}-list-${index}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${messageId}-list-${index}-${itemIndex}`}>{assistantInlineNodes(item, `${messageId}-list-${index}-${itemIndex}`)}</li>
              ))}
            </Tag>
          );
        }
        return <p className="assistantText__paragraph" key={`${messageId}-paragraph-${index}`}>{assistantInlineNodes(block.text, `${messageId}-paragraph-${index}`)}</p>;
      })}
      {writing ? <span className="assistantText__caret" /> : null}
    </div>
  );
}

function StaticAssistantText({ message }: { message: TranscriptMessage }) {
  return (
    <div className="assistantText" aria-label={message.text}>
      <AssistantMarkdownText messageId={message.id} text={message.text} writing={false} />
    </div>
  );
}

function AnimatedAssistantText({ message, onAnimationComplete }: { message: TranscriptMessage; onAnimationComplete?: (messageId: string) => void }) {
  const textRef = useRef<HTMLDivElement>(null);
  const animationSource = useMemo(() => assistantVisibleAnimationSource(message.text), [message.text]);
  const totalCharacters = animationSource.length;
  const [visibleCharacters, setVisibleCharacters] = useState(0);
  const [animationSettled, setAnimationSettled] = useState(false);
  const completionReportedRef = useRef(false);

  useEffect(() => {
    completionReportedRef.current = false;
    setAnimationSettled(false);
    setVisibleCharacters(0);
    if (totalCharacters === 0 || prefersReducedMotion()) {
      setVisibleCharacters(totalCharacters);
      setAnimationSettled(true);
      return;
    }

    let frame = 0;
    const startedAt = performance.now();
    const duration = Math.min(4200, Math.max(720, totalCharacters * 18));

    setVisibleCharacters(0);

    const tick = (now: number) => {
      const progress = Math.min(1, (now - startedAt) / duration);
      const eased = 1 - Math.pow(1 - progress, 2.4);
      setVisibleCharacters(Math.min(totalCharacters, Math.ceil(totalCharacters * eased)));
      if (progress < 1) {
        frame = requestAnimationFrame(tick);
      } else {
        setAnimationSettled(true);
      }
    };

    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [animationSource, message.id, totalCharacters]);

  useEffect(() => {
    if (completionReportedRef.current || totalCharacters === 0 || !animationSettled || visibleCharacters < totalCharacters) {
      return;
    }
    completionReportedRef.current = true;
    sidebarShadowStore.finishChatSessionPreview();
    onAnimationComplete?.(message.id);
    void panelsChatBottomStore.dispatch({ kind: "assistant_write_complete", value: message.id });
  }, [animationSettled, message.id, onAnimationComplete, totalCharacters, visibleCharacters]);

  useLayoutEffect(() => {
    followTranscriptLatest(latestTranscriptContainerFor(textRef.current));
  }, [visibleCharacters]);

  const writing = visibleCharacters < totalCharacters;
  const visibleText = animationSource.slice(0, visibleCharacters);

  return (
    <div className="assistantText" aria-label={message.text} ref={textRef}>
      <AssistantMarkdownText messageId={message.id} text={visibleText} writing={writing} />
    </div>
  );
}

function PendingAssistantText() {
  return (
    <div className="assistantText assistantText--pending" aria-label="Assistant response pending">
      <span className="assistantText__pendingCaret" aria-hidden="true" />
    </div>
  );
}

function TranscriptCanvas({
  activeSessionId,
  messages,
  className = "chatCanvas",
  onEditImage
}: {
  activeSessionId: string;
  messages: TranscriptMessage[];
  className?: string;
  onEditImage?: (preview: ComposerUploadPreview) => void;
}) {
  const storageKey = useMemo(() => pinsStorageKey(activeSessionId), [activeSessionId]);
  const [pins, setPins] = useState<PinnedChapter[]>(() => loadPins(storageKey));
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const assistantAnimationRef = useRef<{ sessionId: string; known: Set<string>; active: Set<string>; hadPending: boolean } | null>(null);
  const latestMessage = messages.at(-1);
  const messageIds = useMemo(() => new Set(messages.map((message) => message.id)), [messages]);
  const assistantMessageIds = useMemo(
    () => messages
      .filter((message) => message.role === "assistant" && message.text.trim() && !message.id.startsWith("assistant-pending-") && !message.id.startsWith("assistant-error-"))
      .map((message) => message.id),
    [messages]
  );
  const latestAssistantMessageId = assistantMessageIds.at(-1) ?? "";
  const hasPendingAssistant = messages.some((message) => message.role === "assistant" && message.id.startsWith("assistant-pending-"));
  const visiblePins = useMemo(() => pins.filter((pin) => messageIds.has(pin.id)), [messageIds, pins]);

  if (assistantAnimationRef.current === null) {
    assistantAnimationRef.current = {
      sessionId: activeSessionId,
      known: new Set(assistantMessageIds),
      active: new Set(),
      hadPending: hasPendingAssistant
    };
  } else if (assistantAnimationRef.current.sessionId !== activeSessionId) {
    const previous = assistantAnimationRef.current;
    const keepDraftResponseLive = previous.sessionId === "" && previous.hadPending;
    assistantAnimationRef.current = {
      sessionId: activeSessionId,
      known: keepDraftResponseLive ? previous.known : new Set(assistantMessageIds),
      active: keepDraftResponseLive ? previous.active : new Set(),
      hadPending: hasPendingAssistant
    };
  } else {
    assistantAnimationRef.current.hadPending = hasPendingAssistant;
  }

  const completeAssistantAnimation = useCallback((messageId: string) => {
    assistantAnimationRef.current?.active.delete(messageId);
  }, []);

  useEffect(() => {
    setPins(loadPins(storageKey));
  }, [storageKey]);

  useEffect(() => {
    savePins(storageKey, visiblePins);
  }, [storageKey, visiblePins]);

  useLayoutEffect(() => {
    followTranscriptLatest(messagesRef.current, "smooth");
  }, [latestMessage?.id, latestMessage?.text]);

  const pinnedIds = new Set(visiblePins.map((pin) => pin.id));

  const togglePin = (message: TranscriptMessage) => {
    setPins((current) =>
      current.some((pin) => pin.id === message.id)
        ? current.filter((pin) => pin.id !== message.id)
        : [...current, { id: message.id, text: message.text }]
    );
  };

  const copyMessage = (message: TranscriptMessage) => {
    void globalThis.navigator?.clipboard?.writeText(message.text);
    setCopiedId(message.id);
    globalThis.setTimeout(() => setCopiedId((id) => (id === message.id ? null : id)), 1200);
  };

  const jumpToChapter = (id: string) => {
    const item = messagesRef.current?.querySelector<HTMLElement>(`[data-msg-id="${CSS.escape(id)}"]`);
    if (!item) {
      return;
    }
    item.scrollIntoView({ block: "nearest", behavior: "smooth" });
    const pill = item.querySelector<HTMLElement>(".transcriptPill");
    pill?.classList.add("transcriptPill--flash");
    globalThis.setTimeout(() => pill?.classList.remove("transcriptPill--flash"), 900);
  };

  if (messages.length === 0) {
    return null;
  }
  // Single-session audit anchor: <div className="chatCanvas">
  return (
    <div className={className}>
      {visiblePins.length > 0 ? (
        <div className="chatCanvas__chapters" aria-label="Pinned chapters">
          {visiblePins.map((pin) => (
            <button type="button" className="chapterChip" key={pin.id} title={pin.text} onClick={() => jumpToChapter(pin.id)}>
              <PinGlyph filled />
              <span>{pin.text}</span>
            </button>
          ))}
        </div>
      ) : null}
      <div className="chatCanvas__messages" ref={messagesRef}>
        {messages.map((message, index) => {
          const attachments = message.attachments ?? [];
          const visualAttachments = attachments.filter(isTranscriptVisualAttachment);
          if (!message.text.trim() && attachments.length === 0) {
            return null;
          }
          const previousMessage = messages[index - 1];
          const followsVisualUserMessage =
            message.role !== "user" &&
            previousMessage?.role === "user" &&
            (previousMessage.attachments ?? []).some(isTranscriptVisualAttachment);
          const role = message.role === "user" ? "user" : "assistant";
          const pinned = pinnedIds.has(message.id);
          const assistantPending = role === "assistant" && message.id.startsWith("assistant-pending-");
          const assistantError = role === "assistant" && message.id.startsWith("assistant-error-");
          const assistantCanAnimate = role === "assistant" && !assistantPending && !assistantError;
          let assistantShouldAnimate = false;
          if (assistantCanAnimate && assistantAnimationRef.current) {
            const animationState = assistantAnimationRef.current;
            if (animationState.active.has(message.id)) {
              assistantShouldAnimate = true;
            } else if (!animationState.known.has(message.id) && message.id === latestAssistantMessageId) {
              animationState.known.add(message.id);
              animationState.active.add(message.id);
              assistantShouldAnimate = true;
            } else {
              animationState.known.add(message.id);
            }
          }
          const renderedMessage = message;
          const actions = (
            <div className="transcriptActions">
              <button
                type="button"
                className="transcriptActionBtn"
                aria-label="Copy message"
                title={copiedId === message.id ? "Copied" : "Copy"}
                onClick={() => copyMessage(message)}
              >
                <CopyGlyph />
              </button>
              <button
                type="button"
                className={pinned ? "transcriptActionBtn transcriptActionBtn--active" : "transcriptActionBtn"}
                aria-label="Pin as chapter"
                aria-pressed={pinned}
                title="Pin"
                onClick={() => togglePin(message)}
              >
                <PinGlyph filled={pinned} />
              </button>
            </div>
          );
          const item = (
            <div
              className={`transcriptItem transcriptItem--${role}${followsVisualUserMessage ? " transcriptItem--afterVisualMedia" : ""}`}
              data-msg-id={message.id}
              key={message.id}
            >
              {role === "assistant" ? (
                <div className="transcriptTextFrame">
                  <div
                    className={
                      assistantPending
                        ? "transcriptPill transcriptPill--assistant transcriptPill--assistantPending"
                        : assistantError
                          ? "transcriptPill transcriptPill--assistant transcriptPill--assistantError"
                          : "transcriptPill transcriptPill--assistant"
                    }
                  >
                    {assistantPending ? (
                      <PendingAssistantText />
                    ) : assistantShouldAnimate ? (
                      <AnimatedAssistantText message={renderedMessage} onAnimationComplete={completeAssistantAnimation} />
                    ) : (
                      <StaticAssistantText message={renderedMessage} />
                    )}
                  </div>
                  {assistantPending ? null : actions}
                </div>
              ) : (
                <>
                  {visualAttachments.length > 0 ? (
                    <div className="transcriptFloatMedia" aria-label="Attached visual media">
                      <TranscriptAttachmentStack previews={visualAttachments} onEditImage={onEditImage} />
                    </div>
                  ) : null}
                  <div className="transcriptUserRow">
                    <div className="transcriptUserStack">
                      {message.text.trim() ? (
                        <div className="transcriptUserTextFrame">
                          <p className="transcriptPill transcriptPill--user">{message.text}</p>
                          {actions}
                        </div>
                      ) : null}
                      {visualAttachments.length > 0 ? <TranscriptVisualAttachmentEvents previews={visualAttachments} /> : null}
                    </div>
                  </div>
                </>
              )}
            </div>
          );
          return item;
        })}
      </div>
    </div>
  );
}

interface PanelsChatBottomSliceProps {
  parallelPrompts?: string[];
  onParallelPromptChange?: (index: number, value: string) => void;
  webExplorerOpen?: boolean;
  composerModule?: SidebarModuleId | null;
}

export function PanelsChatBottomSlice({
  parallelPrompts,
  onParallelPromptChange,
  webExplorerOpen = false,
  composerModule = null
}: PanelsChatBottomSliceProps = {}) {
  const { snapshot } = usePanelsChatBottomStore();
  const [draft, setDraft] = useState(snapshot.composer.chatText);
  const [focusedParallelIndex, setFocusedParallelIndex] = useState(0);
  const [moduleDropPhase, setModuleDropPhase] = useState<"idle" | "armed" | "over">("idle");
  const [fileDropPhase, setFileDropPhase] = useState<"idle" | "armed" | "over">("idle");
  const fileDragDepthRef = useRef(0);
  const composerRef = useRef<HTMLFormElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const burstRef = useRef<ComposerSendBurstHandle>(null);

  useEffect(() => {
    void panelsChatBottomStore.refresh();
  }, []);

  useEffect(() => {
    const onModuleDragZone = (event: Event) => {
      const detail = (event as CustomEvent<ModuleDragZoneDetail>).detail;
      if (detail.phase === "start") {
        setModuleDropPhase("armed");
      } else if (detail.phase === "move") {
        setModuleDropPhase(detail.over ? "over" : "armed");
      } else {
        setModuleDropPhase("idle");
      }
    };
    window.addEventListener(MODULE_DRAG_ZONE_EVENT, onModuleDragZone);
    return () => window.removeEventListener(MODULE_DRAG_ZONE_EVENT, onModuleDragZone);
  }, []);

  useEffect(() => {
    setDraft(snapshot.composer.chatText);
  }, [snapshot.composer.chatText]);

  useEffect(() => {
    const focusComposer = () => inputRef.current?.focus();
    globalThis.window?.addEventListener(IMAGE_EDIT_STAGED_EVENT, focusComposer);
    return () => globalThis.window?.removeEventListener(IMAGE_EDIT_STAGED_EVENT, focusComposer);
  }, []);

  useEffect(() => {
    const preventExternalFileNavigation = (event: DragEvent) => {
      if (!hasDraggedFiles(event.dataTransfer)) {
        return;
      }
      event.preventDefault();
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = "none";
      }
    };
    window.addEventListener("dragover", preventExternalFileNavigation);
    window.addEventListener("drop", preventExternalFileNavigation);
    return () => {
      window.removeEventListener("dragover", preventExternalFileNavigation);
      window.removeEventListener("drop", preventExternalFileNavigation);
    };
  }, []);

  useLayoutEffect(() => {
    const node = inputRef.current;
    if (!node) {
      return;
    }
    node.style.height = "auto";
    const inputHeight = Math.min(node.scrollHeight, COMPOSER_MAX_INPUT_HEIGHT);
    node.style.height = `${inputHeight}px`;
    composerRef.current?.style.setProperty("--composer-input-height", `${inputHeight}px`);
  }, [draft]);

  const dispatch = panelsChatBottomStore.dispatch;
  const providers = snapshot.composer.providers;
  const uploadPreviews = snapshot.composer.uploadPreviews;
  const canvasMessages = snapshot.transcript.filter((message) => message.role !== "system");
  const activeDropPhase = fileDropPhase !== "idle" ? fileDropPhase : moduleDropPhase;
  const attachFiles = () => void dispatch({ kind: "attach_files" });
  const attachDroppedFiles = (filePaths: string[]) => {
    if (filePaths.length === 0) {
      return;
    }
    void dispatch({ kind: "attach_dropped_files", filePaths }).then(() => inputRef.current?.focus());
  };
  const stageImageForEdit = useCallback((preview: ComposerUploadPreview) => {
    void dispatch({ kind: "stage_attachment_for_edit", attachmentIds: [preview.id] }).then(() => {
      globalThis.window?.dispatchEvent(new CustomEvent(IMAGE_EDIT_STAGED_EVENT, { detail: { attachmentId: preview.id } }));
      inputRef.current?.focus();
    });
  }, [dispatch]);
  const parallelMode = Boolean(parallelPrompts && parallelPrompts.length > 1 && onParallelPromptChange);
  const focusedParallelPrompt = parallelMode && parallelPrompts ? parallelPrompts[focusedParallelIndex]?.trim() ?? "" : "";
  const firstFilledParallelIndex = parallelMode && parallelPrompts ? parallelPrompts.findIndex((prompt) => prompt.trim().length > 0) : -1;
  const filledParallelPrompts = parallelMode && parallelPrompts
    ? parallelPrompts
      .map((prompt, index) => ({ index, value: prompt.trim() }))
      .filter((prompt) => prompt.value.length > 0)
    : [];
  const submitText = parallelMode && parallelPrompts
    ? filledParallelPrompts.map((prompt) => prompt.value).join("\n")
    : draft;
  const canSend = Boolean(submitText.trim() || uploadPreviews.length > 0);
  const commitParallelPrompt = (targetParallelIndex: number, value: string) => {
    if (parallelMode && onParallelPromptChange) {
      onParallelPromptChange(targetParallelIndex, "");
    }
    void dispatch({
      kind: "send_chat",
      value,
      moduleId: composerModule ?? undefined,
      parallelSessionIndex: targetParallelIndex
    });
  };
  const sendComposer = (parallelIndex?: number) => {
    if (parallelMode && parallelPrompts && parallelIndex === undefined && filledParallelPrompts.length > 0) {
      const sessionIsNew = !snapshot.transcript.some((message) => message.role === "user" || message.role === "assistant");
      const burst = burstRef.current;
      const commit = () => {
        for (const prompt of filledParallelPrompts) {
          onParallelPromptChange?.(prompt.index, "");
        }
        void dispatch({
          kind: "send_parallel_chat_batch",
          parallelDrafts: filledParallelPrompts.map((prompt) => ({
            parallelSessionIndex: prompt.index,
            value: prompt.value
          })),
          moduleId: composerModule ?? undefined
        });
      };
      if (sessionIsNew && burst) {
        burst.fire(composerRef.current, commit);
      } else {
        commit();
      }
      return;
    }
    const targetParallelIndex = parallelMode && parallelPrompts
      ? parallelIndex ?? (focusedParallelPrompt || uploadPreviews.length > 0 ? focusedParallelIndex : Math.max(0, firstFilledParallelIndex))
      : 0;
    const value = parallelMode && parallelPrompts ? parallelPrompts[targetParallelIndex]?.trim() ?? "" : draft;
    if (!value.trim() && uploadPreviews.length === 0) {
      return;
    }
    const commit = () => {
      if (parallelMode && parallelPrompts && onParallelPromptChange) {
        onParallelPromptChange(targetParallelIndex, "");
      }
      void dispatch({
        kind: "send_chat",
        value,
        moduleId: composerModule ?? undefined,
        parallelSessionIndex: parallelMode ? targetParallelIndex : undefined
      });
    };
    const targetTranscript = targetParallelIndex > 0
      ? snapshot.parallelLanes.find((lane) => lane.index === targetParallelIndex)?.transcript ?? []
      : snapshot.transcript;
    const sessionIsNew = !targetTranscript.some((message) => message.role === "user" || message.role === "assistant");
    const burst = burstRef.current;
    if (sessionIsNew && burst) {
      burst.fire(composerRef.current, commit);
    } else {
      commit();
    }
  };

  return (
    <section
      className="panelsChatBottom"
      aria-label="Panels chat composer and bottom controls"
      onDragEnter={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        event.preventDefault();
        fileDragDepthRef.current += 1;
        setFileDropPhase(targetInsideComposer(event.target) ? "over" : "armed");
      }}
      onDragOver={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        event.preventDefault();
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = "copy";
        }
        setFileDropPhase(targetInsideComposer(event.target) ? "over" : "armed");
      }}
      onDragLeave={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        fileDragDepthRef.current = Math.max(0, fileDragDepthRef.current - 1);
        if (fileDragDepthRef.current === 0) {
          setFileDropPhase("idle");
        }
      }}
      onDrop={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        event.preventDefault();
        fileDragDepthRef.current = 0;
        setFileDropPhase("idle");
        attachDroppedFiles(droppedFilePaths(event.dataTransfer));
      }}
    >
      {parallelMode && parallelPrompts ? (
        <div className={`parallelTranscriptGrid parallelTranscriptGrid--count${parallelPrompts.length}`}>
          {parallelPrompts.map((_prompt, index) => {
            const lane = snapshot.parallelLanes.find((candidate) => candidate.index === index);
            const laneMessages = index === 0 ? canvasMessages : lane?.transcript.filter((message) => message.role !== "system") ?? [];
            return (
              <TranscriptCanvas
                activeSessionId={index === 0 ? snapshot.activeSessionId : lane?.sessionId ?? `parallel-${index}`}
                messages={laneMessages}
                className="chatCanvas chatCanvas--parallelPane"
                key={`parallel-transcript-${index}`}
                onEditImage={stageImageForEdit}
              />
            );
          })}
        </div>
      ) : (
        <TranscriptCanvas
          key={snapshot.activeSessionId || "draft-session"}
          activeSessionId={snapshot.activeSessionId}
          messages={canvasMessages}
          onEditImage={stageImageForEdit}
        />
      )}
      <div
        className={activeDropPhase !== "idle" ? "composerDropScrim composerDropScrim--active" : "composerDropScrim"}
        aria-hidden="true"
      />
      <form
        ref={composerRef}
        className={[
          "composer",
          activeDropPhase !== "idle" ? "composer--moduleDropArmed" : "",
          activeDropPhase === "over" ? "composer--moduleDropOver" : "",
          fileDropPhase !== "idle" ? "composer--fileDropArmed" : ""
        ].filter(Boolean).join(" ")}
        aria-label="Chat composer"
        onSubmit={(event) => {
          event.preventDefault();
          sendComposer();
        }}
      >
        <div
          className="composer__upload"
          role="button"
          tabIndex={0}
          aria-label="Attach files"
          onClick={attachFiles}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              attachFiles();
            }
          }}
        >
          {snapshot.composer.uploadPreviewKind === "IMAGE_EDIT_TARGET" ? (
            <div className="composerImageEditPill" role="status" aria-live="polite">
              <EditImageGlyph />
              <span>image a modifier</span>
            </div>
          ) : null}
          <div className="composerUploadPreview" aria-hidden={uploadPreviews.length > 0 ? undefined : true}>
            <UploadPreviewGrid previews={uploadPreviews} />
          </div>
          <span>+</span>
        </div>
        {snapshot.composer.uploadErrorText ? (
          <div className="composer__uploadError" role="status" aria-live="polite">
            {snapshot.composer.uploadErrorText}
          </div>
        ) : null}
        {composerModule ? (
          <span className="composerWebExplorerLogo composerWebExplorerLogo--module" aria-hidden="true">
            <ModuleLogo id={composerModule} />
          </span>
        ) : webExplorerOpen ? (
          <span className="composerWebExplorerLogo" aria-hidden="true" />
        ) : null}
        <div className="composer__divider" />
        {parallelMode && parallelPrompts && onParallelPromptChange ? (
          <div className={`composerParallelPrompts composerParallelPrompts--count${parallelPrompts.length}`}>
            {parallelPrompts.map((prompt, index) => (
              <label className="composerParallelPrompt" key={`parallel-prompt-${index}`}>
                {index > 0 ? <span className="composerParallelPrompt__slash" aria-hidden="true">/</span> : null}
                <textarea
                  className="composerParallelPrompt__input"
                  aria-label={`Prompt ${index + 1}`}
                  value={prompt}
                  rows={1}
                  spellCheck={false}
                  placeholder={`Prompt ${index + 1}`}
                  onFocus={() => setFocusedParallelIndex(index)}
                  onChange={(event) => onParallelPromptChange(index, event.currentTarget.value)}
                  onKeyDown={(event) => {
                    emitChatKeyColor(event);
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      sendComposer(index);
                    }
                  }}
                />
              </label>
            ))}
          </div>
        ) : (
          <textarea
            ref={inputRef}
            className="composer__input"
            aria-label="Message"
            value={draft}
            rows={1}
            spellCheck={false}
            onChange={(event) => {
              setDraft(event.currentTarget.value);
              void dispatch({ kind: "chat_text_edited", value: event.currentTarget.value });
            }}
            onKeyDown={(event) => {
              emitChatKeyColor(event);
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                sendComposer();
              }
            }}
          />
        )}
        <div className="composer__providers" aria-label="LLM providers">
          {providers.map((provider) => (
            <button
              type="button"
              className={[
                "providerDot",
                provider.connected ? "providerDot--connected" : "",
                provider.active ? "providerDot--active" : ""
              ].filter(Boolean).join(" ")}
              key={provider.provider}
              aria-label={provider.label}
              onClick={() => void dispatch({ kind: provider.connected ? "select_llm" : "open_llm_providers", provider: provider.provider })}
            >
              <ProviderLogo provider={provider.provider} size={15} />
            </button>
          ))}
        </div>
        <button
          type="submit"
          className={canSend ? "composer__send" : "composer__send composer__send--empty"}
          aria-label="Send message"
          disabled={!canSend}
        >
          <svg className="sendGlyph" width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
            <path
              fill="currentColor"
              d="M20 4v9a4 4 0 0 1-4 4H6.914l2.5 2.5L8 20.914L3.086 16L8 11.086L9.414 12.5l-2.5 2.5H16a2 2 0 0 0 2-2V4h2Z"
            />
          </svg>
        </button>
      </form>

      <nav className="bottomControls" aria-label="Bottom controls">
        <button
          type="button"
          onClick={() =>
            void dispatch({
              kind: "permission_mode_selected",
              value:
                snapshot.composer.permissionMode === "ask-permissions"
                  ? "auto-accept-edits"
                  : snapshot.composer.permissionMode === "auto-accept-edits"
                    ? "full-autonomy"
                    : "ask-permissions"
            })
          }
        >
          {snapshot.composer.permissionMode}
        </button>
        <div className="bottomControls__models">
          <button
            type="button"
            className="bottomControls__modelButton"
            title={snapshot.composer.modelLabel}
            onClick={() => void dispatch({ kind: "cycle_llm_model", provider: snapshot.composer.selectedProvider, direction: 1 })}
          >
            {snapshot.composer.modelLabel}
          </button>
          <button
            type="button"
            className="bottomControls__reasoningButton"
            title={`reasoning ${snapshot.composer.reasoningLabel}`}
            onClick={() => void dispatch({ kind: "cycle_llm_reasoning", direction: 1 })}
          >
            {snapshot.composer.reasoningLabel}
          </button>
        </div>
      </nav>

      <ComposerSendBurst ref={burstRef} />
    </section>
  );
}
