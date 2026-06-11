import { useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { Object3D } from "three";
import type { ComposerUploadPreview, TranscriptMessage } from "../shared/ipc-contract";
import { ComposerSendBurst, type ComposerSendBurstHandle } from "./ComposerSendBurst";
import { panelsChatBottomStore, usePanelsChatBottomStore } from "./panels-chat-bottom-store";
import { ProviderLogo } from "./ProviderLogo";
import { ModuleLogo, type SidebarModuleId } from "./SidebarSlice";
import { sidebarShadowStore } from "./sidebar-shadow-store";

const COMPOSER_MAX_INPUT_HEIGHT = 360;
const CHAT_KEY_COLOR_EVENT = "ingen:chat-key-color";
const UPLOAD_PREVIEW_MAX_PDF_PAGES = 3;

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
    const settleOnStillFrame = () => {
      video.pause();
      try {
        const duration = Number.isFinite(video.duration) && video.duration > 0 ? video.duration : 0;
        video.currentTime = duration > 1 ? Math.min(0.5, duration * 0.08) : 0;
      } catch {
        // Metadata-only previews are allowed to fail on codecs Electron cannot seek.
      }
    };
    video.addEventListener("loadedmetadata", settleOnStillFrame);
    return () => {
      video.removeEventListener("loadedmetadata", settleOnStillFrame);
      video.pause();
    };
  }, [preview.url]);

  return (
    <video
      ref={videoRef}
      className="composerUploadPreview__video"
      src={preview.url}
      muted
      playsInline
      preload="metadata"
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

type TranscriptMediaFrame = "portrait" | "landscape" | "square";

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
  if (ratio < 0.86) {
    return "portrait";
  }
  if (ratio > 1.16) {
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
  if (frame === "portrait") {
    return { width: 75, height: 100 };
  }
  if (frame === "landscape") {
    return { width: 160, height: 90 };
  }
  return { width: 100, height: 100 };
}

function dreamBlobBlur(box: BlobBox): number {
  return Math.max(4, Math.min(box.width, box.height) * 0.06);
}

function blobEdgeInset(box: BlobBox): BlobPoint {
  // The feather must finish dissolving before the frame edge: at 2.4 sigma the
  // residual alpha is under 1%, so the box never clips the fade flat.
  const inset = dreamBlobBlur(box) * 2.4;
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
    x: box.width * (frame === "portrait" ? 0.44 : 0.47),
    y: box.height * (frame === "landscape" ? 0.44 : 0.47)
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
    return (
      <video
        className="transcriptAttachment__media"
        src={preview.url}
        controls
        playsInline
        preload="metadata"
        onLoadedMetadata={(event) => onMeasure?.(event.currentTarget.videoWidth, event.currentTarget.videoHeight)}
      />
    );
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

function TranscriptAttachmentFigure({ preview }: { preview: ComposerUploadPreview }) {
  const [frame, setFrame] = useState<TranscriptMediaFrame>(preview.kind === "model3d" ? "square" : "landscape");
  const reactId = useId();
  const maskId = `transcript-dream-mask-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const blurId = `transcript-dream-blur-${reactId.replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const box = useMemo(() => blobBoxForFrame(frame), [frame]);
  const blobPath = useMemo(() => blobPathForAttachment(`${preview.id}:${preview.name}`, frame, box), [box, frame, preview.id, preview.name]);

  if (preview.kind === "image") {
    return (
      <figure className={`transcriptAttachment transcriptAttachment--${preview.kind} transcriptAttachment--frame-${frame}`} title={preview.name}>
        <TranscriptImageDreamPreview
          preview={preview}
          maskId={maskId}
          blurId={blurId}
          blobPath={blobPath}
          box={box}
          onMeasure={(width, height) => setFrame(transcriptMediaFrame(width, height))}
        />
      </figure>
    );
  }

  return (
    <figure className={`transcriptAttachment transcriptAttachment--${preview.kind} transcriptAttachment--frame-${frame}`} title={preview.name}>
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

function TranscriptAttachmentStack({ previews }: { previews: ComposerUploadPreview[] }) {
  if (previews.length === 0) {
    return null;
  }
  return (
    <div className="transcriptAttachmentStack">
      {previews.map((preview) => (
        <TranscriptAttachmentFigure key={preview.id} preview={preview} />
      ))}
    </div>
  );
}

function isTranscriptVisualAttachment(preview: ComposerUploadPreview): boolean {
  return preview.kind === "image" || preview.kind === "video" || preview.kind === "model3d";
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

function ThreeUploadPreview({ preview }: { preview: ComposerUploadPreview }) {
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

      const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      renderer.setClearColor(0x000000, 0);
      host.replaceChildren(renderer.domElement);

      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(40, 1, 0.01, 1000);
      camera.position.set(0, 0, 4.2);
      scene.add(new THREE.HemisphereLight(0xffffff, 0x707070, 1.8));
      const keyLight = new THREE.DirectionalLight(0xffffff, 2.2);
      keyLight.position.set(3, 4, 5);
      scene.add(keyLight);

      const modelRoot = new THREE.Group();
      scene.add(modelRoot);

      const previewMaterial = () =>
        new THREE.MeshStandardMaterial({
          color: 0xd7d7d7,
          metalness: 0.16,
          roughness: 0.44
        });
      const fallbackMesh = () => new THREE.Mesh(new THREE.BoxGeometry(1.3, 1.3, 1.3), previewMaterial());
      const resize = () => {
        const width = Math.max(1, Math.floor(host.clientWidth));
        const height = Math.max(1, Math.floor(host.clientHeight));
        renderer.setSize(width, height, false);
        camera.aspect = 1;
        camera.updateProjectionMatrix();
      };
      const renderScene = () => {
        if (disposed) {
          return;
        }
        resize();
        modelRoot.rotation.y = 0.72;
        modelRoot.rotation.x = -0.18;
        camera.lookAt(0, 0, 0);
        renderer.render(scene, camera);
      };
      const requestRender = () => {
        window.cancelAnimationFrame(frameId);
        frameId = window.requestAnimationFrame(renderScene);
      };
      const frameModel = (object: Object3D) => {
        const box = new THREE.Box3().setFromObject(object);
        const center = box.getCenter(new THREE.Vector3());
        const sphere = box.getBoundingSphere(new THREE.Sphere());
        object.position.sub(center);
        object.scale.multiplyScalar(1.1 / Math.max(sphere.radius, 0.001));
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
        requestRender();
      };

      const extension = preview.name.split(".").pop()?.toLowerCase() ?? "";
      if (extension === "glb" || extension === "gltf") {
        new GLTFLoader().load(preview.url, (gltf) => addObject(gltf.scene), undefined, () => addObject(fallbackMesh()));
      } else if (extension === "obj") {
        new OBJLoader().load(preview.url, addObject, undefined, () => addObject(fallbackMesh()));
      } else if (extension === "stl") {
        new STLLoader().load(
          preview.url,
          (geometry) => addObject(new THREE.Mesh(geometry, previewMaterial())),
          undefined,
          () => addObject(fallbackMesh())
        );
      } else if (extension === "ply") {
        new PLYLoader().load(
          preview.url,
          (geometry) => addObject(new THREE.Mesh(geometry, previewMaterial())),
          undefined,
          () => addObject(fallbackMesh())
        );
      } else {
        addObject(fallbackMesh());
      }

      const resizeObserver = new ResizeObserver(requestRender);
      resizeObserver.observe(host);
      requestRender();
      cleanupRenderer = () => {
        window.cancelAnimationFrame(frameId);
        resizeObserver.disconnect();
        disposeObject(scene);
        renderer.dispose();
      };
    })();

    return () => {
      disposed = true;
      cleanupRenderer?.();
      host.replaceChildren();
    };
  }, [preview.name, preview.url]);

  return <div ref={hostRef} className="composerUploadPreview__model" aria-label={preview.name} />;
}

interface PinnedChapter {
  id: string;
  text: string;
}

const PINS_STORAGE_KEY = "ingen.chat.pins.v1";

function loadPins(): PinnedChapter[] {
  try {
    const raw = globalThis.localStorage?.getItem(PINS_STORAGE_KEY);
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

function prefersReducedMotion() {
  return globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

function splitAssistantParagraphs(text: string) {
  const paragraphs = text
    .trim()
    .split(/\n{2,}/)
    .map((paragraph) => paragraph.trim())
    .filter(Boolean);
  return paragraphs.length > 0 ? paragraphs : [text.trim()];
}

function AnimatedAssistantText({ message }: { message: TranscriptMessage }) {
  const paragraphs = useMemo(() => splitAssistantParagraphs(message.text), [message.text]);
  const totalCharacters = useMemo(
    () => paragraphs.reduce((total, paragraph) => total + paragraph.length, 0),
    [paragraphs]
  );
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
  }, [message.id, message.text, totalCharacters]);

  useEffect(() => {
    if (completionReportedRef.current || totalCharacters === 0 || !animationSettled || visibleCharacters < totalCharacters) {
      return;
    }
    completionReportedRef.current = true;
    sidebarShadowStore.finishChatSessionPreview();
    void panelsChatBottomStore.dispatch({ kind: "assistant_write_complete", value: message.id });
  }, [animationSettled, message.id, totalCharacters, visibleCharacters]);

  let remainingCharacters = visibleCharacters;
  const writing = visibleCharacters < totalCharacters;

  return (
    <div className="assistantText" aria-label={message.text}>
      <div aria-hidden="true">
        {paragraphs.map((paragraph, index) => {
          const shown = Math.max(0, Math.min(paragraph.length, remainingCharacters));
          remainingCharacters -= paragraph.length;
          if (shown === 0 && writing) {
            return null;
          }
          const paragraphDone = shown >= paragraph.length;
          return (
            <p className="assistantText__paragraph" key={`${message.id}-${index}`}>
              {paragraph.slice(0, shown)}
              {!paragraphDone && index === paragraphs.findIndex((candidate, candidateIndex) => {
                const previous = paragraphs.slice(0, candidateIndex).reduce((total, item) => total + item.length, 0);
                return visibleCharacters <= previous + candidate.length;
              }) ? <span className="assistantText__caret" /> : null}
            </p>
          );
        })}
      </div>
    </div>
  );
}

function TranscriptCanvas({ messages }: { messages: TranscriptMessage[] }) {
  const [pins, setPins] = useState<PinnedChapter[]>(loadPins);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const messagesRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    try {
      globalThis.localStorage?.setItem(PINS_STORAGE_KEY, JSON.stringify(pins));
    } catch {
      // storage unavailable: pins stay in memory for this session only
    }
  }, [pins]);

  const pinnedIds = new Set(pins.map((pin) => pin.id));

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

  const stagedVisualAttachments =
    [...messages]
      .reverse()
      .find((message) => (message.attachments ?? []).some(isTranscriptVisualAttachment))
      ?.attachments?.filter(isTranscriptVisualAttachment) ?? [];
  const chatCanvasClassName = stagedVisualAttachments.length > 0 ? "chatCanvas chatCanvas--media-stage" : "chatCanvas";

  return (
    <div className={chatCanvasClassName}>
      {pins.length > 0 ? (
        <div className="chatCanvas__chapters" aria-label="Pinned chapters">
          {pins.map((pin) => (
            <button type="button" className="chapterChip" key={pin.id} title={pin.text} onClick={() => jumpToChapter(pin.id)}>
              <PinGlyph filled />
              <span>{pin.text}</span>
            </button>
          ))}
        </div>
      ) : null}
      {stagedVisualAttachments.length > 0 ? (
        <aside className="transcriptMediaStage" aria-label="Canvas media preview">
          <TranscriptAttachmentStack previews={stagedVisualAttachments} />
        </aside>
      ) : null}
      <div className="chatCanvas__messages" ref={messagesRef}>
        {messages.map((message) => {
          const attachments = message.attachments ?? [];
          const visualAttachments = attachments.filter(isTranscriptVisualAttachment);
          if (!message.text.trim() && attachments.length === 0) {
            return null;
          }
          if (message.role === "user" && !message.text.trim() && visualAttachments.length > 0) {
            return null;
          }
          const role = message.role === "user" ? "user" : "assistant";
          const pinned = pinnedIds.has(message.id);
          const assistantError = role === "assistant" && message.id.startsWith("assistant-error-");
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
          return (
            <div className={`transcriptItem transcriptItem--${role}`} data-msg-id={message.id} key={message.id}>
              {role === "assistant" ? (
                <div className="transcriptTextFrame">
                  <div className={assistantError ? "transcriptPill transcriptPill--assistant transcriptPill--assistantError" : "transcriptPill transcriptPill--assistant"}>
                    <AnimatedAssistantText message={message} />
                  </div>
                  {actions}
                </div>
              ) : (
                <>
                  <div className="transcriptUserStack">
                    {message.text.trim() ? <p className="transcriptPill transcriptPill--user">{message.text}</p> : null}
                    {actions}
                  </div>
                </>
              )}
            </div>
          );
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
  const composerRef = useRef<HTMLFormElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const burstRef = useRef<ComposerSendBurstHandle>(null);

  useEffect(() => {
    void panelsChatBottomStore.refresh();
  }, []);

  useEffect(() => {
    setDraft(snapshot.composer.chatText);
  }, [snapshot.composer.chatText]);

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
  const attachFiles = () => void dispatch({ kind: "attach_files" });
  const parallelMode = Boolean(parallelPrompts && parallelPrompts.length > 1 && onParallelPromptChange);
  const parallelDraft = parallelPrompts?.map((prompt, index) => `/ ${index + 1}: ${prompt.trim()}`).join("\n") ?? "";
  const submitText = parallelMode ? parallelDraft : draft;
  const canSend = Boolean(submitText.trim() || uploadPreviews.length > 0);
  const sendComposer = () => {
    const value = submitText;
    if (!value.trim() && uploadPreviews.length === 0) {
      return;
    }
    // The pixel border laps the composer frame, then explodes; the message is
    // committed to the canvas at that peak (handled inside fire, or immediately
    // when reduced motion / no canvas is available).
    const commit = () => void dispatch({ kind: "send_chat", value });
    const burst = burstRef.current;
    if (burst) {
      burst.fire(composerRef.current, commit);
    } else {
      commit();
    }
  };

  return (
    <section className="panelsChatBottom" aria-label="Panels chat composer and bottom controls">
      <TranscriptCanvas messages={canvasMessages} />
      <form
        ref={composerRef}
        className="composer"
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
          <div className="composerUploadPreview" aria-hidden={uploadPreviews.length > 0 ? undefined : true}>
            <UploadPreviewGrid previews={uploadPreviews} />
          </div>
          <span>+</span>
          <code>{snapshot.composer.uploadCount > 0 ? snapshot.composer.uploadPreviewKind : ""}</code>
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
                  onChange={(event) => onParallelPromptChange(index, event.currentTarget.value)}
                  onKeyDown={(event) => {
                    emitChatKeyColor(event);
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      sendComposer();
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
