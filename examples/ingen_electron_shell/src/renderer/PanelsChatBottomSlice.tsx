import { Fragment, useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import { Ban, ChevronLeft, ChevronRight, ChevronsUpDown, Copy, FolderPlus, List, ListChecks, MoveRight, Pencil, RefreshCw, Search, Square, Terminal, Trash2, ShieldAlert, ShieldCheck, Sparkles } from "lucide-react";
import type { Camera, Object3D } from "three";
import type { BrainCodeActCommand, ComposerUploadPreview, PanelsChatBottomCommand, PanelsChatBottomSnapshot, TranscriptMessage } from "../shared/ipc-contract";
import {
  BRAIN_BRAIN_COMMAND,
  BRAIN_AIRBNB_COMMAND,
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS,
  BRAIN_CODEACT_COMMANDS,
  BRAIN_FRONTDESIGN_COMMAND,
  BRAIN_GMAIL_COMMAND,
  BRAIN_GMAIL_COM_COMMAND,
  BRAIN_GOOGLEWEB_COMMAND,
  BRAIN_MAPS_COMMAND,
  BRAIN_GOOGLE_AGENDA_COMMAND,
  BRAIN_NAMED_COMPUTE_COMMAND,
  BRAIN_NEWCOMPUTE_COMMAND,
  BRAIN_SCIENCE_COMMAND,
  BRAIN_CODING_COMMAND,
  BRAIN_EDITIMAGE_COMMAND,
  BRAIN_NEWIMAGE_COMMAND,
  BRAIN_NEWMODULE_COMMAND,
  BRAIN_NEWOBJECT_COMMAND,
  BRAIN_QUESTIONNAIRE_COMMAND,
  BRAIN_RUST_PORT_ADAPTER_COMMAND,
  BRAIN_RUST_STATE_STORE_COMMAND,
  BRAIN_SEARCHARCHIVE_COMMAND,
  BRAIN_SELECTCOMPUTE_COMMAND,
  BRAIN_WORKSPACE_COMMAND,
  BRAIN_WEB_COMMAND
} from "../shared/ipc-contract";
import { ComposerSendBurst, type ComposerSendBurstHandle } from "./ComposerSendBurst";
import {
  AGENT_COPY_PATH_COMMAND,
  AGENT_CREATE_DIRECTORY_COMMAND,
  AGENT_DELETE_EMPTY_DIRECTORY_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
  AGENT_LIST_COMMAND,
  AGENT_MOVE_PATH_COMMAND,
  AGENT_READONLY_SHELL_COMMAND,
  AGENT_RENAME_PATH_COMMAND,
  AGENT_SEARCH_COMMAND,
  AGENT_SHELL_COMMAND,
  agentActionEventFromLine,
  agentActionEventText,
  agentActionEventCommandFromToken,
  type AgentActionEventCommand
} from "./agent-action-events";
import { BRAIN_AGENT_MEMORY_UPDATED_EVENT, readBrainAgentMemory } from "./brain-user-memory-store";
import { panelsChatBottomStore, usePanelsChatBottomStore } from "./panels-chat-bottom-store";
import { ProviderLogo } from "./ProviderLogo";
import { MODULE_DRAG_ZONE_EVENT, type ModuleDragZoneDetail, type SidebarModuleId } from "./SidebarSlice";
import { assistantGeoEntityLabel } from "./assistant-geo-entities";
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

function fileDropEffectForTarget(target: EventTarget | null): DataTransfer["dropEffect"] {
  return targetInsideComposer(target) ? "copy" : "none";
}

function useVisibleAutoplayVideo(videoRef: RefObject<HTMLVideoElement | null>, sourceKey: string) {
  useEffect(() => {
    const video = videoRef.current;
    if (!video) {
      return undefined;
    }

    let visible = true;
    const syncPlayback = () => {
      if (document.hidden || !visible) {
        video.pause();
        return;
      }
      void video.play().catch(() => undefined);
    };
    const observer = typeof IntersectionObserver === "undefined"
      ? null
      : new IntersectionObserver(
        ([entry]) => {
          visible = Boolean(entry?.isIntersecting && entry.intersectionRatio > 0);
          syncPlayback();
        },
        { threshold: [0, 0.01] }
      );

    observer?.observe(video);
    document.addEventListener("visibilitychange", syncPlayback);
    video.addEventListener("loadedmetadata", syncPlayback);
    video.addEventListener("canplay", syncPlayback);
    syncPlayback();

    return () => {
      observer?.disconnect();
      document.removeEventListener("visibilitychange", syncPlayback);
      video.removeEventListener("loadedmetadata", syncPlayback);
      video.removeEventListener("canplay", syncPlayback);
      video.pause();
    };
  }, [sourceKey, videoRef]);
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
  useVisibleAutoplayVideo(videoRef, preview.url);

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

export type TranscriptMediaFrame = "portrait" | "landscape" | "nineSixteen" | "square";

type BlobPoint = {
  x: number;
  y: number;
};

export type BlobBox = {
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

export function blobBoxForFrame(frame: TranscriptMediaFrame): BlobBox {
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

export function dreamBlobBlur(box: BlobBox): number {
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

export function blobPathForAttachment(seed: string, frame: TranscriptMediaFrame, box: BlobBox): string {
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
          preserveAspectRatio="xMidYMid meet"
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
  useVisibleAutoplayVideo(videoRef, preview.url);

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

export function ThreeUploadPreview({
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
      let needsResize = true;
      let lastWidth = 0;
      let lastHeight = 0;
      let viewportVisible = typeof IntersectionObserver === "undefined";
      let renderActive = false;
      const resize = () => {
        const width = Math.max(1, Math.floor(host.clientWidth));
        const height = Math.max(1, Math.floor(host.clientHeight));
        if (width === lastWidth && height === lastHeight && !needsResize) {
          return;
        }
        lastWidth = width;
        lastHeight = height;
        needsResize = false;
        renderer.setSize(width, height, false);
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
      };
      const scheduleRender = () => {
        if (frameId === 0 && !disposed && renderActive) {
          frameId = window.requestAnimationFrame(renderScene);
        }
      };
      const stopRender = () => {
        if (frameId !== 0) {
          window.cancelAnimationFrame(frameId);
          frameId = 0;
        }
      };
      const setViewportVisible = (active: boolean) => {
        viewportVisible = active;
        renderActive = viewportVisible && !document.hidden;
        if (renderActive) {
          scheduleRender();
        } else {
          stopRender();
        }
      };
      const renderScene = (time = 0) => {
        frameId = 0;
        if (disposed) {
          return;
        }
        if (!renderActive) {
          return;
        }
        if (needsResize) {
          resize();
        }
        modelRoot.rotation.y = 0.72 + time * 0.00034;
        modelRoot.rotation.x = -0.18;
        camera.lookAt(0, 0, 0);
        renderer.render(scene, camera);
        scheduleRender();
      };
      const resizeObserver = new ResizeObserver(() => {
        needsResize = true;
        scheduleRender();
      });
      resizeObserver.observe(host);
      const visibilityObserver = typeof IntersectionObserver === "undefined"
        ? null
        : new IntersectionObserver(
          ([entry]) => setViewportVisible(Boolean(entry?.isIntersecting && entry.intersectionRatio > 0)),
          { threshold: [0, 0.01] }
        );
      visibilityObserver?.observe(host);
      const onVisibilityChange = () => setViewportVisible(viewportVisible);
      document.addEventListener("visibilitychange", onVisibilityChange);
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
        stopRender();
        resizeObserver.disconnect();
        visibilityObserver?.disconnect();
        document.removeEventListener("visibilitychange", onVisibilityChange);
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

      setViewportVisible(viewportVisible);
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

function AssistantTaskCheck({ checked }: { checked: boolean }) {
  return (
    <span className={checked ? "assistantText__taskCheck assistantText__taskCheck--checked" : "assistantText__taskCheck"} aria-hidden="true">
      <span className="assistantText__taskCheckRail" />
      <span className="assistantText__taskCheckDot" />
    </span>
  );
}

function copyTextToClipboard(text: string): Promise<boolean> {
  const clipboard = globalThis.navigator?.clipboard;
  if (clipboard?.writeText) {
    return clipboard.writeText(text).then(
      () => true,
      () => false
    );
  }
  const documentRef = globalThis.document;
  if (!documentRef?.body) {
    return Promise.resolve(false);
  }
  const textarea = documentRef.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "true");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  documentRef.body.appendChild(textarea);
  textarea.select();
  let copied = false;
  try {
    copied = documentRef.execCommand("copy");
  } catch {
    copied = false;
  } finally {
    textarea.remove();
  }
  return Promise.resolve(copied);
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
  | { kind: "task_list"; items: AssistantTaskListItem[] }
  | { kind: "paragraph"; text: string }
  | { kind: "table"; headers: string[]; alignments: AssistantTableAlignment[]; rows: string[][] }
  | { kind: "code"; language: string; code: string }
  | { kind: "quote"; text: string }
  | { kind: "callout"; tone: AssistantCalloutTone; title: string; body: string }
  | { kind: "facts"; items: AssistantFactItem[] }
  | { kind: "divider" }
  | { kind: "event"; event: TranscriptCodeActEvent }
  | { kind: "event_group"; events: TranscriptCodeActEvent[] };

const CONTEXT_COMPACTION_COMMAND = "/context_compaction_";

type TranscriptContextCompactionState = "compressing" | "compressed";
type TranscriptCodeActCommand = BrainCodeActCommand | AgentActionEventCommand | `/compute_${string}_` | typeof CONTEXT_COMPACTION_COMMAND;
type AssistantTableAlignment = "left" | "center" | "right";
type AssistantCalloutTone = "info" | "warning" | "success" | "assumption";

interface AssistantTaskListItem {
  checked: boolean;
  text: string;
}

interface AssistantFactItem {
  label: string;
  value: string;
}

interface AssistantMacroListItem {
  label: string;
  body: string;
}

interface TranscriptCodeActEvent {
  command: TranscriptCodeActCommand;
  text: string;
  detail?: string;
  path?: string;
  toPath?: string;
  compactionState?: TranscriptContextCompactionState;
}

const BRAIN_CODEACT_COMMAND_SET = new Set<string>(BRAIN_CODEACT_COMMANDS);
const BRAIN_CODEACT_COMMANDS_BY_LENGTH = [...BRAIN_CODEACT_COMMANDS].sort((left, right) => right.length - left.length);
const BRAIN_CODEACT_DESCRIPTION_BY_COMMAND = new Map<string, string>(
  BRAIN_CODEACT_COMMAND_DESCRIPTIONS.map((entry) => [entry.command, entry.description])
);

const TRANSCRIPT_CODEACT_EVENT_TEXT = new Map<string, string>([
  [BRAIN_SEARCHARCHIVE_COMMAND, "archive memory search returned bounded context"],
  [BRAIN_GOOGLEWEB_COMMAND, "native Google WebExplorer search event created"],
  [BRAIN_MAPS_COMMAND, "Use Google Earth"],
  [BRAIN_GMAIL_COMMAND, "Gmail event prepared"],
  [BRAIN_GMAIL_COM_COMMAND, "Gmail surface opened"],
  [BRAIN_AIRBNB_COMMAND, "Use Airbnb"],
  [BRAIN_NEWIMAGE_COMMAND, "image generation prepared"],
  [BRAIN_EDITIMAGE_COMMAND, "image edit prepared"],
  [BRAIN_QUESTIONNAIRE_COMMAND, "Questionnaire opened"],
  [BRAIN_SCIENCE_COMMAND, "Changed from General Brain to Science Brain"],
  [BRAIN_CODING_COMMAND, "Changed from General Brain to Coding Brain"],
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

function contextCompactionStateFromLine(line: string): TranscriptContextCompactionState {
  const match = /(?:^|\s)state\s*=\s*("([^"]+)"|'([^']+)'|([^\s]+))/.exec(line);
  const value = (match?.[2] ?? match?.[3] ?? match?.[4] ?? "").trim().toLowerCase();
  return value === "compressed" ? "compressed" : "compressing";
}

function transcriptCodeActEvent(command: TranscriptCodeActCommand, line = ""): TranscriptCodeActEvent {
  const event: TranscriptCodeActEvent = {
    command,
    text: codeActEventText(command)
  };
  if (command === CONTEXT_COMPACTION_COMMAND) {
    event.compactionState = contextCompactionStateFromLine(line);
  }
  return event;
}

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
  if (command === CONTEXT_COMPACTION_COMMAND) {
    return "Compressing context";
  }
  const agentActionText = agentActionEventCommandFromToken(command);
  if (agentActionText) {
    return agentActionEventText(agentActionText);
  }
  if (isDynamicComputeCommand(command)) {
    const label = dynamicComputeLabel(command);
    return `${label || "named"} compute executed`;
  }
  return TRANSCRIPT_CODEACT_EVENT_TEXT.get(command) || BRAIN_CODEACT_DESCRIPTION_BY_COMMAND.get(command) || "CodeAct command executed";
}

function readCodeActCommand(value: string): TranscriptCodeActCommand | undefined {
  const trimmed = value.trim().replace(/^["'`]+|["'`,;]+$/g, "");
  if (trimmed === CONTEXT_COMPACTION_COMMAND) {
    return CONTEXT_COMPACTION_COMMAND;
  }
  const agentActionCommand = agentActionEventCommandFromToken(trimmed);
  if (agentActionCommand) {
    return agentActionCommand;
  }
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
  const agentActionEvent = agentActionEventFromLine(trimmed);
  if (agentActionEvent) {
    return agentActionEvent;
  }
  const commandAssignment = /(?:^|\s)command\s*=\s*("([^"]+)"|'([^']+)'|([^\s]+))/.exec(trimmed);
  const assignedCommand = commandAssignment ? readCodeActCommand(commandAssignment[2] ?? commandAssignment[3] ?? commandAssignment[4] ?? "") : undefined;
  if (assignedCommand) {
    return transcriptCodeActEvent(assignedCommand, trimmed);
  }
  const firstToken = trimmed.split(/\s+/, 1)[0] ?? "";
  const directCommand = readCodeActCommand(firstToken);
  if (directCommand) {
    return transcriptCodeActEvent(directCommand, trimmed);
  }
  const containedCommand = BRAIN_CODEACT_COMMANDS_BY_LENGTH.find((command) => trimmed.includes(command));
  if (containedCommand) {
    return transcriptCodeActEvent(containedCommand);
  }
  const dynamicCompute = /\/compute_[a-zA-Z0-9][a-zA-Z0-9_]*_/.exec(trimmed)?.[0];
  if (dynamicCompute && isDynamicComputeCommand(dynamicCompute)) {
    return transcriptCodeActEvent(dynamicCompute);
  }
  return undefined;
}

function isCodeActResultHeader(line: string): boolean {
  return /^[A-Z][A-Z0-9_]*_RESULT\b/.test(line.trim()) || /^R(?:e|\u00e9)sultat\s*:/i.test(line.trim());
}

function isCodeActMetadataLine(line: string): boolean {
  const trimmed = line.trim();
  return (
    isCodeActResultHeader(trimmed) ||
    /^(?:status|error|path|toPath|items?|stdout|stderr|exitCode|exit_code|proofHash|proof_hash|hash)\s*[:=]/i.test(trimmed) ||
    (/^[{[]/.test(trimmed) && /[}\]]$/.test(trimmed))
  );
}

function codeActMetadataDisplayText(line: string): string {
  const trimmed = line.trim();
  const result = /^R(?:e|\u00e9)sultat\s*:\s*(.*)$/i.exec(trimmed);
  if (result) {
    return `Result: ${result[1]}`.trim();
  }
  return trimmed;
}

function normalizeAssistantMarkdownText(text: string): string {
  return text
    .replace(/\r\n/g, "\n")
    .replace(/\|\s+\|(?=\s*:?---)/g, "|\n|")
    .replace(/\|\s+\|(?=\s*[^|\n]+\s*\|)/g, "|\n|")
    .replace(/([.!?])\s+(-\s+)/g, "$1\n$2")
    .replace(/([.!?])\s+(\d+[.)]\s+)/g, "$1\n$2");
}

function splitMarkdownTableRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes("|")) {
    return null;
  }
  const body = trimmed.replace(/^\|/, "").replace(/\|$/, "");
  const cells: string[] = [];
  let current = "";
  let escaped = false;
  for (const char of body) {
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }
    if (char === "\\") {
      escaped = true;
      continue;
    }
    if (char === "|") {
      cells.push(current.trim());
      current = "";
      continue;
    }
    current += char;
  }
  cells.push(current.trim());
  return cells.length >= 2 ? cells : null;
}

function markdownTableAlignments(line: string, expectedColumns: number): AssistantTableAlignment[] | null {
  const cells = splitMarkdownTableRow(line);
  if (!cells || cells.length !== expectedColumns) {
    return null;
  }
  const alignments: AssistantTableAlignment[] = [];
  for (const cell of cells) {
    const marker = cell.replace(/\s+/g, "");
    if (!/^:?-{3,}:?$/.test(marker)) {
      return null;
    }
    if (marker.startsWith(":") && marker.endsWith(":")) {
      alignments.push("center");
    } else if (marker.endsWith(":")) {
      alignments.push("right");
    } else {
      alignments.push("left");
    }
  }
  return alignments;
}

function fitMarkdownTableRow(cells: string[], width: number): string[] {
  return Array.from({ length: width }, (_value, index) => cells[index] ?? "");
}

function markdownFenceInfo(line: string): { marker: string; language: string } | null {
  const match = /^(```+|~~~+)\s*([a-zA-Z0-9_+.-]*)\s*$/.exec(line.trim());
  if (!match) {
    return null;
  }
  return {
    marker: match[1],
    language: match[2] || ""
  };
}

function isMarkdownFenceClose(line: string, marker: string): boolean {
  return line.trim().startsWith(marker);
}

function markdownDividerLine(line: string): boolean {
  return /^ {0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line);
}

function taskListItemFromLine(line: string): AssistantTaskListItem | null {
  const match = /^[-*]\s+\[([ xX])\]\s+(.+)$/.exec(line.trim());
  if (!match) {
    return null;
  }
  return {
    checked: match[1].toLowerCase() === "x",
    text: match[2].trim()
  };
}

function quoteTextFromLine(line: string): string | null {
  const match = /^>\s?(.*)$/.exec(line.trim());
  return match ? match[1].trim() : null;
}

function calloutFromLine(line: string): { tone: AssistantCalloutTone; title: string; body: string } | null {
  const match = /^(note|info|important|warning|attention|danger|hypoth[eè]se|assumption|source|résumé|resume|summary|conseil|tip)\s*[:：]\s*(.*)$/i.exec(line.trim());
  if (!match) {
    return null;
  }
  const label = match[1].normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const tone: AssistantCalloutTone =
    label === "warning" || label === "attention" || label === "danger"
      ? "warning"
      : label === "hypothese" || label === "assumption"
        ? "assumption"
        : label === "resume" || label === "summary" || label === "tip" || label === "conseil"
          ? "success"
          : "info";
  const title = label === "hypothese"
    ? "Hypothese"
    : match[1].charAt(0).toUpperCase() + match[1].slice(1).toLowerCase();
  return { tone, title, body: match[2].trim() };
}

function factItemFromLine(line: string): AssistantFactItem | null {
  const match = /^([A-Za-zÀ-ÖØ-öø-ÿ0-9][A-Za-zÀ-ÖØ-öø-ÿ0-9 /_.-]{1,38})\s*:\s+(.+)$/.exec(line.trim());
  if (!match || calloutFromLine(line)) {
    return null;
  }
  const label = match[1].trim();
  const value = match[2].trim();
  if (!label || !value || value.length > 220) {
    return null;
  }
  return { label, value };
}

function assistantMacroListItemFromText(text: string): AssistantMacroListItem | null {
  const match = /^\s*(.+?)\s*[:\uFF1A]\s+(.+)$/.exec(text.trim());
  if (!match || calloutFromLine(text)) {
    return null;
  }
  const label = match[1]
    .trim()
    .replace(/^\*\*(.+)\*\*$/, "$1")
    .replace(/^__(.+)__$/, "$1")
    .replace(/^`(.+)`$/, "$1")
    .trim();
  const body = match[2].trim();
  if (!label || !body || label.length > 42 || body.length < 8) {
    return null;
  }
  if (!/^[\p{L}\p{N}][\p{L}\p{N} /_.’'()-]{1,41}$/u.test(label)) {
    return null;
  }
  return { label, body };
}

function assistantMacroListItems(items: string[]): AssistantMacroListItem[] | null {
  if (items.length < 2) {
    return null;
  }
  const macroItems = items.map(assistantMacroListItemFromText);
  if (macroItems.some((item) => !item)) {
    return null;
  }
  return macroItems as AssistantMacroListItem[];
}

function assistantMarkdownBlocks(text: string): AssistantMarkdownBlock[] {
  const lines = normalizeAssistantMarkdownText(text).split("\n");
  const blocks: AssistantMarkdownBlock[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  let skippingCodeActMetadata = false;
  let sawCodeActMetadata = false;
  let lastEvent: TranscriptCodeActEvent | null = null;

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

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const rawLine = lines[lineIndex];
    const line = rawLine.trim();
    if (!line) {
      flushParagraph();
      flushList();
      skippingCodeActMetadata = false;
      lastEvent = null;
      continue;
    }
    if (isCodeActMetadataLine(line)) {
      if (lastEvent) {
        lastEvent.detail = codeActMetadataDisplayText(line);
      }
      flushParagraph();
      flushList();
      skippingCodeActMetadata = true;
      sawCodeActMetadata = true;
      continue;
    }
    if (skippingCodeActMetadata) {
      skippingCodeActMetadata = false;
      lastEvent = null;
    }
    const fence = markdownFenceInfo(line);
    if (fence) {
      flushParagraph();
      flushList();
      const codeLines: string[] = [];
      lineIndex += 1;
      while (lineIndex < lines.length && !isMarkdownFenceClose(lines[lineIndex], fence.marker)) {
        codeLines.push(lines[lineIndex]);
        lineIndex += 1;
      }
      blocks.push({ kind: "code", language: fence.language, code: codeLines.join("\n").replace(/\n+$/g, "") });
      continue;
    }
    if (markdownDividerLine(rawLine)) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "divider" });
      continue;
    }
    const event = codeActEventFromLine(line);
    if (event) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "event", event });
      lastEvent = event;
      if (event.command === BRAIN_QUESTIONNAIRE_COMMAND) {
        sawCodeActMetadata = true;
        break;
      }
      skippingCodeActMetadata = true;
      sawCodeActMetadata = true;
      continue;
    }
    const quoteLine = quoteTextFromLine(rawLine);
    if (quoteLine !== null) {
      flushParagraph();
      flushList();
      const quoteLines = [quoteLine];
      while (lineIndex + 1 < lines.length) {
        const nextQuote = quoteTextFromLine(lines[lineIndex + 1]);
        if (nextQuote === null) {
          break;
        }
        quoteLines.push(nextQuote);
        lineIndex += 1;
      }
      blocks.push({ kind: "quote", text: quoteLines.join("\n").trim() });
      continue;
    }
    const callout = calloutFromLine(line);
    if (callout) {
      flushParagraph();
      flushList();
      blocks.push({ kind: "callout", ...callout });
      continue;
    }
    const tableHeaders = splitMarkdownTableRow(line);
    const tableAlignments = tableHeaders
      ? markdownTableAlignments(lines[lineIndex + 1] ?? "", tableHeaders.length)
      : null;
    if (tableHeaders && tableAlignments) {
      flushParagraph();
      flushList();
      const rows: string[][] = [];
      lineIndex += 2;
      while (lineIndex < lines.length) {
        const rowLine = lines[lineIndex].trim();
        const rowCells = rowLine ? splitMarkdownTableRow(rowLine) : null;
        if (!rowCells) {
          lineIndex -= 1;
          break;
        }
        rows.push(fitMarkdownTableRow(rowCells, tableHeaders.length));
        lineIndex += 1;
      }
      blocks.push({
        kind: "table",
        headers: tableHeaders,
        alignments: tableAlignments,
        rows
      });
      continue;
    }
    const taskItem = taskListItemFromLine(line);
    if (taskItem) {
      flushParagraph();
      flushList();
      const items = [taskItem];
      while (lineIndex + 1 < lines.length) {
        const nextTaskItem = taskListItemFromLine(lines[lineIndex + 1]);
        if (!nextTaskItem) {
          break;
        }
        items.push(nextTaskItem);
        lineIndex += 1;
      }
      blocks.push({ kind: "task_list", items });
      continue;
    }
    const factItem = factItemFromLine(line);
    const nextFactItem = factItemFromLine(lines[lineIndex + 1] ?? "");
    if (factItem && nextFactItem) {
      flushParagraph();
      flushList();
      const items = [factItem, nextFactItem];
      lineIndex += 1;
      while (lineIndex + 1 < lines.length) {
        const followingFactItem = factItemFromLine(lines[lineIndex + 1]);
        if (!followingFactItem) {
          break;
        }
        items.push(followingFactItem);
        lineIndex += 1;
      }
      blocks.push({ kind: "facts", items });
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
  const renderedBlocks = blocks.length > 0 ? blocks : sawCodeActMetadata ? [] : [fallbackBlock];
  return groupAssistantCodeActEvents(renderedBlocks);
}

function shouldGroupAsCodexCommandEvent(event: TranscriptCodeActEvent): boolean {
  if (event.command === CONTEXT_COMPACTION_COMMAND) {
    return false;
  }
  if (isAgentActionCommand(event.command) && isAgentFileModificationCommand(event.command)) {
    return false;
  }
  return !isBrainSegmentCommand(event.command);
}

function groupAssistantCodeActEvents(blocks: AssistantMarkdownBlock[]): AssistantMarkdownBlock[] {
  const grouped: AssistantMarkdownBlock[] = [];
  let pendingEvents: TranscriptCodeActEvent[] = [];

  const flushEvents = () => {
    if (pendingEvents.length === 0) {
      return;
    }
    if (pendingEvents.length === 1) {
      grouped.push({ kind: "event", event: pendingEvents[0] });
    } else {
      grouped.push({ kind: "event_group", events: pendingEvents });
    }
    pendingEvents = [];
  };

  for (const block of blocks) {
    if (block.kind === "event" && shouldGroupAsCodexCommandEvent(block.event)) {
      pendingEvents.push(block.event);
      continue;
    }
    flushEvents();
    grouped.push(block);
  }
  flushEvents();
  return grouped;
}

function assistantRenderableText(text: string): string {
  return text
    .replace(/\/(["'`])[^"'`\r\n]{1,120}\1_renamechat_/g, "")
    .replace(/(^|\n)\s*\/(["'`])[^"'`\r\n]{0,120}(?:\2(?:_renamechat_?)?|_renamechat_?)?\s*$/g, "$1")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function assistantVisibleAnimationSource(text: string): string {
  const normalized = normalizeAssistantMarkdownText(assistantRenderableText(text));
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
    if (isCodeActMetadataLine(line)) {
      skippingCodeActMetadata = true;
      continue;
    }
    if (skippingCodeActMetadata) {
      skippingCodeActMetadata = false;
    }
    const event = codeActEventFromLine(line);
    if (event) {
      lastVisibleEnd = lineEnd;
      if (event.command === BRAIN_QUESTIONNAIRE_COMMAND) {
        break;
      }
      skippingCodeActMetadata = true;
      continue;
    }
    lastVisibleEnd = lineEnd;
  }

  return normalized.slice(0, lastVisibleEnd || normalized.trimEnd().length);
}

function assistantRevealBreakpoints(text: string): number[] {
  const breakpoints: number[] = [];
  const pattern = /\S+\s*|\s+/g;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (/\S/.test(match[0])) {
      breakpoints.push(match.index + match[0].length);
    }
  }
  if (text.length > 0 && breakpoints[breakpoints.length - 1] !== text.length) {
    breakpoints.push(text.length);
  }
  return breakpoints;
}

interface TranscriptQuestionnaire {
  title: string;
  intro: string;
  pages: TranscriptQuestionnairePage[];
  sourceMessageId: string;
}

interface TranscriptQuestionnairePage {
  question: string;
  options: [string, string, string];
}

interface TranscriptQuestionnaireOptionCopy {
  label: string;
  detail: string;
  tags: string[];
  colors: string[];
}

interface ParsedQuestionnaireQuestion {
  question: string;
  inlineOptions: string[];
}

interface QuestionnaireAnswer {
  kind: "option" | "other";
  value: string;
  otherText: string;
}

function unquoteCodeActSlotValue(value: string): string {
  const trimmed = value.trim();
  if ((trimmed.startsWith('"') && trimmed.endsWith('"')) || (trimmed.startsWith("'") && trimmed.endsWith("'"))) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function readQuestionnaireSlots(block: string): Map<string, string> {
  const slots = new Map<string, string>();
  const assignmentPattern = /\b(title|intro|questions|q\d+(?:_(?:options|option[123]|a|b|c))?|mode|output)\s*=\s*("[^"]*"|'[^']*'|[^\s]+)/g;
  let match: RegExpExecArray | null;
  while ((match = assignmentPattern.exec(block)) !== null) {
    slots.set(match[1], unquoteCodeActSlotValue(match[2] ?? ""));
  }
  for (const rawLine of block.split(/\r?\n/)) {
    const line = rawLine.trim();
    const colon = /^(title|intro|questions|q\d+(?:_(?:options|option[123]|a|b|c))?)\s*:\s*(.+)$/.exec(line);
    if (colon && !slots.has(colon[1])) {
      slots.set(colon[1], unquoteCodeActSlotValue(colon[2] ?? ""));
    }
  }
  return slots;
}

function splitQuestionnaireOptions(value: string, allowComma = false): string[] {
  const separator = allowComma ? /\s*\|\s*|\s*;\s*|\s*,\s*/ : /\s*\|\s*|\s*;\s*/;
  return value
    .split(separator)
    .map((option) => option.trim())
    .map((option) => option.replace(/[?.!]+$/g, "").trim())
    .filter(Boolean)
    .filter((option) => !/^other$/i.test(option) && !/^autre$/i.test(option));
}

const QUESTIONNAIRE_TAGS: Array<[RegExp, string]> = [
  [/\b(recommended|recommandee?|conseillee?|best|meilleur)\b/i, "Recommended"],
  [/\b(fast|rapide|quick|vite|prototype rapide)\b/i, "Fast"],
  [/\b(quality|qualite|premium|superior|superieur|high[- ]?end)\b/i, "Quality"],
  [/\b(ambitious|ambitieux|ambitieuse|advanced|avance|longer|plus long)\b/i, "Ambitious"],
  [/\b(cheap|cheaper|budget|moins cher|economique)\b/i, "Cheaper"],
  [/\b(safe|safer|sur|robuste|low[- ]?risk|faible risque)\b/i, "Safer"],
  [/\b(risky|riskier|risque|experimental|experimentale?)\b/i, "Riskier"]
];

function normalizedQuestionnaireTagSource(value: string): string {
  return value.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
}

function isQuestionnaireKnownTag(value: string): boolean {
  const normalized = normalizedQuestionnaireTagSource(value);
  return QUESTIONNAIRE_TAGS.some(([pattern]) => pattern.test(normalized));
}

function supportsQuestionnaireColor(value: string): boolean {
  const candidate = value.trim();
  if (!candidate || candidate.length > 96 || /[;{}<>]/.test(candidate)) {
    return false;
  }
  if (/^(currentcolor|transparent)$/i.test(candidate)) {
    return false;
  }
  if (/^#[0-9a-f]{3,8}$/i.test(candidate)) {
    return true;
  }
  if (!/^(rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\(/i.test(candidate)) {
    return false;
  }
  return typeof CSS !== "undefined" && typeof CSS.supports === "function"
    ? CSS.supports("color", candidate)
    : false;
}

function splitQuestionnaireColorList(value: string): string[] {
  const colors: string[] = [];
  let current = "";
  let depth = 0;
  for (const char of value) {
    if (char === "(") depth += 1;
    if (char === ")") depth = Math.max(0, depth - 1);
    if ((char === "," || char === ";") && depth === 0) {
      const color = current.trim();
      if (supportsQuestionnaireColor(color)) {
        colors.push(color);
      }
      current = "";
      continue;
    }
    current += char;
  }
  const color = current.trim();
  if (supportsQuestionnaireColor(color)) {
    colors.push(color);
  }
  return colors.slice(0, 4);
}

function extractQuestionnaireColors(option: string): { colors: string[]; text: string } {
  let colors: string[] = [];
  let text = option.replace(/\bcolors\s*:\s*([^)\]|]+?)(?=\s*(?:\)|\]|\||$)|\s+(?:\u2014|\u2013|-)\s+)/gi, (match, value: string) => {
    colors = splitQuestionnaireColorList(value);
    return colors.length > 0 ? " " : match;
  });
  text = text.replace(/\bcolor\s*:\s*(#[0-9a-f]{3,8}|(?:rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch|color)\([^)]*\))/gi, (match, value: string) => {
    const parsed = splitQuestionnaireColorList(value);
    if (parsed.length === 0) {
      return match;
    }
    if (colors.length === 0) {
      colors = parsed;
    }
    return " ";
  });
  return { colors, text: text.replace(/\s+/g, " ").trim() };
}

function questionnaireOptionTags(option: string): string[] {
  const tagSource = normalizedQuestionnaireTagSource(option.split(/\s+(?:\u2014|\u2013|-)\s+|:\s+/, 1)[0] ?? option);
  const tags: string[] = [];
  for (const [pattern, label] of QUESTIONNAIRE_TAGS) {
    if (pattern.test(tagSource) && !tags.includes(label)) {
      tags.push(label);
    }
  }
  return tags;
}

function stripQuestionnaireOptionTags(option: string): string {
  return option
    .replace(/\s*\(([^)]{1,32})\)\s*/g, (match, tag: string) => isQuestionnaireKnownTag(tag) ? " " : match)
    .replace(/\s+/g, " ")
    .trim();
}

function questionnaireOptionCopy(option: string): TranscriptQuestionnaireOptionCopy {
  const cleaned = option.trim();
  const colorExtraction = extractQuestionnaireColors(cleaned);
  const tags = questionnaireOptionTags(colorExtraction.text);
  const normalized = stripQuestionnaireOptionTags(colorExtraction.text);
  const split = /\s+(?:\u2014|\u2013|-)\s+/.exec(normalized) ?? /:\s+/.exec(normalized);
  if (!split) {
    return {
      label: normalized,
      detail: tags.includes("Recommended") ? "Option recommand\u00e9e pour avancer avec une bonne base." : "",
      tags,
      colors: colorExtraction.colors
    };
  }
  const label = normalized.slice(0, split.index).trim();
  const detail = normalized.slice(split.index + split[0].length).trim();
  return {
    label: label || normalized,
    detail,
    tags,
    colors: colorExtraction.colors
  };
}

function questionnaireColorPreviewStyle(colors: string[]): CSSProperties | undefined {
  if (colors.length === 0) {
    return undefined;
  }
  const [first, second = first, third = second, fourth = first] = colors;
  return {
    "--questionnaire-option-color-a": first,
    "--questionnaire-option-color-b": second,
    "--questionnaire-option-color-c": third,
    "--questionnaire-option-color-d": fourth
  } as CSSProperties;
}

function parsedQuestionnaireQuestion(value: string): ParsedQuestionnaireQuestion {
  const trimmed = value.trim();
  const colonIndex = trimmed.indexOf(":");
  if (colonIndex <= 0) {
    return { question: trimmed, inlineOptions: [] };
  }
  const head = trimmed.slice(0, colonIndex).trim();
  const tail = trimmed.slice(colonIndex + 1).trim();
  const inlineOptions = splitQuestionnaireOptions(tail, true);
  if (!head || inlineOptions.length < 2) {
    return { question: trimmed, inlineOptions: [] };
  }
  return {
    question: head.endsWith("?") ? head : `${head} ?`,
    inlineOptions
  };
}

function questionnaireOptionsFor(slots: Map<string, string>, questionKey: string, inlineOptions: string[] = []): [string, string, string] {
  const explicitOptions = splitQuestionnaireOptions(slots.get(`${questionKey}_options`) ?? "");
  const individualOptions = [
    slots.get(`${questionKey}_option1`) ?? slots.get(`${questionKey}_a`) ?? "",
    slots.get(`${questionKey}_option2`) ?? slots.get(`${questionKey}_b`) ?? "",
    slots.get(`${questionKey}_option3`) ?? slots.get(`${questionKey}_c`) ?? ""
  ].map((option) => option.trim()).filter(Boolean);
  const options = (explicitOptions.length > 0 ? explicitOptions : individualOptions.length > 0 ? individualOptions : inlineOptions).slice(0, 3);
  const fallbackOptions = [
    "Base \u00e9quilibr\u00e9e (Recommended) - bon point de d\u00e9part, complexit\u00e9 ma\u00eetris\u00e9e",
    "Version qualit\u00e9 sup\u00e9rieure - meilleur r\u00e9sultat, plus longue ou plus co\u00fbteuse",
    "Prototype rapide - valide l'id\u00e9e vite, avec moins de finition"
  ];
  while (options.length < 3) {
    options.push(fallbackOptions[options.length]);
  }
  return [options[0], options[1], options[2]];
}

function parseQuestionnaireFromMessage(message: TranscriptMessage): TranscriptQuestionnaire | null {
  if (message.role !== "assistant" || !message.text.includes(BRAIN_QUESTIONNAIRE_COMMAND)) {
    return null;
  }
  const lines = message.text.replace(/\r\n/g, "\n").split("\n");
  const commandIndex = lines.findIndex((line) => line.includes(BRAIN_QUESTIONNAIRE_COMMAND));
  if (commandIndex < 0) {
    return null;
  }
  const blockLines: string[] = [];
  for (let index = commandIndex; index < lines.length; index += 1) {
    const line = lines[index];
    if (index > commandIndex && (!line.trim() || isCodeActResultHeader(line))) {
      break;
    }
    blockLines.push(line);
  }
  const slots = readQuestionnaireSlots(blockLines.join("\n"));
  const numberedQuestions = [...slots.entries()]
    .filter(([name, value]) => /^q\d+$/.test(name) && value.trim().length > 0)
    .sort(([left], [right]) => Number(left.slice(1)) - Number(right.slice(1)))
    .map(([, value]) => value.trim());
  const joinedQuestions = (slots.get("questions") ?? "")
    .split(/\s+\|\s+|\s*;\s*/)
    .map((question) => question.trim())
    .filter(Boolean);
  const questions = numberedQuestions.length > 0 ? numberedQuestions : joinedQuestions;
  if (questions.length === 0) {
    return null;
  }
  const pages = questions.slice(0, 5).map((rawQuestion, index) => {
    const questionKey = `q${index + 1}`;
    const parsedQuestion = parsedQuestionnaireQuestion(rawQuestion);
    return {
      question: parsedQuestion.question,
      options: questionnaireOptionsFor(slots, questionKey, parsedQuestion.inlineOptions)
    };
  });
  return {
    title: slots.get("title")?.trim() || "",
    intro: slots.get("intro")?.trim() || "",
    pages,
    sourceMessageId: message.id
  };
}

function latestQuestionnaireFromMessages(messages: TranscriptMessage[]): TranscriptQuestionnaire | null {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index].role === "user") {
      return null;
    }
    const questionnaire = parseQuestionnaireFromMessage(messages[index]);
    if (questionnaire) {
      return questionnaire;
    }
  }
  return null;
}

function ComposerQuestionnaire({
  questionnaire,
  onCommitAnswers
}: {
  questionnaire: TranscriptQuestionnaire;
  onCommitAnswers: (text: string) => void;
}) {
  const titleId = useId();
  const promptId = useId();
  const [pageIndex, setPageIndex] = useState(0);
  const [leavingPageIndex, setLeavingPageIndex] = useState<number | null>(null);
  const [pageMotionDirection, setPageMotionDirection] = useState<"forward" | "back">("forward");
  const [isClosing, setIsClosing] = useState(false);
  const [answers, setAnswers] = useState<Record<number, QuestionnaireAnswer>>({});
  const pageTransitionTimerRef = useRef<number | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const page = questionnaire.pages[Math.min(pageIndex, Math.max(0, questionnaire.pages.length - 1))];
  const currentAnswer = answers[pageIndex];
  const isLastPage = pageIndex >= questionnaire.pages.length - 1;
  const questionnaireTitle = questionnaire.title.trim();
  const currentAnswerComplete = Boolean(
    currentAnswer?.kind === "option" ||
    (currentAnswer?.kind === "other" && currentAnswer.otherText.trim())
  );

  useEffect(() => {
    setPageIndex(0);
    setLeavingPageIndex(null);
    setPageMotionDirection("forward");
    setIsClosing(false);
    setAnswers({});
  }, [questionnaire.sourceMessageId]);

  useEffect(() => {
    return () => {
      if (pageTransitionTimerRef.current !== null) {
        window.clearTimeout(pageTransitionTimerRef.current);
      }
      if (closeTimerRef.current !== null) {
        window.clearTimeout(closeTimerRef.current);
      }
    };
  }, []);

  if (!page) {
    return null;
  }

  const goToPage = (nextPageIndex: number) => {
    const clampedNextPageIndex = Math.max(0, Math.min(questionnaire.pages.length - 1, nextPageIndex));
    if (clampedNextPageIndex === pageIndex || isClosing) {
      return;
    }
    if (pageTransitionTimerRef.current !== null) {
      window.clearTimeout(pageTransitionTimerRef.current);
    }
    setPageMotionDirection(clampedNextPageIndex > pageIndex ? "forward" : "back");
    setLeavingPageIndex(pageIndex);
    setPageIndex(clampedNextPageIndex);
    pageTransitionTimerRef.current = window.setTimeout(() => {
      setLeavingPageIndex(null);
      pageTransitionTimerRef.current = null;
    }, 260);
  };
  const commitAnswersWith = (nextAnswers: Record<number, QuestionnaireAnswer>) => {
    const lines = questionnaire.pages
      .map((candidate, index) => {
        const answer = nextAnswers[index];
        const value = answer?.kind === "other" ? answer.otherText.trim() : answer?.value.trim() ?? "";
        return value ? `- ${candidate.question}: ${value}` : "";
      })
      .filter(Boolean);
    if (lines.length === 0 || isClosing) {
      return;
    }
    setIsClosing(true);
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
    }
    closeTimerRef.current = window.setTimeout(() => {
      onCommitAnswers(`Questionnaire answers:\n${lines.join("\n")}`);
      closeTimerRef.current = null;
    }, 320);
  };
  const chooseOption = (value: string) => {
    setAnswers((current) => {
      const nextAnswers = {
        ...current,
        [pageIndex]: { kind: "option" as const, value, otherText: current[pageIndex]?.otherText ?? "" }
      };
      if (isLastPage) {
        commitAnswersWith(nextAnswers);
      } else {
        window.setTimeout(() => goToPage(pageIndex + 1), 120);
      }
      return nextAnswers;
    });
  };
  const chooseOther = (otherText = currentAnswer?.otherText ?? "") => {
    setAnswers((current) => ({
      ...current,
      [pageIndex]: { kind: "other", value: "Autre", otherText }
    }));
  };
  const commitAnswers = () => {
    commitAnswersWith(answers);
  };
  const renderQuestionnairePage = (
    candidatePage: TranscriptQuestionnairePage,
    candidatePageIndex: number,
    motionState: "active" | "leaving"
  ) => {
    const candidateAnswer = answers[candidatePageIndex];
    return (
      <div
        className={[
          "composerQuestionnaire__page",
          `composerQuestionnaire__page--${motionState}`,
          `composerQuestionnaire__page--${pageMotionDirection}`
        ].join(" ")}
        key={`${questionnaire.sourceMessageId}-${candidatePageIndex}-${motionState}`}
        role={motionState === "active" ? "group" : undefined}
        aria-labelledby={motionState === "active" ? promptId : undefined}
        aria-hidden={motionState === "leaving" ? "true" : undefined}
      >
        <div className="composerQuestionnaire__prompt">
          <span className="composerQuestionnaire__promptQuestion" id={motionState === "active" ? promptId : undefined}>
            {candidatePage.question}
          </span>
          {questionnaire.intro ? (
            <span className="composerQuestionnaire__promptHint">{questionnaire.intro}</span>
          ) : null}
        </div>
        <div className="composerQuestionnaire__options">
          {candidatePage.options.map((option, optionIndex) => {
            const optionCopy = questionnaireOptionCopy(option);
            return (
              <label
                className={[
                  "composerQuestionnaire__option",
                  optionCopy.colors.length > 0 ? "composerQuestionnaire__option--colorPreview" : ""
                ].filter(Boolean).join(" ")}
                key={`${questionnaire.sourceMessageId}-${candidatePageIndex}-${optionIndex}`}
                style={questionnaireColorPreviewStyle(optionCopy.colors)}
              >
                <input
                  checked={candidateAnswer?.kind === "option" && candidateAnswer.value === option}
                  disabled={motionState === "leaving" || isClosing}
                  name={`${questionnaire.sourceMessageId}-page-${candidatePageIndex}`}
                  type="radio"
                  value={option}
                  onChange={() => chooseOption(option)}
                />
                <span className="composerQuestionnaire__optionIndex" aria-hidden="true">{optionIndex + 1}</span>
                <span className="composerQuestionnaire__optionCopy">
                  <span className="composerQuestionnaire__optionLabel">
                    {optionCopy.label}
                    {optionCopy.tags.map((tag) => <em key={`${option}-${tag}`}>{tag}</em>)}
                  </span>
                  {optionCopy.detail ? <small>{optionCopy.detail}</small> : null}
                </span>
              </label>
            );
          })}
          <label className="composerQuestionnaire__option composerQuestionnaire__option--other">
            <input
              checked={candidateAnswer?.kind === "other"}
              disabled={motionState === "leaving" || isClosing}
              name={`${questionnaire.sourceMessageId}-page-${candidatePageIndex}`}
              type="radio"
              value="Autre"
              onChange={() => chooseOther()}
            />
            <span className="composerQuestionnaire__optionIndex" aria-hidden="true">4</span>
            <span className="composerQuestionnaire__optionCopy">
              <span className="composerQuestionnaire__optionLabel">Autre</span>
              <textarea
                className="composerQuestionnaire__otherInput"
                aria-label="Autre"
                disabled={motionState === "leaving" || isClosing}
                placeholder={"Pr\u00e9cise ta r\u00e9ponse..."}
                rows={2}
                value={candidateAnswer?.kind === "other" ? candidateAnswer.otherText : ""}
                onChange={(event) => chooseOther(event.currentTarget.value)}
                onClick={(event) => event.stopPropagation()}
                onFocus={() => chooseOther(candidateAnswer?.kind === "other" ? candidateAnswer.otherText : "")}
              />
            </span>
          </label>
        </div>
      </div>
    );
  };
  const leavingPage = leavingPageIndex === null ? null : questionnaire.pages[leavingPageIndex] ?? null;

  return (
    <section
      className={[
        "composerQuestionnaire",
        isClosing ? "composerQuestionnaire--closing" : "",
        questionnaireTitle ? "composerQuestionnaire--titled" : "composerQuestionnaire--untitled"
      ].filter(Boolean).join(" ")}
      aria-label={questionnaireTitle || "Questionnaire"}
      aria-labelledby={questionnaireTitle ? titleId : undefined}
      key={questionnaire.sourceMessageId}
      role="form"
    >
      <div className="composerQuestionnaire__header">
        {questionnaireTitle ? (
          <>
            <span className="composerQuestionnaire__dot" aria-hidden="true" />
            <strong id={titleId}>{questionnaireTitle}</strong>
          </>
        ) : (
          <span className="composerQuestionnaire__headerSpacer" aria-hidden="true" />
        )}
        <div className="composerQuestionnaire__pager" aria-label="Pagination du questionnaire">
          <button
            type="button"
            className="composerQuestionnaire__arrow"
            aria-label="Question précédente"
            title="Précédent"
            disabled={pageIndex === 0 || isClosing}
            onClick={() => goToPage(pageIndex - 1)}
          >
            <ChevronLeft aria-hidden="true" size={15} strokeWidth={1.9} />
          </button>
          <span className="composerQuestionnaire__progress">
            {pageIndex + 1}/{questionnaire.pages.length}
          </span>
          {isLastPage ? (
            <button
              type="button"
              className="composerQuestionnaire__arrow composerQuestionnaire__arrow--primary"
              aria-label="Envoyer les réponses"
              title="Envoyer"
              disabled={!currentAnswerComplete || isClosing}
              onClick={commitAnswers}
            >
              <ChevronRight aria-hidden="true" size={15} strokeWidth={1.9} />
            </button>
          ) : (
            <button
              type="button"
              className="composerQuestionnaire__arrow composerQuestionnaire__arrow--primary"
              aria-label="Question suivante"
              title="Suivant"
              disabled={!currentAnswerComplete || isClosing}
              onClick={() => goToPage(pageIndex + 1)}
            >
              <ChevronRight aria-hidden="true" size={15} strokeWidth={1.9} />
            </button>
          )}
        </div>
      </div>
      <div className="composerQuestionnaire__pageStage">
        {leavingPage && leavingPageIndex !== null ? renderQuestionnairePage(leavingPage, leavingPageIndex, "leaving") : null}
        {renderQuestionnairePage(page, pageIndex, "active")}
      </div>
    </section>
  );
}

function assistantMathLabel(value: string): string {
  return value
    .replace(/\\quad/g, "  ")
    .replace(/\\dots/g, "...")
    .replace(/\\cdot/g, "·")
    .replace(/\s+/g, " ")
    .trim();
}

type AssistantMathUseHandler = (formula: string) => void;

const TEXTUAL_MATH_ATOM_SOURCE = String.raw`(?:\d+(?:\.\d+)?|(?:sin|cos|tan|log|ln|sqrt)\([^()\n]{1,30}\)|[A-Za-z](?:_\{?[-+A-Za-z0-9]+\}?|\^\{?[-+A-Za-z0-9]+\}?|\([^()\n]{1,30}\))?)`;
const TEXTUAL_MATH_OPERATOR_SOURCE = String.raw`(?:\s*(?:[+\-*/^=,]|<=|>=|≤|≥|≈|≃|≠|!=)\s*|\s*[(){}\[\]]\s*)`;
const TEXTUAL_MATH_EQUATION_SOURCE = String.raw`\b${TEXTUAL_MATH_ATOM_SOURCE}\s*(?:=|≈|≃|≤|≥|!=|≠|<=|>=)\s*${TEXTUAL_MATH_ATOM_SOURCE}(?:${TEXTUAL_MATH_OPERATOR_SOURCE}${TEXTUAL_MATH_ATOM_SOURCE}){0,12}`;
const TEXTUAL_MATH_SEQUENCE_SOURCE = String.raw`\b\d+(?:\s*,\s*\d+){3,}\b`;
const TEXTUAL_MATH_PATTERN = new RegExp(`${TEXTUAL_MATH_SEQUENCE_SOURCE}|${TEXTUAL_MATH_EQUATION_SOURCE}`, "g");

function assistantWordFlashNodes(text: string, keyPrefix: string, writing: boolean): ReactNode[] {
  if (!writing) {
    return [text];
  }
  const nodes: ReactNode[] = [];
  const chunks = text.match(/\s+|\S+/g) ?? [];
  let offset = 0;
  for (const chunk of chunks) {
    const key = `${keyPrefix}-word-${offset}`;
    offset += chunk.length;
    if (/^\s+$/.test(chunk)) {
      nodes.push(chunk);
    } else {
      nodes.push(
        <span className="assistantText__wordFlash" key={key}>
          {chunk}
        </span>
      );
    }
  }
  return nodes;
}

function assistantMathTokenNode(formula: string, key: string, onUseMathInCompute?: AssistantMathUseHandler): ReactNode {
  return (
    <span className="assistantText__mathToken" key={key}>
      <span className="assistantText__mathPill">{assistantMathLabel(formula)}</span>
      <button
        type="button"
        className="assistantText__mathComputeHint"
        aria-label="Use this math in Compute"
        title="Use this math in Compute"
        onClick={() => onUseMathInCompute?.(formula)}
      >
        <span className="assistantText__mathComputeIcon" aria-hidden="true">
          <NewComputeCodeActIcon />
        </span>
        <span className="assistantText__mathComputeLabel">Use in Compute</span>
      </button>
    </span>
  );
}

const ASSISTANT_GEO_ENTITY_STYLE: CSSProperties = {
  appearance: "none",
  display: "inline",
  maxWidth: "100%",
  padding: "0 1px",
  border: 0,
  background: "transparent",
  color: "color-mix(in oklab, var(--assistant-mark-accent), var(--forge-text) 36%)",
  font: "inherit",
  fontWeight: 540,
  lineHeight: "inherit",
  cursor: "pointer",
  overflowWrap: "anywhere",
  transition: "color 160ms ease"
};

const ASSISTANT_COUNTRY_ENTITY_STYLE: CSSProperties = {
  ...ASSISTANT_GEO_ENTITY_STYLE,
  color: "color-mix(in oklab, #d8a657, var(--forge-text) 32%)"
};

function assistantGeoEntityNode(token: string, key: string): ReactNode {
  const label = assistantGeoEntityLabel(token);
  const style = token.startsWith("#{") ? ASSISTANT_COUNTRY_ENTITY_STYLE : ASSISTANT_GEO_ENTITY_STYLE;
  if (label.length < 2) {
    return <Fragment key={key}>{token}</Fragment>;
  }
  return (
    <button
      type="button"
      key={key}
      style={style}
      aria-label={`Open ${label}`}
      title={`Open ${label}`}
      onClick={() => {
        const api = window.forgeShell;
        if (api?.openGeoEntity) {
          void api.openGeoEntity(label).catch(() => undefined);
          return;
        }
        void api?.searchCitySuggestions?.(label).catch(() => undefined);
      }}
    >
      {label}
    </button>
  );
}

function assistantPlainTextNodes(text: string, keyPrefix: string, onUseMathInCompute?: AssistantMathUseHandler, writing = false): ReactNode[] {
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let match: RegExpExecArray | null;
  TEXTUAL_MATH_PATTERN.lastIndex = 0;
  while ((match = TEXTUAL_MATH_PATTERN.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(...assistantWordFlashNodes(text.slice(cursor, match.index), `${keyPrefix}-plain-${cursor}`, writing));
    }
    const formula = match[0].trim();
    nodes.push(assistantMathTokenNode(formula, `${keyPrefix}-text-math-${match.index}`, onUseMathInCompute));
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) {
    nodes.push(...assistantWordFlashNodes(text.slice(cursor), `${keyPrefix}-plain-${cursor}`, writing));
  }
  return nodes;
}

function assistantInlineNodes(text: string, keyPrefix: string, onUseMathInCompute?: AssistantMathUseHandler, writing = false): ReactNode[] {
  const nodes: ReactNode[] = [];
  const pattern = /(\\\[[\s\S]+?\\\]|\\\([\s\S]+?\\\)|[@#]\{[^{}\n]{1,120}\}|\*\*[^*]+?\*\*|`[^`]+?`)/g;
  let cursor = 0;
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(...assistantPlainTextNodes(text.slice(cursor, match.index), `${keyPrefix}-plain-${cursor}`, onUseMathInCompute, writing));
    }
    const token = match[0];
    if (token.startsWith("\\[") || token.startsWith("\\(")) {
      const formula = token.slice(2, -2).trim();
      nodes.push(assistantMathTokenNode(formula, `${keyPrefix}-math-${match.index}`, onUseMathInCompute));
    } else if (token.startsWith("@{") || token.startsWith("#{")) {
      nodes.push(assistantGeoEntityNode(token, `${keyPrefix}-geo-${match.index}`));
    } else if (token.startsWith("**")) {
      nodes.push(<strong key={`${keyPrefix}-strong-${match.index}`}>{token.slice(2, -2)}</strong>);
    } else {
      nodes.push(<code key={`${keyPrefix}-code-${match.index}`}>{token.slice(1, -1)}</code>);
    }
    cursor = match.index + token.length;
  }
  if (cursor < text.length) {
    nodes.push(...assistantPlainTextNodes(text.slice(cursor), `${keyPrefix}-plain-${cursor}`, onUseMathInCompute, writing));
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

function BrainSegmentCodeActIcon({ phase }: { phase: "changing" | "changed" }) {
  return (
    <svg className={`brainSegmentIcon brainSegmentIcon--${phase}`} viewBox="0 0 24 24" aria-hidden="true">
      <path className="brainSegmentIcon__stem" d="M12 18V5" />
      <path className="brainSegmentIcon__side" d="M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
      <path className="brainSegmentIcon__top" d="M12 5A3 3 0 1 1 17.598 6.5" />
      <path className="brainSegmentIcon__top" d="M12 5A3 3 0 1 0 6.402 6.5" />
      <path d="M17.997 5.125a4 4 0 0 1 2.526 5.77" />
      <path className="brainSegmentIcon__low" d="M18 18a4 4 0 0 0 2-7.464" />
      <path d="M19.967 17.483A4 4 0 1 1 12 18a4 4 0 1 1-7.967-.517" />
      <path className="brainSegmentIcon__low" d="M6 18a4 4 0 0 1-2-7.464" />
      <path d="M6.003 5.125a4 4 0 0 0-2.526 5.77" />
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

function AgentActionCodeActIcon({ command }: { command: AgentActionEventCommand }) {
  if (command === AGENT_LIST_COMMAND) return <List />;
  if (command === AGENT_SEARCH_COMMAND) return <Search />;
  if (command === AGENT_CREATE_DIRECTORY_COMMAND) return <FolderPlus />;
  if (command === AGENT_RENAME_PATH_COMMAND) return <Pencil />;
  if (command === AGENT_MOVE_PATH_COMMAND) return <MoveRight />;
  if (command === AGENT_COPY_PATH_COMMAND) return <Copy />;
  if (command === AGENT_DELETE_EMPTY_DIRECTORY_COMMAND || command === AGENT_DELETE_TREE_COMMAND) return <Trash2 />;
  if (command === AGENT_READONLY_SHELL_COMMAND || command === AGENT_SHELL_COMMAND) return <Terminal />;
  return <GenericCodeActIcon />;
}

function isAgentActionCommand(command: TranscriptCodeActCommand): command is AgentActionEventCommand {
  return Boolean(agentActionEventCommandFromToken(command));
}

function agentActionTone(command: AgentActionEventCommand): "read" | "write" | "destructive" | "shell" {
  if (command === AGENT_DELETE_EMPTY_DIRECTORY_COMMAND || command === AGENT_DELETE_TREE_COMMAND) return "destructive";
  if (command === AGENT_READONLY_SHELL_COMMAND || command === AGENT_SHELL_COMMAND) return "shell";
  if (command === AGENT_LIST_COMMAND || command === AGENT_SEARCH_COMMAND) return "read";
  return "write";
}

function CodeActEventIcon({ command, brainSegmentPhase = "changed" }: { command: TranscriptCodeActCommand; brainSegmentPhase?: "changing" | "changed" }) {
  if (isAgentActionCommand(command)) return <AgentActionCodeActIcon command={command} />;
  if (command === BRAIN_GMAIL_COMMAND || command === BRAIN_GMAIL_COM_COMMAND) return <ModuleLogo id="gmail" />;
  if (command === BRAIN_AIRBNB_COMMAND) return <ModuleLogo id="airbnb" />;
  if (command === BRAIN_SCIENCE_COMMAND || command === BRAIN_CODING_COMMAND) return <BrainSegmentCodeActIcon phase={brainSegmentPhase} />;
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

function isContextCompactionCommand(command: TranscriptCodeActCommand): command is typeof CONTEXT_COMPACTION_COMMAND {
  return command === CONTEXT_COMPACTION_COMMAND;
}

function brainSegmentName(command: TranscriptCodeActCommand): string {
  return command === BRAIN_CODING_COMMAND ? "Coding Brain" : "Science Brain";
}

function brainSegmentEventText(command: TranscriptCodeActCommand, phase: "changing" | "changed"): string {
  const target = brainSegmentName(command);
  return phase === "changing"
    ? `Changing from General Brain to ${target}`
    : `Changed from General Brain to ${target}`;
}

function activeAgentEventText(agentName: string, command: TranscriptCodeActCommand): string {
  const label = agentName.trim() || "Agent";
  return `${label} is using ${command}`;
}

interface AgentFileModificationSummary {
  fileName: string;
  path: string;
  addedChars: number;
  removedChars: number;
}

function isAgentFileModificationCommand(command: AgentActionEventCommand): boolean {
  return command === AGENT_COPY_PATH_COMMAND;
}

function fileNameFromEventPath(path: string): string {
  return path.replace(/[\\/]+$/g, "").split(/[\\/]/).filter(Boolean).at(-1) || path || "file";
}

function eventResultPath(detail?: string): string | undefined {
  if (!detail) {
    return undefined;
  }
  const normalized = detail.replace(/^R(?:esult|esultat):\s*/i, "").replace(/\s+chars\s+\+\d+\s+-\d+\.?$/i, "").trim();
  const moved = /^(.+?)\s*->\s*(.+?)\.?$/.exec(normalized);
  if (moved) {
    return moved[2].replace(/\.$/, "").trim();
  }
  const applied = /^action applied on\s+(.+?)\.?$/i.exec(normalized) ?? /^action appliquee sur\s+(.+?)\.?$/i.exec(normalized);
  if (applied) {
    return applied[1].replace(/\.$/, "").trim();
  }
  return undefined;
}

function eventModificationDelta(detail?: string): { addedChars: number; removedChars: number } | undefined {
  const match = /chars\s+\+(\d+)\s+-(\d+)/i.exec(detail ?? "");
  if (!match) {
    return undefined;
  }
  const delta = {
    addedChars: Number(match[1]),
    removedChars: Number(match[2])
  };
  return delta.addedChars > 0 || delta.removedChars > 0 ? delta : undefined;
}

function agentFileModificationSummary(event: TranscriptCodeActEvent, command: AgentActionEventCommand): AgentFileModificationSummary | undefined {
  if (!isAgentFileModificationCommand(command)) {
    return undefined;
  }
  const path = event.toPath ?? eventResultPath(event.detail) ?? event.path;
  if (!path) {
    return undefined;
  }
  const delta = eventModificationDelta(event.detail);
  if (!delta) {
    return undefined;
  }
  return {
    fileName: fileNameFromEventPath(path),
    path,
    ...delta
  };
}

function AnimatedModificationCounter({ addedChars, removedChars }: { addedChars: number; removedChars: number }) {
  const [display, setDisplay] = useState({ addedChars, removedChars });

  useEffect(() => {
    if (prefersReducedMotion()) {
      setDisplay({ addedChars, removedChars });
      return;
    }
    const start = performance.now();
    const from = display;
    let frame = 0;
    const tick = (now: number) => {
      const progress = Math.min(1, (now - start) / 320);
      const eased = 1 - Math.pow(1 - progress, 3);
      setDisplay({
        addedChars: Math.round(from.addedChars + (addedChars - from.addedChars) * eased),
        removedChars: Math.round(from.removedChars + (removedChars - from.removedChars) * eased)
      });
      if (progress < 1) {
        frame = requestAnimationFrame(tick);
      }
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [addedChars, removedChars]);

  return (
    <span className="transcriptCodeActFileEvent__counter" aria-label={`${addedChars} characters added, ${removedChars} characters removed`}>
      <span className="transcriptCodeActFileEvent__added">+{display.addedChars}</span>
      <span className="transcriptCodeActFileEvent__removed">-{display.removedChars}</span>
    </span>
  );
}

function TranscriptContextCompactionEventLine({ event }: { event: TranscriptCodeActEvent }) {
  const state = event.compactionState ?? "compressing";
  const complete = state === "compressed";
  return (
    <div className={`transcriptContextCompactionEvent transcriptContextCompactionEvent--${state}`} role="status" aria-live={complete ? "off" : "polite"}>
      <span className="transcriptContextCompactionEvent__line" aria-hidden="true" />
      <span className="transcriptContextCompactionEvent__text">{complete ? "Context compressed" : "Compressing context"}</span>
      <span className="transcriptContextCompactionEvent__line" aria-hidden="true" />
    </div>
  );
}

function TranscriptCodeActEventLine({ agentName, event, writing }: { agentName: string; event: TranscriptCodeActEvent; writing: boolean }) {
  if (isContextCompactionCommand(event.command)) {
    return <TranscriptContextCompactionEventLine event={event} />;
  }
  const isBrainSegment = isBrainSegmentCommand(event.command);
  const agentCommand = isAgentActionCommand(event.command) ? event.command : undefined;
  const isPendingAgentEvent = writing && Boolean(agentCommand) && !event.detail;
  const fileModification = agentCommand ? agentFileModificationSummary(event, agentCommand) : undefined;
  const [brainSegmentPhase, setBrainSegmentPhase] = useState<"changing" | "changed">(isBrainSegment ? "changing" : "changed");

  useEffect(() => {
    if (!isBrainSegment) {
      return;
    }
    setBrainSegmentPhase("changing");
    const timeout = window.setTimeout(() => setBrainSegmentPhase("changed"), 1000);
    return () => window.clearTimeout(timeout);
  }, [event.command, isBrainSegment]);

  const eventClassName = isBrainSegment
    ? `transcriptCodeActEvent transcriptCodeActEvent--brainSegment transcriptCodeActEvent--brainSegment-${brainSegmentPhase}`
    : agentCommand
      ? `transcriptCodeActEvent transcriptCodeActEvent--agent transcriptCodeActEvent--agent-${agentActionTone(agentCommand)}${fileModification ? " transcriptCodeActEvent--fileModification" : ""}${isPendingAgentEvent ? " transcriptCodeActEvent--agent-pending" : " transcriptCodeActEvent--agent-complete"}`
      : "transcriptCodeActEvent";
  const text = isBrainSegment
    ? brainSegmentEventText(event.command, brainSegmentPhase)
    : isPendingAgentEvent
      ? activeAgentEventText(agentName, event.command)
      : event.text;

  if (fileModification) {
    return (
      <div className={eventClassName}>
        <code className="transcriptCodeActEvent__command">{event.command}</code>
        <span className="transcriptCodeActEvent__icon transcriptCodeActFileEvent__icon" aria-hidden="true">
          <Pencil size={13} strokeWidth={1.85} />
        </span>
        <span className="transcriptCodeActFileEvent__label">Modified</span>
        <span className="transcriptCodeActFileEvent__fileCard" title={fileModification.path}>
          {fileModification.fileName}
        </span>
        <AnimatedModificationCounter addedChars={fileModification.addedChars} removedChars={fileModification.removedChars} />
      </div>
    );
  }

  return (
    <div className={eventClassName}>
      <code className="transcriptCodeActEvent__command">{event.command}</code>
      <span className="transcriptCodeActEvent__icon" aria-hidden="true">
        <CodeActEventIcon command={event.command} brainSegmentPhase={brainSegmentPhase} />
      </span>
      <span className="transcriptCodeActEvent__text">{text}</span>
    </div>
  );
}

function TranscriptCommandSummaryLine({ events }: { events: TranscriptCodeActEvent[] }) {
  const [expanded, setExpanded] = useState(false);
  const treeId = useId();
  const count = events.length;
  const treeHeight = Math.max(24, count * 28);
  return (
    <div className="transcriptCommandSummary">
      <button
        aria-controls={treeId}
        aria-expanded={expanded}
        className="transcriptCommandSummaryLine"
        onClick={() => setExpanded((value) => !value)}
        type="button"
      >
        <span className="transcriptCommandSummaryLine__icon" aria-hidden="true">
          <svg viewBox="0 0 16 16">
            <rect x="2.5" y="2.5" width="11" height="11" rx="2" />
            <path d="m5.2 6.1 2.1 1.9-2.1 1.9" />
            <path d="M8.7 10h2.4" />
          </svg>
        </span>
        <span>{count} {count > 1 ? "commands executed" : "command executed"}</span>
        <span className="transcriptCommandSummaryLine__chevron" aria-hidden="true">
          <svg viewBox="0 0 16 16">
            <path d="m4.5 6.5 3.5 3 3.5-3" />
          </svg>
        </span>
      </button>
      {expanded ? (
        <div
          aria-label="Executed command tree"
          className="transcriptCommandTree"
          id={treeId}
          style={{ "--transcript-command-tree-height": `${treeHeight}px` } as CSSProperties}
        >
          <svg className="transcriptCommandTree__rail" viewBox={`0 0 33 ${treeHeight}`} preserveAspectRatio="none" aria-hidden="true">
            <path className="transcriptCommandTree__trunk" d={`M1 0 V${treeHeight}`} />
            {events.map((_, index) => {
              const y = 12 + index * 28;
              return <path className="transcriptCommandTree__branch" d={`M1 ${y} H32`} key={index} />;
            })}
          </svg>
          <div className="transcriptCommandTree__rows">
            {events.map((event, index) => (
              <div className="transcriptCommandTree__row" key={`${event.command}-${index}`}>
                <code>{event.command}</code>
                <span>
                  <span className="transcriptCommandTree__eventText">{event.text}</span>
                  {event.detail ? <span className="transcriptCommandTree__detail">{event.detail}</span> : null}
                </span>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function AssistantMarkdownText({
  agentName,
  text,
  messageId,
  writing,
  onUseMathInCompute
}: {
  agentName: string;
  text: string;
  messageId: string;
  writing: boolean;
  onUseMathInCompute?: AssistantMathUseHandler;
}) {
  const blocks = assistantMarkdownBlocks(text);
  return (
    <div className="assistantText__body">
      {blocks.map((block, index) => {
        if (block.kind === "event_group") {
          return <TranscriptCommandSummaryLine events={block.events} key={`${messageId}-event-group-${index}`} />;
        }
        if (block.kind === "event") {
          return <TranscriptCodeActEventLine agentName={agentName} event={block.event} key={`${messageId}-event-${index}-${block.event.command}`} writing={writing} />;
        }
        if (block.kind === "heading") {
          const Tag = block.level <= 2 ? "h3" : "h4";
          return <Tag className="assistantText__heading" key={`${messageId}-heading-${index}`}>{assistantInlineNodes(block.text, `${messageId}-heading-${index}`, onUseMathInCompute, writing)}</Tag>;
        }
        if (block.kind === "list") {
          const Tag = block.ordered ? "ol" : "ul";
          const macroItems = block.ordered ? null : assistantMacroListItems(block.items);
          if (macroItems) {
            return (
              <ul className="assistantText__list assistantText__list--macro" key={`${messageId}-list-${index}`}>
                {macroItems.map((item, itemIndex) => (
                  <li key={`${messageId}-list-${index}-${itemIndex}`}>
                    <span className="assistantText__macroLabel">{assistantInlineNodes(item.label, `${messageId}-list-${index}-${itemIndex}-label`, onUseMathInCompute, writing)}</span>
                    <span className="assistantText__macroBody">{assistantInlineNodes(item.body, `${messageId}-list-${index}-${itemIndex}-body`, onUseMathInCompute, writing)}</span>
                  </li>
                ))}
              </ul>
            );
          }
          return (
            <Tag className="assistantText__list" key={`${messageId}-list-${index}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${messageId}-list-${index}-${itemIndex}`}>{assistantInlineNodes(item, `${messageId}-list-${index}-${itemIndex}`, onUseMathInCompute, writing)}</li>
              ))}
            </Tag>
          );
        }
        if (block.kind === "task_list") {
          return (
            <ul className="assistantText__taskList" key={`${messageId}-task-list-${index}`}>
              {block.items.map((item, itemIndex) => (
                <li className={item.checked ? "assistantText__taskItem assistantText__taskItem--checked" : "assistantText__taskItem"} key={`${messageId}-task-${index}-${itemIndex}`}>
                  <AssistantTaskCheck checked={item.checked} />
                  <span className="assistantText__taskText">{assistantInlineNodes(item.text, `${messageId}-task-${index}-${itemIndex}`, onUseMathInCompute, writing)}</span>
                </li>
              ))}
            </ul>
          );
        }
        if (block.kind === "table") {
          return (
            <div className="assistantText__tableWrap" key={`${messageId}-table-${index}`}>
              <table className="assistantText__table">
                <thead>
                  <tr>
                    {block.headers.map((header, cellIndex) => (
                      <th
                        className={`assistantText__tableCell assistantText__tableCell--${block.alignments[cellIndex] ?? "left"}`}
                        key={`${messageId}-table-${index}-header-${cellIndex}`}
                        scope="col"
                      >
                        {assistantInlineNodes(header, `${messageId}-table-${index}-header-${cellIndex}`, onUseMathInCompute, writing)}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {block.rows.map((row, rowIndex) => (
                    <tr key={`${messageId}-table-${index}-row-${rowIndex}`}>
                      {row.map((cell, cellIndex) => (
                        <td
                          className={`assistantText__tableCell assistantText__tableCell--${block.alignments[cellIndex] ?? "left"}`}
                          key={`${messageId}-table-${index}-row-${rowIndex}-cell-${cellIndex}`}
                        >
                          {assistantInlineNodes(cell, `${messageId}-table-${index}-row-${rowIndex}-cell-${cellIndex}`, onUseMathInCompute, writing)}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          );
        }
        if (block.kind === "code") {
          return (
            <figure className="assistantText__codeBlock" key={`${messageId}-code-${index}`}>
              <figcaption className="assistantText__codeHeader">
                <span className="assistantText__codeLanguage">{block.language || "code"}</span>
              </figcaption>
              <pre><code>{block.code}</code></pre>
            </figure>
          );
        }
        if (block.kind === "quote") {
          return (
            <blockquote className="assistantText__quote" key={`${messageId}-quote-${index}`}>
              {block.text.split("\n").map((line, lineIndex) => (
                <p key={`${messageId}-quote-${index}-${lineIndex}`}>{assistantInlineNodes(line, `${messageId}-quote-${index}-${lineIndex}`, onUseMathInCompute, writing)}</p>
              ))}
            </blockquote>
          );
        }
        if (block.kind === "callout") {
          return (
            <aside className={`assistantText__callout assistantText__callout--${block.tone}`} key={`${messageId}-callout-${index}`} role="note">
              <strong>{block.title}</strong>
              {block.body ? <p>{assistantInlineNodes(block.body, `${messageId}-callout-${index}`, onUseMathInCompute, writing)}</p> : null}
            </aside>
          );
        }
        if (block.kind === "facts") {
          return (
            <dl className="assistantText__factGrid" key={`${messageId}-facts-${index}`}>
              {block.items.map((item, itemIndex) => (
                <div className="assistantText__fact" key={`${messageId}-fact-${index}-${itemIndex}`}>
                  <dt>{item.label}</dt>
                  <dd>{assistantInlineNodes(item.value, `${messageId}-fact-${index}-${itemIndex}`, onUseMathInCompute, writing)}</dd>
                </div>
              ))}
            </dl>
          );
        }
        if (block.kind === "divider") {
          return <hr className="assistantText__divider" key={`${messageId}-divider-${index}`} />;
        }
        return <p className="assistantText__paragraph" key={`${messageId}-paragraph-${index}`}>{assistantInlineNodes(block.text, `${messageId}-paragraph-${index}`, onUseMathInCompute, writing)}</p>;
      })}
      {writing ? <span className="assistantText__caret" /> : null}
    </div>
  );
}

function StaticAssistantText({ agentName, message, onUseMathInCompute }: { agentName: string; message: TranscriptMessage; onUseMathInCompute?: AssistantMathUseHandler }) {
  const renderableText = useMemo(() => assistantRenderableText(message.text), [message.text]);
  return (
    <div className="assistantText" aria-label={renderableText}>
      <AssistantMarkdownText agentName={agentName} messageId={message.id} text={renderableText} writing={false} onUseMathInCompute={onUseMathInCompute} />
    </div>
  );
}

function AnimatedAssistantText({
  agentName,
  message,
  onAnimationComplete,
  onUseMathInCompute
}: {
  agentName: string;
  message: TranscriptMessage;
  onAnimationComplete?: (messageId: string) => void;
  onUseMathInCompute?: AssistantMathUseHandler;
}) {
  const textRef = useRef<HTMLDivElement>(null);
  const renderableText = useMemo(() => assistantRenderableText(message.text), [message.text]);
  const animationSource = useMemo(() => assistantVisibleAnimationSource(renderableText), [renderableText]);
  const revealBreakpoints = useMemo(() => assistantRevealBreakpoints(animationSource), [animationSource]);
  const totalCharacters = animationSource.length;
  const totalRevealSteps = revealBreakpoints.length;
  const [visibleCharacters, setVisibleCharacters] = useState(0);
  const [animationSettled, setAnimationSettled] = useState(false);
  const completionReportedRef = useRef(false);
  const visibleCharactersRef = useRef(0);

  useEffect(() => {
    completionReportedRef.current = false;
    setAnimationSettled(false);
    visibleCharactersRef.current = 0;
    setVisibleCharacters(0);
  }, [message.id]);

  useEffect(() => {
    if (totalCharacters === 0 || totalRevealSteps === 0 || prefersReducedMotion()) {
      visibleCharactersRef.current = totalCharacters;
      setVisibleCharacters(totalCharacters);
      setAnimationSettled(true);
      return;
    }

    let frame = 0;
    let lastTickAt = performance.now();

    const tick = (now: number) => {
      const elapsed = now - lastTickAt;
      const current = visibleCharactersRef.current;
      const cadence = totalCharacters - current > 900 ? 62 : 78;
      if (elapsed < cadence) {
        frame = requestAnimationFrame(tick);
        return;
      }
      lastTickAt = now;
      const currentStep = revealBreakpoints.findIndex((point) => point > current);
      const nextIndex = currentStep === -1 ? totalRevealSteps - 1 : currentStep;
      const stepBudget = Math.max(1, Math.min(2, Math.floor(elapsed / cadence)));
      const targetIndex = Math.min(totalRevealSteps - 1, nextIndex + stepBudget - 1);
      const nextVisibleCharacters = Math.max(current + 1, revealBreakpoints[targetIndex] ?? totalCharacters);
      if (nextVisibleCharacters !== current) {
        visibleCharactersRef.current = nextVisibleCharacters;
        setVisibleCharacters(nextVisibleCharacters);
      }
      if (nextVisibleCharacters < totalCharacters) {
        frame = requestAnimationFrame(tick);
      } else {
        setAnimationSettled(true);
      }
    };

    if (visibleCharactersRef.current >= totalCharacters) {
      setVisibleCharacters(totalCharacters);
      setAnimationSettled(true);
      return;
    }

    setAnimationSettled(false);
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [animationSource, message.id, revealBreakpoints, totalCharacters, totalRevealSteps]);

  useEffect(() => {
    if (completionReportedRef.current || totalCharacters === 0 || !animationSettled || visibleCharacters < totalCharacters) {
      return;
    }
    completionReportedRef.current = true;
    sidebarShadowStore.finishChatSessionPreview();
    onAnimationComplete?.(message.id);
    void panelsChatBottomStore.dispatch({ kind: "assistant_write_complete", value: message.id });
  }, [animationSettled, message.id, onAnimationComplete, totalCharacters, visibleCharacters]);

  useEffect(() => {
    followTranscriptLatest(latestTranscriptContainerFor(textRef.current));
  }, [visibleCharacters]);

  const writing = visibleCharacters < totalCharacters;
  const visibleText = animationSource.slice(0, visibleCharacters);

  return (
    <div className="assistantText" aria-label={renderableText} ref={textRef}>
      <AssistantMarkdownText agentName={agentName} messageId={message.id} text={visibleText} writing={writing} onUseMathInCompute={onUseMathInCompute} />
    </div>
  );
}

function PendingAssistantText({ agentName }: { agentName: string }) {
  const label = agentName.trim() || "Agent";
  return (
    <div className="assistantText assistantText--pending assistantThinkingEvent" aria-label={`${label} is thinking`} role="status">
      <span className="sessionRow__loaderViewbox assistantThinkingEvent__loaderViewbox" aria-hidden="true">
        <span className="loader" />
      </span>
      <span className="assistantThinkingEvent__label">
        <strong>{label}</strong> is thinking
      </span>
    </div>
  );
}

function ProviderUnavailableIcon() {
  return (
    <svg className="assistantErrorIcon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" aria-hidden="true">
      <circle cx="10" cy="16" r="2" fill="currentColor" />
      <path fill="currentColor" d="M16.4 11.6A7.1 7.1 0 0 0 12 9.1l3.4 3.4zM19 8.4A12.2 14 0 0 0 8.2 4.2L10 6a9.9 9.9 0 0 1 7.4 3.7zM3.5 2L2 3.4l2.2 2.2A13.1 13.1 0 0 0 1 8.4l1.5 1.3a10.7 10.7 0 0 1 3.2-2.6L8 9.3a7.3 7.3 0 0 0-3.3 2.3L6.1 13a5.2 5.2 0 0 1 3.6-2l6.8 7l1.5-1.5z" />
    </svg>
  );
}

function AssistantErrorText({ message }: { message: TranscriptMessage }) {
  return (
    <div className="assistantErrorText" role="status" aria-label={message.text}>
      <ProviderUnavailableIcon />
      <span>{message.text}</span>
    </div>
  );
}

function TranscriptCanvas({
  activeSessionId,
  messages,
  agentName,
  parallelSessionIndex = 0,
  className = "chatCanvas",
  onEditImage,
  onUseMathInCompute
}: {
  activeSessionId: string;
  messages: TranscriptMessage[];
  agentName: string;
  parallelSessionIndex?: number;
  className?: string;
  onEditImage?: (preview: ComposerUploadPreview) => void;
  onUseMathInCompute?: AssistantMathUseHandler;
}) {
  const storageKey = useMemo(() => pinsStorageKey(activeSessionId), [activeSessionId]);
  const [pins, setPins] = useState<PinnedChapter[]>(() => loadPins(storageKey));
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const messagesRef = useRef<HTMLDivElement>(null);
  const assistantAnimationRef = useRef<{ sessionId: string; known: Set<string>; active: Set<string>; queue: string[]; hadPending: boolean } | null>(null);
  const [, setAssistantAnimationQueueVersion] = useState(0);
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
  let hadPendingBeforeRender = assistantAnimationRef.current?.hadPending ?? false;

  if (assistantAnimationRef.current === null) {
    assistantAnimationRef.current = {
      sessionId: activeSessionId,
      known: new Set(assistantMessageIds),
      active: new Set(),
      queue: [],
      hadPending: hasPendingAssistant
    };
    hadPendingBeforeRender = false;
  } else if (assistantAnimationRef.current.sessionId !== activeSessionId) {
    const previous = assistantAnimationRef.current;
    const keepDraftResponseLive = previous.hadPending;
    hadPendingBeforeRender = keepDraftResponseLive ? previous.hadPending : false;
    assistantAnimationRef.current = {
      sessionId: activeSessionId,
      known: keepDraftResponseLive ? previous.known : new Set(assistantMessageIds),
      active: keepDraftResponseLive ? previous.active : new Set(),
      queue: keepDraftResponseLive ? previous.queue.filter((id) => previous.active.has(id)) : [],
      hadPending: hasPendingAssistant
    };
  } else {
    hadPendingBeforeRender = assistantAnimationRef.current.hadPending;
    assistantAnimationRef.current.hadPending = hasPendingAssistant;
  }
  if (assistantAnimationRef.current) {
    assistantAnimationRef.current.queue = assistantAnimationRef.current.queue.filter((id) => messageIds.has(id));
    for (const id of [...assistantAnimationRef.current.active]) {
      if (!messageIds.has(id)) {
        assistantAnimationRef.current.active.delete(id);
      }
    }
  }
  const pendingResolvedThisRender = hadPendingBeforeRender && !hasPendingAssistant;

  const completeAssistantAnimation = useCallback((messageId: string) => {
    const animationState = assistantAnimationRef.current;
    if (!animationState) {
      return;
    }
    animationState.active.delete(messageId);
    animationState.queue = animationState.queue.filter((id) => id !== messageId);
    setAssistantAnimationQueueVersion((version) => version + 1);
  }, []);

  useEffect(() => {
    setPins(loadPins(storageKey));
  }, [storageKey]);

  useEffect(() => {
    savePins(storageKey, visiblePins);
  }, [storageKey, visiblePins]);

  useEffect(() => {
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
    void copyTextToClipboard(message.text).then((copied) => {
      if (!copied) {
        return;
      }
      setCopiedId(message.id);
      globalThis.setTimeout(() => setCopiedId((id) => (id === message.id ? null : id)), 1200);
    });
  };

  const requestDifferentAnswer = (message: TranscriptMessage, messageIndex: number) => {
    const previousUserMessage = messages
      .slice(0, messageIndex)
      .reverse()
      .find((candidate) => candidate.role === "user" && candidate.text.trim().length > 0);
    const regeneratePrompt = "Generate a different answer to the previous user request.";
    if (!previousUserMessage) {
      return;
    }
    void panelsChatBottomStore.dispatch({
      kind: "send_chat",
      value: regeneratePrompt,
      internalPrompt: true,
      replaceAssistantMessageId: message.id,
      parallelSessionIndex
    });
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
          const role = message.role === "user" ? "user" : "assistant";
          const assistantPending = role === "assistant" && message.id.startsWith("assistant-pending-");
          if (!assistantPending && !message.text.trim() && attachments.length === 0) {
            return null;
          }
          const previousMessage = messages[index - 1];
          const followsVisualUserMessage =
            message.role !== "user" &&
            previousMessage?.role === "user" &&
            (previousMessage.attachments ?? []).some(isTranscriptVisualAttachment);
          const followsAssistantMessage = message.role !== "user" && previousMessage?.role === "assistant";
          const pinned = pinnedIds.has(message.id);
          const assistantError = role === "assistant" && message.id.startsWith("assistant-error-");
          const assistantCanAnimate = role === "assistant" && !assistantPending && !assistantError;
          let assistantShouldAnimate = false;
          let assistantQueued = false;
          if (assistantCanAnimate && assistantAnimationRef.current) {
            const animationState = assistantAnimationRef.current;
            if (animationState.active.has(message.id)) {
              assistantShouldAnimate = animationState.queue[0] === message.id;
              assistantQueued = !assistantShouldAnimate;
            } else if (!animationState.known.has(message.id) && (message.id === latestAssistantMessageId || pendingResolvedThisRender)) {
              animationState.known.add(message.id);
              animationState.active.add(message.id);
              if (!animationState.queue.includes(message.id)) {
                animationState.queue.push(message.id);
              }
              assistantShouldAnimate = animationState.queue[0] === message.id;
              assistantQueued = !assistantShouldAnimate;
            } else {
              animationState.known.add(message.id);
            }
          }
          const assistantAwaitingAnimation = assistantPending || assistantQueued;
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
              {role === "assistant" && !assistantPending && !assistantError ? (
                <button
                  type="button"
                  className="transcriptActionBtn"
                  aria-label="Generate a different answer"
                  title="Different answer"
                  onClick={() => requestDifferentAnswer(message, index)}
                >
                  <RefreshCw aria-hidden="true" size={13} strokeWidth={1.8} />
                </button>
              ) : null}
            </div>
          );
          const item = (
            <div
              className={`transcriptItem transcriptItem--${role}${followsVisualUserMessage ? " transcriptItem--afterVisualMedia" : ""}${followsAssistantMessage ? " transcriptItem--assistantLoop" : ""}`}
              data-msg-id={message.id}
              key={message.id}
            >
              {role === "assistant" ? (
                <div className="transcriptTextFrame">
                  <div
                    className={
                      assistantPending
                        ? "transcriptPill transcriptPill--assistant transcriptPill--assistantPending"
                        : assistantQueued
                          ? "transcriptPill transcriptPill--assistant transcriptPill--assistantPending"
                          : assistantError
                          ? "transcriptPill transcriptPill--assistant transcriptPill--assistantError"
                          : "transcriptPill transcriptPill--assistant"
                    }
                  >
                    {assistantAwaitingAnimation ? (
                      <PendingAssistantText agentName={agentName} />
                    ) : assistantError ? (
                      <AssistantErrorText message={renderedMessage} />
                    ) : assistantShouldAnimate ? (
                      <AnimatedAssistantText agentName={agentName} message={renderedMessage} onAnimationComplete={completeAssistantAnimation} onUseMathInCompute={onUseMathInCompute} />
                    ) : (
                      <StaticAssistantText agentName={agentName} message={renderedMessage} onUseMathInCompute={onUseMathInCompute} />
                    )}
                  </div>
                  {assistantAwaitingAnimation ? null : actions}
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
  composerOnly?: boolean;
  parallelPrompts?: string[];
  onParallelPromptChange?: (index: number, value: string) => void;
  webExplorerOpen?: boolean;
  composerModule?: SidebarModuleId | null;
  onComposerModuleChange?: (id: SidebarModuleId | null) => void;
  widgetMode?: boolean;
  widgetModeTransitioning?: boolean;
  onWidgetModeChange?: (enabled: boolean) => void;
}

type PermissionMode = PanelsChatBottomSnapshot["composer"]["permissionMode"];

const PERMISSION_MODE_CHOICES: Array<{
  value: PermissionMode;
  label: string;
  detail: string;
  shortLabel: string;
}> = [
  {
    value: "ask-permissions",
    label: "Ask Permission",
    detail: "Approval required before tools run.",
    shortLabel: "Ask"
  },
  {
    value: "auto-accept-edits",
    label: "Auto Edit",
    detail: "Can edit files; asks for risky actions.",
    shortLabel: "Edit"
  },
  {
    value: "full-autonomy",
    label: "Autonomous",
    detail: "Can browse web and read local files.",
    shortLabel: "Auto"
  },
  {
    value: "self-directed",
    label: "Self-Directed",
    detail: "Creates its own goals and tasks.",
    shortLabel: "Self"
  }
];

const SELF_DIRECTED_LOOP_SAFETY_FUSE = 24;
const SELF_DIRECTED_DRAFT_STEP_MS = 14;
const SELF_DIRECTED_DRAFT_SEND_DELAY_MS = 460;

interface SelfDirectedRunState {
  active: boolean;
  goal: string;
  cycles: number;
  seenAssistantIds: Set<string>;
  typingTimer: number | null;
  sendTimer: number | null;
}

function compactSelfDirectedText(value: string, maxLength = 460): string {
  const compacted = value.replace(/\s+/g, " ").trim();
  if (compacted.length <= maxLength) {
    return compacted;
  }
  return `${compacted.slice(0, maxLength - 1).trim()}...`;
}

function selfDirectedContinuationPrompt({
  cycle,
  goal,
  previousAssistantText,
  goalReached = false
}: {
  cycle: number;
  goal: string;
  previousAssistantText: string;
  goalReached?: boolean;
}): string {
  if (goalReached) {
    return [
      "SELF_DIRECTED_PROJECT_EXPANSION v1",
      `cycle=${cycle}`,
      `prior_goal="${compactSelfDirectedText(goal).replace(/"/g, "'")}"`,
      `completion_evidence="${compactSelfDirectedText(previousAssistantText, 360).replace(/"/g, "'")}"`,
      "role=agent_authored_prompt_visible_in_composer",
      "instruction=Invent the next stronger project direction yourself. Extend the result, raise quality, add missing professional depth, and start a new loop stream. If the new direction is ambiguous, open /questionnaire_; otherwise begin with the next concrete action and CodeAct event."
    ].join("\n");
  }
  return [
    "SELF_DIRECTED_CONTINUATION v1",
    `cycle=${cycle}`,
    `goal="${compactSelfDirectedText(goal).replace(/"/g, "'")}"`,
    `previous_assistant_summary="${compactSelfDirectedText(previousAssistantText, 360).replace(/"/g, "'")}"`,
    "role=agent_authored_prompt_visible_in_composer",
    "policy=work autonomously inside InGen; research the web when current knowledge may be stale; choose or switch Brain when useful; emit CodeAct commands instead of prose when an action is needed.",
    `brain_switches=${BRAIN_CODING_COMMAND} ${BRAIN_SCIENCE_COMMAND}`,
    `codeact_floor=${BRAIN_GOOGLEWEB_COMMAND} ${BRAIN_NEWCOMPUTE_COMMAND} ${BRAIN_SELECTCOMPUTE_COMMAND} ${BRAIN_NEWMODULE_COMMAND} ${BRAIN_NEWOBJECT_COMMAND} ${BRAIN_FRONTDESIGN_COMMAND}`,
    "guardrails=no payment, no destructive delete, no credential action and no irreversible external submit without explicit human confirmation.",
    "loop_stream=Write one short paragraph that states the next action and tool, then emit the CodeAct/event below it.",
    "stop_condition=Use the final questionnaire answer as the definition of done. When satisfied, include SELF_DIRECTED_GOAL_REACHED reason=\"...\" next_prompt=\"...\" so the UI can inject a stronger next prompt.",
    "instruction=Continue the project from the user's direction. Pick the next concrete goal yourself, do the useful research or CodeAct, then either produce the next artifact or prepare the next autonomous prompt."
  ].join("\n");
}

function selfDirectedQuestionnaireAnswersPrompt(value: string): string {
  return [
    "SELF_DIRECTED_QUESTIONNAIRE_ANSWERS v1",
    "role=user_clarified_goal",
    "instruction=The questionnaire is complete. Start autonomous loop stream work now. Use paragraph -> CodeAct event rhythm. Use the final answer as the stop condition.",
    "",
    value
  ].join("\n");
}

function selfDirectedAssistantReachedGoal(text: string): boolean {
  return /\bSELF_DIRECTED_GOAL_REACHED\b/.test(text);
}

function PermissionModeIcon({ mode }: { mode: PermissionMode }) {
  if (mode === "self-directed") {
    return <Sparkles aria-hidden="true" size={16} strokeWidth={1.8} />;
  }
  if (mode === "full-autonomy") {
    return <ShieldAlert aria-hidden="true" size={16} strokeWidth={1.8} />;
  }
  if (mode === "auto-accept-edits") {
    return <Pencil aria-hidden="true" size={16} strokeWidth={1.8} />;
  }
  return <ShieldCheck aria-hidden="true" size={16} strokeWidth={1.8} />;
}

function permissionModeShortLabel(mode: PermissionMode): string {
  if (mode === "self-directed") {
    return "Self";
  }
  if (mode === "full-autonomy") {
    return "Auto";
  }
  if (mode === "auto-accept-edits") {
    return "Edit";
  }
  return "Ask";
}

function BottomControlsFlipText({ value }: { value: string }) {
  const [flip, setFlip] = useState({ current: value, previous: "", serial: 0 });

  useEffect(() => {
    setFlip((currentFlip) => {
      if (currentFlip.current === value) {
        return currentFlip;
      }
      return {
        current: value,
        previous: currentFlip.current,
        serial: currentFlip.serial + 1
      };
    });
  }, [value]);

  useEffect(() => {
    if (!flip.previous) {
      return undefined;
    }
    const timeout = window.setTimeout(() => {
      setFlip((currentFlip) => currentFlip.serial === flip.serial ? { ...currentFlip, previous: "" } : currentFlip);
    }, 260);
    return () => window.clearTimeout(timeout);
  }, [flip.previous, flip.serial]);

  return (
    <span className="bottomControls__flipTextViewport" aria-hidden="true">
      {flip.previous ? (
        <span className="bottomControls__flipText bottomControls__flipText--leaving" key={`previous-${flip.serial}`}>
          {flip.previous}
        </span>
      ) : null}
      <span
        className={[
          "bottomControls__flipText",
          flip.previous ? "bottomControls__flipText--entering" : ""
        ].filter(Boolean).join(" ")}
        key={`current-${flip.serial}`}
      >
        {flip.current}
      </span>
    </span>
  );
}

export function PanelsChatBottomSlice({
  composerOnly = false,
  parallelPrompts,
  onParallelPromptChange,
  webExplorerOpen = false,
  composerModule = null,
  onComposerModuleChange,
  widgetMode = false,
  widgetModeTransitioning = false,
  onWidgetModeChange
}: PanelsChatBottomSliceProps = {}) {
  const { snapshot } = usePanelsChatBottomStore();
  const permissionMenuId = useId();
  const [draft, setDraft] = useState(snapshot.composer.chatText);
  const [focusedParallelIndex, setFocusedParallelIndex] = useState(0);
  const [permissionMenuOpen, setPermissionMenuOpen] = useState(false);
  const [selfDirectedMenuStage, setSelfDirectedMenuStage] = useState<"idle" | "console">("idle");
  const [moduleDropPhase, setModuleDropPhase] = useState<"idle" | "armed" | "over">("idle");
  const [fileDropPhase, setFileDropPhase] = useState<"idle" | "armed" | "over">("idle");
  const [composerSendBusyCount, setComposerSendBusyCount] = useState(0);
  const fileDragDepthRef = useRef(0);
  const panelsRef = useRef<HTMLElement>(null);
  const composerRef = useRef<HTMLFormElement>(null);
  const permissionControlRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const draftRef = useRef(draft);
  const composerResetFenceRef = useRef(false);
  const burstRef = useRef<ComposerSendBurstHandle>(null);
  const composerSendBusyRef = useRef(0);
  const sendEchoBaselineRef = useRef<number | null>(null);
  const [brainAgentName, setBrainAgentName] = useState(() => readBrainAgentMemory().preferredFirstName);
  const [selfDirectedDrafting, setSelfDirectedDrafting] = useState(false);
  const selfDirectedRunRef = useRef<SelfDirectedRunState>({
    active: false,
    goal: "",
    cycles: 0,
    seenAssistantIds: new Set(),
    typingTimer: null,
    sendTimer: null
  });

  useEffect(() => {
    void panelsChatBottomStore.refresh();
  }, []);

  useEffect(() => {
    const syncAgentName = () => setBrainAgentName(readBrainAgentMemory().preferredFirstName);
    const onAgentMemoryUpdated = (event: Event) => {
      const detail = (event as CustomEvent<{ preferredFirstName?: unknown }>).detail;
      if (typeof detail?.preferredFirstName === "string") {
        setBrainAgentName(detail.preferredFirstName);
        return;
      }
      syncAgentName();
    };
    const onStorage = (event: StorageEvent) => {
      if (event.key === "ingen.brain.memory.agent_identity.v1") {
        syncAgentName();
      }
    };
    window.addEventListener(BRAIN_AGENT_MEMORY_UPDATED_EVENT, onAgentMemoryUpdated);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener(BRAIN_AGENT_MEMORY_UPDATED_EVENT, onAgentMemoryUpdated);
      window.removeEventListener("storage", onStorage);
    };
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
    draftRef.current = draft;
  }, [draft]);

  useEffect(() => {
    if (selfDirectedDrafting) {
      return;
    }
    if (composerResetFenceRef.current) {
      if (snapshot.composer.chatText === "") {
        composerResetFenceRef.current = false;
      } else if (draftRef.current === "") {
        return;
      }
    }
    setDraft(snapshot.composer.chatText);
  }, [selfDirectedDrafting, snapshot.composer.chatText]);

  useEffect(() => {
    if (!permissionMenuOpen) {
      setSelfDirectedMenuStage("idle");
      return;
    }
    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && permissionControlRef.current?.contains(target)) {
        return;
      }
      setPermissionMenuOpen(false);
    };
    const closeOnEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setPermissionMenuOpen(false);
      }
    };
    window.addEventListener("pointerdown", closeOnOutsidePointer);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("pointerdown", closeOnOutsidePointer);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [permissionMenuOpen]);

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
        event.dataTransfer.dropEffect = fileDropEffectForTarget(event.target);
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

  useLayoutEffect(() => {
    const composer = composerRef.current;
    const panels = panelsRef.current;
    if (!composer || !panels) {
      return;
    }
    const syncComposerHeight = () => {
      panels.style.setProperty("--composer-live-height", `${composer.offsetHeight}px`);
    };
    syncComposerHeight();
    if (typeof ResizeObserver === "undefined") {
      return;
    }
    const observer = new ResizeObserver(syncComposerHeight);
    observer.observe(composer);
    return () => observer.disconnect();
  }, []);

  const dispatch = panelsChatBottomStore.dispatch;
  const providers = snapshot.composer.providers;
  const uploadPreviews = snapshot.composer.uploadPreviews;
  const permissionMode = snapshot.composer.permissionMode;
  const canvasMessages = snapshot.transcript.filter((message) => message.role !== "system");
  const activeQuestionnaire = useMemo(() => latestQuestionnaireFromMessages(canvasMessages), [canvasMessages]);
  const activeDropPhase = moduleDropPhase;
  const composerSendBusy = composerSendBusyCount > 0;
  // The send spinner runs from Enter until the user's message lands in the
  // transcript (canvas), not merely until the wave drains or dispatch resolves.
  const transcriptUserMessageCount = useMemo(() => {
    let count = snapshot.transcript.reduce((total, message) => total + (message.role === "user" ? 1 : 0), 0);
    for (const lane of snapshot.parallelLanes) {
      count += lane.transcript.reduce((total, message) => total + (message.role === "user" ? 1 : 0), 0);
    }
    return count;
  }, [snapshot.transcript, snapshot.parallelLanes]);
  const transcriptUserMessageCountRef = useRef(transcriptUserMessageCount);
  useEffect(() => {
    transcriptUserMessageCountRef.current = transcriptUserMessageCount;
  }, [transcriptUserMessageCount]);
  const beginComposerSendBusy = useCallback(() => {
    if (composerSendBusyRef.current > 0) {
      return false;
    }
    composerSendBusyRef.current += 1;
    setComposerSendBusyCount(composerSendBusyRef.current);
    sendEchoBaselineRef.current = transcriptUserMessageCountRef.current;
    return true;
  }, []);
  const endComposerSendBusy = useCallback(() => {
    sendEchoBaselineRef.current = null;
    composerSendBusyRef.current = Math.max(0, composerSendBusyRef.current - 1);
    setComposerSendBusyCount(composerSendBusyRef.current);
  }, []);
  // Release the spinner the moment the sent message echoes into the transcript.
  useEffect(() => {
    if (sendEchoBaselineRef.current !== null && transcriptUserMessageCount > sendEchoBaselineRef.current) {
      endComposerSendBusy();
    }
  }, [transcriptUserMessageCount, endComposerSendBusy]);
  // Dispatch failures must still release the spinner even though success now
  // waits for the transcript echo above.
  const dispatchTrackedComposerSend = useCallback((command: Omit<PanelsChatBottomCommand, "version" | "requestId">) => {
    void dispatch(command).catch(endComposerSendBusy);
  }, [dispatch, endComposerSendBusy]);
  const attachFiles = () => void dispatch({ kind: "attach_files" });
  const attachDroppedFiles = (filePaths: string[]) => {
    if (filePaths.length === 0) {
      return;
    }
    void dispatch({ kind: "attach_dropped_files", filePaths }).then(() => inputRef.current?.focus());
  };
  const toggleWidgetMode = () => {
    if (!onWidgetModeChange || widgetModeTransitioning) {
      return;
    }
    if (widgetMode) {
      onWidgetModeChange(false);
      return;
    }
    onWidgetModeChange(true);
  };
  const stageImageForEdit = useCallback((preview: ComposerUploadPreview) => {
    void dispatch({ kind: "stage_attachment_for_edit", attachmentIds: [preview.id] }).then(() => {
      globalThis.window?.dispatchEvent(new CustomEvent(IMAGE_EDIT_STAGED_EVENT, { detail: { attachmentId: preview.id } }));
      inputRef.current?.focus();
    });
  }, [dispatch]);
  const clearSelfDirectedTimers = useCallback(() => {
    const run = selfDirectedRunRef.current;
    if (run.typingTimer !== null) {
      window.clearTimeout(run.typingTimer);
      run.typingTimer = null;
    }
    if (run.sendTimer !== null) {
      window.clearTimeout(run.sendTimer);
      run.sendTimer = null;
    }
  }, []);
  const cancelSelfDirectedDraft = useCallback(() => {
    clearSelfDirectedTimers();
    selfDirectedRunRef.current.active = false;
    setSelfDirectedDrafting(false);
  }, [clearSelfDirectedTimers]);
  const startSelfDirectedRun = useCallback((goal: string) => {
    clearSelfDirectedTimers();
    selfDirectedRunRef.current = {
      active: true,
      goal: goal.trim() || "Continue le projet en autonomie.",
      cycles: 0,
      seenAssistantIds: new Set(),
      typingTimer: null,
      sendTimer: null
    };
  }, [clearSelfDirectedTimers]);
  const typeSelfDirectedDraft = useCallback((value: string) => {
    const run = selfDirectedRunRef.current;
    clearSelfDirectedTimers();
    setSelfDirectedDrafting(true);
    setDraft("");
    inputRef.current?.focus();

    const commit = () => {
      run.sendTimer = window.setTimeout(() => {
        run.sendTimer = null;
        setSelfDirectedDrafting(false);
        setDraft("");
        void dispatch({
          kind: "send_chat",
          value,
          moduleId: composerModule ?? undefined
        });
      }, prefersReducedMotion() ? 80 : SELF_DIRECTED_DRAFT_SEND_DELAY_MS);
    };

    if (prefersReducedMotion()) {
      setDraft(value);
      commit();
      return;
    }

    let cursor = 0;
    const tick = () => {
      cursor = Math.min(value.length, cursor + Math.max(1, Math.ceil(value.length / 180)));
      setDraft(value.slice(0, cursor));
      if (cursor >= value.length) {
        run.typingTimer = null;
        commit();
        return;
      }
      run.typingTimer = window.setTimeout(tick, SELF_DIRECTED_DRAFT_STEP_MS);
    };
    tick();
  }, [clearSelfDirectedTimers, composerModule, dispatch]);
  const commitQuestionnaireAnswers = useCallback((value: string, parallelSessionIndex = 0) => {
    const nextValue =
      permissionMode === "self-directed" && parallelSessionIndex === 0
        ? selfDirectedQuestionnaireAnswersPrompt(value)
        : value;
    if (permissionMode === "self-directed" && parallelSessionIndex === 0) {
      startSelfDirectedRun(nextValue);
    }
    setDraft("");
    void dispatch({
      kind: "send_chat",
      value: nextValue,
      moduleId: composerModule ?? undefined,
      parallelSessionIndex: parallelSessionIndex > 0 ? parallelSessionIndex : undefined
    });
    inputRef.current?.focus();
  }, [composerModule, dispatch, permissionMode, startSelfDirectedRun]);
  const parallelMode = Boolean(parallelPrompts && parallelPrompts.length > 1 && onParallelPromptChange);
  const focusedParallelPrompt = parallelMode && parallelPrompts ? parallelPrompts[focusedParallelIndex]?.trim() ?? "" : "";
  const focusedParallelQuestionnaire = useMemo(() => {
    if (!parallelMode || focusedParallelIndex <= 0) {
      return null;
    }
    const lane = snapshot.parallelLanes.find((candidate) => candidate.index === focusedParallelIndex);
    const laneMessages = lane?.transcript.filter((message) => message.role !== "system") ?? [];
    return latestQuestionnaireFromMessages(laneMessages);
  }, [focusedParallelIndex, parallelMode, snapshot.parallelLanes]);
  const composerQuestionnaire = focusedParallelQuestionnaire
    ? { questionnaire: focusedParallelQuestionnaire, parallelSessionIndex: focusedParallelIndex }
    : activeQuestionnaire
      ? { questionnaire: activeQuestionnaire, parallelSessionIndex: 0 }
      : null;
  const useMathInCompute = useCallback((formula: string) => {
    const math = formula.trim();
    if (!math || parallelMode) {
      return;
    }
    if (selfDirectedDrafting) {
      cancelSelfDirectedDraft();
    }
    onComposerModuleChange?.("compute");
    setDraft((current) => {
      const nextDraft = current.trim()
        ? `${current.trimEnd()}\n${math}`
        : math;
      void dispatch({ kind: "chat_text_edited", value: nextDraft });
      return nextDraft;
    });
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [cancelSelfDirectedDraft, dispatch, onComposerModuleChange, parallelMode, selfDirectedDrafting]);
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
  const assistantStopActive = composerSendBusy || Boolean(snapshot.composer.assistantBusy) || selfDirectedDrafting;
  const stopAssistant = useCallback(() => {
    cancelSelfDirectedDraft();
    endComposerSendBusy();
    void dispatch({ kind: "stop_assistant" });
  }, [cancelSelfDirectedDraft, dispatch, endComposerSendBusy]);
  const openSelfDirectedDetails = () => {
    setSelfDirectedMenuStage("console");
  };
  const selectPermissionMode = (mode: PermissionMode) => {
    if (mode === "self-directed" && selfDirectedMenuStage === "idle") {
      openSelfDirectedDetails();
      return;
    }
    setPermissionMenuOpen(false);
    if (mode !== permissionMode) {
      void dispatch({ kind: "permission_mode_selected", value: mode });
    }
  };
  useEffect(() => {
    if (permissionMode !== "self-directed") {
      cancelSelfDirectedDraft();
    }
  }, [cancelSelfDirectedDraft, permissionMode]);
  useEffect(() => () => clearSelfDirectedTimers(), [clearSelfDirectedTimers]);
  useEffect(() => {
    if (
      permissionMode !== "self-directed" ||
      parallelMode ||
      composerQuestionnaire ||
      selfDirectedDrafting ||
      draft.trim() ||
      uploadPreviews.length > 0
    ) {
      return;
    }
    const run = selfDirectedRunRef.current;
    if (!run.active || run.cycles >= SELF_DIRECTED_LOOP_SAFETY_FUSE) {
      return;
    }
    const lastAssistant = [...snapshot.transcript].reverse().find((message) => message.role === "assistant" && message.text.trim().length > 0);
    if (!lastAssistant || run.seenAssistantIds.has(lastAssistant.id)) {
      return;
    }
    run.seenAssistantIds.add(lastAssistant.id);
    run.cycles += 1;
    const lastUserGoal = [...snapshot.transcript].reverse().find((message) => message.role === "user" && message.text.trim().length > 0)?.text;
    typeSelfDirectedDraft(selfDirectedContinuationPrompt({
      cycle: run.cycles,
      goal: run.goal || lastUserGoal || "Continue le projet en autonomie.",
      previousAssistantText: lastAssistant.text,
      goalReached: selfDirectedAssistantReachedGoal(lastAssistant.text)
    }));
  }, [
    composerQuestionnaire,
    draft,
    parallelMode,
    permissionMode,
    selfDirectedDrafting,
    snapshot.transcript,
    typeSelfDirectedDraft,
    uploadPreviews.length
  ]);
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
    if (assistantStopActive) {
      stopAssistant();
      return;
    }
    if (parallelMode && parallelPrompts && parallelIndex === undefined && filledParallelPrompts.length > 0) {
      if (!beginComposerSendBusy()) {
        return;
      }
      const sessionIsNew = !snapshot.transcript.some((message) => message.role === "user" || message.role === "assistant");
      const commit = () => {
        try {
          for (const prompt of filledParallelPrompts) {
            onParallelPromptChange?.(prompt.index, "");
          }
          const command: Omit<PanelsChatBottomCommand, "version" | "requestId"> = {
            kind: "send_parallel_chat_batch",
            parallelDrafts: filledParallelPrompts.map((prompt) => ({
              parallelSessionIndex: prompt.index,
              value: prompt.value
            })),
            moduleId: composerModule ?? undefined
          };
          dispatchTrackedComposerSend(command);
        } catch (error) {
          endComposerSendBusy();
          throw error;
        }
      };
      const burst = burstRef.current;
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
    if (!beginComposerSendBusy()) {
      return;
    }
    if (!parallelMode) {
      composerResetFenceRef.current = true;
      setDraft("");
    }
    const commit = () => {
      try {
        if (parallelMode && parallelPrompts && onParallelPromptChange) {
          onParallelPromptChange(targetParallelIndex, "");
        }
        if (permissionMode === "self-directed" && targetParallelIndex === 0) {
          startSelfDirectedRun(value);
        }
        const command: Omit<PanelsChatBottomCommand, "version" | "requestId"> = {
          kind: "send_chat",
          value,
          moduleId: composerModule ?? undefined,
          parallelSessionIndex: parallelMode ? targetParallelIndex : undefined
        };
        dispatchTrackedComposerSend(command);
      } catch (error) {
        endComposerSendBusy();
        throw error;
      }
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
      ref={panelsRef}
      className={[
        "panelsChatBottom",
        composerQuestionnaire ? "panelsChatBottom--questionnaireOpen" : "",
        permissionMode === "self-directed" ? "panelsChatBottom--selfDirected" : ""
      ].filter(Boolean).join(" ")}
      aria-label="Panels chat composer and bottom controls"
      onDragEnter={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        event.preventDefault();
        fileDragDepthRef.current += 1;
        setFileDropPhase(targetInsideComposer(event.target) ? "over" : "idle");
      }}
      onDragOver={(event) => {
        if (!hasDraggedFiles(event.dataTransfer)) {
          return;
        }
        event.preventDefault();
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = fileDropEffectForTarget(event.target);
        }
        setFileDropPhase(targetInsideComposer(event.target) ? "over" : "idle");
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
        if (targetInsideComposer(event.target)) {
          attachDroppedFiles(droppedFilePaths(event.dataTransfer));
        }
      }}
    >
      {!composerOnly ? (
        parallelMode && parallelPrompts ? (
          <div className={`parallelTranscriptGrid parallelTranscriptGrid--count${parallelPrompts.length}`}>
            {parallelPrompts.map((_prompt, index) => {
              const lane = snapshot.parallelLanes.find((candidate) => candidate.index === index);
              const laneMessages = index === 0 ? canvasMessages : lane?.transcript.filter((message) => message.role !== "system") ?? [];
              return (
                <TranscriptCanvas
                  activeSessionId={index === 0 ? snapshot.activeSessionId : lane?.sessionId ?? `parallel-${index}`}
                  messages={laneMessages}
                  agentName={brainAgentName}
                  parallelSessionIndex={index}
                  className="chatCanvas chatCanvas--parallelPane"
                  key={`parallel-transcript-${index}`}
                  onEditImage={stageImageForEdit}
                  onUseMathInCompute={useMathInCompute}
                />
              );
            })}
          </div>
        ) : (
          <TranscriptCanvas
            key={snapshot.activeSessionId || "draft-session"}
            activeSessionId={snapshot.activeSessionId}
            messages={canvasMessages}
            agentName={brainAgentName}
            parallelSessionIndex={0}
            onEditImage={stageImageForEdit}
            onUseMathInCompute={useMathInCompute}
          />
        )
      ) : null}
      <div
        className={activeDropPhase !== "idle" ? "composerDropScrim composerDropScrim--active" : "composerDropScrim"}
        aria-hidden="true"
      />
      {composerQuestionnaire ? (
        <div className="questionnaireFocusScrim" aria-hidden="true" />
      ) : null}
      {composerQuestionnaire ? (
        <ComposerQuestionnaire
          questionnaire={composerQuestionnaire.questionnaire}
          onCommitAnswers={(value) => commitQuestionnaireAnswers(value, composerQuestionnaire.parallelSessionIndex)}
        />
      ) : null}
      <form
        ref={composerRef}
        className={[
          "composer",
          activeDropPhase !== "idle" ? "composer--moduleDropArmed" : "",
          activeDropPhase === "over" ? "composer--moduleDropOver" : "",
          fileDropPhase !== "idle" ? "composer--fileDropArmed" : "",
          selfDirectedDrafting ? "composer--selfDirectedDrafting" : ""
        ].filter(Boolean).join(" ")}
        aria-label="Chat composer"
        onSubmit={(event) => {
          event.preventDefault();
          if (assistantStopActive) {
            stopAssistant();
          } else {
            sendComposer();
          }
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
                      if (!assistantStopActive) {
                        sendComposer(index);
                      }
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
              if (selfDirectedDrafting) {
                cancelSelfDirectedDraft();
              }
              composerResetFenceRef.current = false;
              setDraft(event.currentTarget.value);
              void dispatch({ kind: "chat_text_edited", value: event.currentTarget.value });
            }}
            onKeyDown={(event) => {
              emitChatKeyColor(event);
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                if (!assistantStopActive) {
                  sendComposer();
                }
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
          className={[
            "composer__send",
            assistantStopActive ? "composer__send--stop" : "",
            !canSend && !assistantStopActive ? "composer__send--empty" : ""
          ].filter(Boolean).join(" ")}
          aria-label={assistantStopActive ? "Stop assistant" : "Send message"}
          aria-busy={assistantStopActive}
          disabled={!canSend && !assistantStopActive}
        >
          {assistantStopActive ? (
            <Square className="sendGlyph" size={15} strokeWidth={2.6} aria-hidden="true" />
          ) : (
            <svg className="sendGlyph" width="18" height="18" viewBox="0 0 24 24" aria-hidden="true">
              <path
                fill="currentColor"
                d="M20 4v9a4 4 0 0 1-4 4H6.914l2.5 2.5L8 20.914L3.086 16L8 11.086L9.414 12.5l-2.5 2.5H16a2 2 0 0 0 2-2V4h2Z"
              />
            </svg>
          )}
        </button>
      </form>

      <nav className="bottomControls" aria-label="Bottom controls">
        <div className="permissionModeControl" ref={permissionControlRef}>
          <button
            type="button"
            className={[
              "permissionModeTrigger",
              permissionMode === "self-directed" ? "permissionModeTrigger--selfDirected" : ""
            ].filter(Boolean).join(" ")}
            aria-haspopup="menu"
            aria-expanded={permissionMenuOpen}
            aria-controls={permissionMenuOpen ? permissionMenuId : undefined}
            title="Permission mode"
            onClick={() => setPermissionMenuOpen((open) => !open)}
          >
            <PermissionModeIcon mode={permissionMode} />
            <span>{permissionModeShortLabel(permissionMode)}</span>
            <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
              <path d="M2.2 3.6 5 6.4l2.8-2.8" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          {permissionMenuOpen ? (
            <div
              className={[
                "permissionModeMenu",
                selfDirectedMenuStage !== "idle" ? "permissionModeMenu--selfDirectedFlow" : "",
                selfDirectedMenuStage === "console" ? "permissionModeMenu--selfDirectedConsole" : ""
              ].filter(Boolean).join(" ")}
              id={permissionMenuId}
              role="menu"
              aria-label="Permission mode"
            >
              <p className="permissionModeMenu__eyebrow">Mode</p>
              {(selfDirectedMenuStage === "idle"
                ? PERMISSION_MODE_CHOICES
                : [
                  PERMISSION_MODE_CHOICES.find((option) => option.value === "self-directed"),
                  ...PERMISSION_MODE_CHOICES.filter((option) => option.value !== "self-directed")
                ].filter((option): option is typeof PERMISSION_MODE_CHOICES[number] => Boolean(option))
              ).map((option) => {
                const selected = permissionMode === option.value;
                const special = option.value === "self-directed";
                const hiddenBySelfDirectedFlow = selfDirectedMenuStage !== "idle" && !special;
                return (
                  <button
                    type="button"
                    className={[
                      "permissionModeOption",
                      selected ? "permissionModeOption--selected" : "",
                      special ? "permissionModeOption--selfDirected" : "",
                      special && selfDirectedMenuStage === "console" ? "permissionModeOption--selfDirectedExpanded" : "",
                      hiddenBySelfDirectedFlow ? "permissionModeOption--selfDirectedExit" : ""
                    ].filter(Boolean).join(" ")}
                    key={option.value}
                    role="menuitemradio"
                    aria-checked={selected}
                    aria-hidden={hiddenBySelfDirectedFlow ? true : undefined}
                    tabIndex={hiddenBySelfDirectedFlow ? -1 : undefined}
                    onClick={() => selectPermissionMode(option.value)}
                  >
                    <span className="permissionModeOption__icon" aria-hidden="true">
                      <PermissionModeIcon mode={option.value} />
                    </span>
                    <span className="permissionModeOption__copy">
                      <span className="permissionModeOption__label">{option.label}</span>
                      <span className="permissionModeOption__detail">{option.detail}</span>
                      {special && selfDirectedMenuStage === "console" ? (
                        <span className="permissionModeOption__autonomy" aria-label="Self-Directed autonomy behavior">
                          <span className="permissionModeOption__autonomyItems">
                            <span className="permissionModeOption__autonomyItem">
                              <ListChecks aria-hidden="true" size={14} strokeWidth={1.8} />
                              <span>
                                <strong>Plans</strong>
                                <small>Chooses useful next goals.</small>
                              </span>
                            </span>
                            <span className="permissionModeOption__autonomyItem">
                              <Sparkles aria-hidden="true" size={14} strokeWidth={1.8} />
                              <span>
                                <strong>Prompts</strong>
                                <small>Writes its own prompts.</small>
                              </span>
                            </span>
                            <span className="permissionModeOption__autonomyItem">
                              <RefreshCw aria-hidden="true" size={14} strokeWidth={1.8} />
                              <span>
                                <strong>Continues</strong>
                                <small>Creates follow-up tasks.</small>
                              </span>
                            </span>
                          </span>
                          <span className="permissionModeOption__autonomyGuardrail">
                            <Ban aria-hidden="true" size={13} strokeWidth={1.9} />
                            <span>No payments. No destructive deletes.</span>
                          </span>
                        </span>
                      ) : null}
                    </span>
                    <span className="permissionModeOption__check" aria-hidden="true">
                      {selected ? (
                        <svg width="12" height="12" viewBox="0 0 12 12">
                          <path d="m2.4 6.2 2.2 2.2 5-5.2" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                      ) : null}
                    </span>
                  </button>
                );
              })}
            </div>
          ) : null}
        </div>
        <button
          type="button"
          className={widgetMode ? "bottomControls__widgetButton bottomControls__widgetButton--active" : "bottomControls__widgetButton"}
          title="Mode widget"
          aria-label="Passer en mode widget"
          aria-pressed={widgetMode}
          disabled={widgetModeTransitioning}
          onClick={toggleWidgetMode}
        >
          <strong>MINI</strong>
        </button>
        <div className="bottomControls__models">
          <button
            type="button"
            className="bottomControls__flipButton bottomControls__flipButton--model"
            title={`flip model ${snapshot.composer.modelLabel}`}
            aria-label={`Flip model from ${snapshot.composer.modelLabel}`}
            onClick={() => void dispatch({ kind: "cycle_llm_model", provider: snapshot.composer.selectedProvider, direction: 1 })}
          >
            <ChevronsUpDown aria-hidden="true" size={9} strokeWidth={2.1} />
          </button>
          <button
            type="button"
            className="bottomControls__modelButton"
            title={snapshot.composer.modelLabel}
            aria-label={`Current model ${snapshot.composer.modelLabel}. Flip model.`}
            onClick={() => void dispatch({ kind: "cycle_llm_model", provider: snapshot.composer.selectedProvider, direction: 1 })}
          >
            <BottomControlsFlipText value={snapshot.composer.modelLabel} />
          </button>
          <button
            type="button"
            className="bottomControls__reasoningButton"
            title={`reasoning ${snapshot.composer.reasoningLabel}`}
            aria-label={`Current reasoning power ${snapshot.composer.reasoningLabel}. Flip reasoning power.`}
            onClick={() => void dispatch({ kind: "cycle_llm_reasoning", direction: 1 })}
          >
            <BottomControlsFlipText value={snapshot.composer.reasoningLabel} />
          </button>
          <button
            type="button"
            className="bottomControls__flipButton bottomControls__flipButton--reasoning"
            title={`flip reasoning ${snapshot.composer.reasoningLabel}`}
            aria-label={`Flip reasoning power from ${snapshot.composer.reasoningLabel}`}
            onClick={() => void dispatch({ kind: "cycle_llm_reasoning", direction: 1 })}
          >
            <ChevronsUpDown aria-hidden="true" size={9} strokeWidth={2.1} />
          </button>
        </div>
      </nav>

      <ComposerSendBurst ref={burstRef} />
    </section>
  );
}
