import {
  Fragment,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import "cesium/Build/Cesium/Widgets/widgets.css";
import type { BangerGoogleTilesConfigResult, BangerPresentLoopBootstrapResult, BangerPreviewFrameResult, ComposerUploadPreview, SessionFilesGroup } from "../shared/ipc-contract";
import {
  EditImageGlyph,
  IMAGE_EDIT_STAGED_EVENT,
  ThreeUploadPreview,
  TranscriptAttachmentEventIcon,
  UploadPreview
} from "./PanelsChatBottomSlice";
import { ModuleLogo } from "./module-logos";
import { panelsChatBottomStore } from "./panels-chat-bottom-store";

interface PaneTabsProps {
  activePane: CanvasToolPane;
  openPanes: CanvasToolPane[];
  filesLabel: string;
  sessionName: string;
  sessionFilesTab?: CanvasSessionFilesTab | null;
  onActivatePane: (pane: CanvasToolPane) => void;
  onClosePane: (pane: CanvasToolPane) => void;
  onSessionFilesTabActivate?: () => void;
  onSessionFilesTabClose?: () => void;
  onSessionFilesGroupSelect?: (group: SessionFilesGroup) => void;
  onTerminalOpen: () => void;
}

export type CanvasToolPane = "files" | "terminal";
type FileKindFilter = "all" | "document" | ComposerUploadPreview["kind"];
type NativeBrowserPage = "maps" | "webexplorer";
interface MapsViewportTarget {
  target?: string;
  latitude?: number;
  longitude?: number;
}
export interface CodingLivePreviewTarget {
  path: string;
  kind: "html" | "react" | "vite" | "unknown";
  revision: number;
}

function BangerSphereNativeViewport({ searchQuery, target }: { searchQuery?: string | null; target?: MapsViewportTarget | null }) {
  const slotRef = useRef<HTMLDivElement | null>(null);
  const lastBoundsRef = useRef("");
  const [status, setStatus] = useState("Banger sphere viewport pending");
  const [frameDataUrl, setFrameDataUrl] = useState("");
  const [tilesConfig, setTilesConfig] = useState<BangerGoogleTilesConfigResult | null>(null);
  const [tilesConfigLoaded, setTilesConfigLoaded] = useState(false);
  const [cesiumFailed, setCesiumFailed] = useState(false);
  const handleCesiumFatal = useCallback(() => setCesiumFailed(true), []);
  const label = searchQuery?.replace(/\s+/g, " ").trim() || target?.target?.replace(/\s+/g, " ").trim() || "Map";

  useEffect(() => {
    const getTilesConfig = globalThis.window?.forgeShell?.getBangerGoogleTilesConfig as
      | (() => Promise<BangerGoogleTilesConfigResult>)
      | undefined;
    if (!getTilesConfig) {
      setTilesConfigLoaded(true);
      return undefined;
    }
    let active = true;
    void getTilesConfig()
      .then((result) => {
        if (active) {
          setTilesConfig(result);
          setTilesConfigLoaded(true);
        }
      })
      .catch(() => {
        if (active) {
          setTilesConfig(null);
          setTilesConfigLoaded(true);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    setCesiumFailed(false);
  }, [tilesConfig?.rootTilesetUrl]);

  useLayoutEffect(() => {
    if (!tilesConfigLoaded) {
      return undefined;
    }
    if (tilesConfig?.accepted && tilesConfig.rootTilesetUrl && !cesiumFailed) {
      setStatus("Cesium photorealistic tiles loading");
      return undefined;
    }
    const getPreview = globalThis.window?.forgeShell?.getBangerPreviewFrame as
      | (() => Promise<BangerPreviewFrameResult>)
      | undefined;
    const getBootstrap = globalThis.window?.forgeShell?.getBangerPresentLoopBootstrap as
      | ((request?: {
          x?: number;
          y?: number;
          width?: number;
          height?: number;
          sceneKind?: "maps_sphere";
        }) => Promise<BangerPresentLoopBootstrapResult>)
      | undefined;
    if (!getBootstrap) {
      if (!getPreview) {
        setStatus("Banger native bridge unavailable");
        return undefined;
      }
      let active = true;
      setStatus("Banger preview frame requested");
      void getPreview()
        .then((result) => {
          if (!active) return;
          if (result?.accepted && result.frameDataUrl) {
            setFrameDataUrl(result.frameDataUrl);
            setStatus("Banger preview frame painted");
          } else {
            setFrameDataUrl("");
            setStatus(result?.error?.message ?? "Banger preview frame unavailable");
          }
        })
        .catch((error: unknown) => {
          if (active) {
            setFrameDataUrl("");
            setStatus(error instanceof Error ? error.message : String(error));
          }
        });
      return () => {
        active = false;
      };
    }

    let animationFrame = 0;
    let retryTimer = 0;
    const settleTimers: number[] = [];
    let active = true;
    const syncBounds = () => {
      animationFrame = 0;
      const slot = slotRef.current;
      if (!slot) {
        retryTimer = window.setTimeout(scheduleSync, 80);
        return;
      }
      const rect = slot.getBoundingClientRect();
      const bounds = {
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.width),
        height: Math.round(rect.height)
      };
      if (bounds.width < 80 || bounds.height < 80) {
        retryTimer = window.setTimeout(scheduleSync, 100);
        setStatus(`Banger sphere waiting for bounds ${bounds.width}x${bounds.height}`);
        return;
      }
      const boundsKey = `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}:maps_sphere`;
      if (lastBoundsRef.current === boundsKey) {
        return;
      }
      lastBoundsRef.current = boundsKey;
      setStatus("Banger sphere native host launching");
      void getBootstrap({ ...bounds, sceneKind: "maps_sphere" })
        .then((result) => {
          if (!active) return;
          if (result?.ok) {
            setFrameDataUrl(result.previewFrameDataUrl ?? "");
            setStatus(`Banger sphere live ${result.instanceCount ?? 1} object`);
          } else {
            setFrameDataUrl("");
            setStatus(result?.error?.message ?? "Banger sphere rejected");
          }
        })
        .catch((error: unknown) => {
          if (active) {
            setFrameDataUrl("");
            setStatus(error instanceof Error ? error.message : String(error));
          }
        });
    };
    const scheduleSync = () => {
      if (animationFrame === 0) {
        animationFrame = window.requestAnimationFrame(syncBounds);
      }
    };
    const observer = new ResizeObserver(scheduleSync);
    if (slotRef.current) {
      observer.observe(slotRef.current);
    }
    window.addEventListener("resize", scheduleSync);
    scheduleSync();
    for (const delay of [80, 180, 360, 720]) {
      settleTimers.push(window.setTimeout(scheduleSync, delay));
    }
    return () => {
      active = false;
      if (animationFrame !== 0) window.cancelAnimationFrame(animationFrame);
      if (retryTimer !== 0) window.clearTimeout(retryTimer);
      for (const timer of settleTimers) window.clearTimeout(timer);
      observer.disconnect();
      window.removeEventListener("resize", scheduleSync);
    };
  }, [cesiumFailed, tilesConfig?.accepted, tilesConfig?.rootTilesetUrl, tilesConfigLoaded]);

  const tilesetEndpoint = redactedTilesetEndpoint(tilesConfig?.rootTilesetUrl);
  const georeference = tilesConfig
    ? `${tilesConfig.georeference.ellipsoid}:${tilesConfig.georeference.originLatitude.toFixed(5)}:${tilesConfig.georeference.originLongitude.toFixed(5)}`
    : "WGS84:pending";

  return (
    <div
      ref={slotRef}
      className="googleEarthDomFrame bangerSphereNativeFrame"
      aria-label={`${label} - ${status}`}
      data-tileset-schema={tilesConfig?.schema ?? "forge.banger.google_photorealistic_tiles_config.v1"}
      data-tileset-provider={tilesConfig?.provider ?? "google_photorealistic_3d_tiles"}
      data-tileset-renderer-model={tilesConfig?.rendererModel ?? "cesium_for_unreal_style_3d_tileset"}
      data-tileset-endpoint={tilesetEndpoint}
      data-tileset-georeference={georeference}
      data-tileset-lod={tilesConfig ? `${tilesConfig.lod.policy}:${tilesConfig.lod.maxScreenSpaceError}` : "screen_space_error:pending"}
      data-tileset-attribution={tilesConfig?.attribution.mode ?? "visible_on_screen"}
      data-tileset-cache={tilesConfig?.cache.authority ?? "banger_tileset_residency_cache"}
      data-native-streamer={tilesConfig?.nativeStreamer.schema ?? "forge.banger.native_3d_tiles_streamer.v1"}
      data-native-streamer-status={tilesConfig?.nativeStreamer.status ?? "pending"}
      data-native-streamer-blocker={tilesConfig?.nativeStreamer.blocker ?? "pending"}
    >
      {tilesConfig?.accepted && tilesConfig.rootTilesetUrl && !cesiumFailed ? (
        <>
          <div className="bangerSphereNativeFrame__fallback bangerSphereNativeFrame__fallback--underlay" aria-hidden="true">
            <span className="bangerSphereNativeFrame__fallbackSphere" />
          </div>
          <BangerMapsCesiumViewport
            config={tilesConfig}
            target={target}
            onStatus={setStatus}
            onFatal={handleCesiumFatal}
          />
        </>
      ) : frameDataUrl ? (
        <img className="bangerSphereNativeFrame__preview" src={frameDataUrl} alt="" draggable={false} />
      ) : (
        <div className="bangerSphereNativeFrame__fallback" aria-hidden="true">
          <span className="bangerSphereNativeFrame__fallbackSphere" />
        </div>
      )}
    </div>
  );
}

function BangerMapsCesiumViewport({
  config,
  target,
  onStatus,
  onFatal
}: {
  config: BangerGoogleTilesConfigResult;
  target?: MapsViewportTarget | null;
  onStatus: (status: string) => void;
  onFatal: () => void;
}) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const host = hostRef.current;
    if (!host || !config.accepted || !config.rootTilesetUrl) {
      return undefined;
    }
    let disposed = false;
    let readyTimer = 0;
    let viewer: import("cesium").Viewer | null = null;
    setReady(false);
    void (async () => {
      try {
        onStatus("Cesium photorealistic tiles bootstrap");
        readyTimer = window.setTimeout(() => {
          if (!disposed) {
            onStatus("Cesium tiles still loading; Banger fallback kept visible");
          }
        }, 5000);
        const Cesium = await import("cesium");
        if (disposed || !hostRef.current) return;
        const cesiumIonAccessToken = await resolveCesiumIonAccessToken(config);
        const usingCesiumIon = config.accessMode !== "google-map-tiles-api-key";
        if (cesiumIonAccessToken) {
          Cesium.Ion.defaultAccessToken = cesiumIonAccessToken;
        }
        if (usingCesiumIon) {
          onStatus(cesiumIonAccessToken ? "Cesium ion direct loading Google photorealistic 3D Tiles" : "Cesium ion default token loading Google photorealistic 3D Tiles");
        }
        if (!usingCesiumIon) {
          const googleMapsEndpoint = googleMapsEndpointFromRootTilesetUrl(config.rootTilesetUrl);
          if (googleMapsEndpoint) {
            Cesium.GoogleMaps.mapTilesApiEndpoint = googleMapsEndpoint;
            Cesium.GoogleMaps.defaultApiKey = googleMapsApiKeyFromRootTilesetUrl(config.rootTilesetUrl) ?? GOOGLE_MAPS_PROXY_KEY_ALIAS;
          }
          onStatus("Cesium direct Google Map Tiles API loading photorealistic 3D Tiles");
        }
        viewer = new Cesium.Viewer(hostRef.current, {
          animation: false,
          baseLayerPicker: false,
          fullscreenButton: false,
          geocoder: Cesium.IonGeocodeProviderType.GOOGLE,
          globe: false,
          homeButton: false,
          infoBox: false,
          navigationHelpButton: false,
          requestRenderMode: true,
          sceneModePicker: false,
          selectionIndicator: false,
          timeline: false,
          vrButton: false,
          shouldAnimate: true
        });
        if (viewer.scene.globe) {
          viewer.scene.globe.show = false;
        }
        if (viewer.scene.skyAtmosphere) {
          viewer.scene.skyAtmosphere.show = false;
        }
        if (viewer.scene.skyBox) {
          viewer.scene.skyBox.show = false;
        }
        viewer.scene.backgroundColor = Cesium.Color.TRANSPARENT;
        onStatus("Cesium loading Google photorealistic 3D Tiles");
        const tileset = await Cesium.createGooglePhotorealistic3DTileset(usingCesiumIon ? {
          onlyUsingWithGoogleGeocoder: true
        } : {
          key: GOOGLE_MAPS_PROXY_KEY_ALIAS,
          onlyUsingWithGoogleGeocoder: true
        }, {
          maximumScreenSpaceError: config.lod.maxScreenSpaceError,
          skipLevelOfDetail: config.lod.skipLevelOfDetail,
          showCreditsOnScreen: config.showCreditsOnScreen
        });
        if (disposed || !viewer || viewer.isDestroyed()) return;
        viewer.scene.primitives.add(tileset);
        const longitude = typeof target?.longitude === "number" ? target.longitude : config.initialView.longitude;
        const latitude = typeof target?.latitude === "number" ? target.latitude : config.initialView.latitude;
        viewer.camera.setView({
          destination: Cesium.Cartesian3.fromDegrees(longitude, latitude, config.initialView.heightMeters),
          orientation: {
            heading: Cesium.Math.toRadians(config.initialView.headingDegrees),
            pitch: Cesium.Math.toRadians(config.initialView.pitchDegrees),
            roll: Cesium.Math.toRadians(config.initialView.rollDegrees)
          }
        });
        void viewer.zoomTo(tileset);
        viewer.scene.requestRender();
        setReady(true);
        onStatus("Cesium Google photorealistic 3D Tiles live");
      } catch (error) {
        console.warn("Banger Maps Cesium tileset failed; falling back to native sphere.", error);
        if (!disposed) {
          onStatus(error instanceof Error ? error.message : "Cesium photorealistic tiles unavailable");
          onFatal();
        }
      }
    })();
    return () => {
      disposed = true;
      if (readyTimer !== 0) {
        window.clearTimeout(readyTimer);
      }
      if (viewer && !viewer.isDestroyed()) {
        viewer.destroy();
      }
    };
  }, [config, onFatal, onStatus, target?.latitude, target?.longitude]);

  return <div ref={hostRef} className={ready ? "bangerMapsCesiumViewport bangerMapsCesiumViewport--ready" : "bangerMapsCesiumViewport"} aria-hidden="true" />;
}

function redactedTilesetEndpoint(value?: string): string {
  if (!value) {
    return "pending";
  }
  try {
    const url = new URL(value);
    if (url.searchParams.has("key")) {
      url.search = "?key=redacted";
    }
    return url.toString();
  } catch {
    return "configured";
  }
}

const GOOGLE_MAPS_PROXY_KEY_ALIAS = "render-proxy";

async function resolveCesiumIonAccessToken(config: BangerGoogleTilesConfigResult): Promise<string | undefined> {
  const embedded = config.cesiumIonAccessToken?.trim();
  if (embedded) {
    return embedded;
  }
  if (!config.cesiumIonAccessTokenUrl) {
    return undefined;
  }
  try {
    const response = await fetch(config.cesiumIonAccessTokenUrl, { cache: "no-store" });
    if (!response.ok) {
      return undefined;
    }
    const payload = await response.json() as { accepted?: unknown; token?: unknown };
    const token = typeof payload.token === "string" ? payload.token.trim() : "";
    return payload.accepted === true && token ? token : undefined;
  } catch {
    return undefined;
  }
}

function googleMapsEndpointFromRootTilesetUrl(value: string): string | undefined {
  try {
    const url = new URL(value);
    url.search = "";
    url.hash = "";
    url.pathname = url.pathname
      .replace(/\/v1\/3dtiles\/root\.json$/i, "/")
      .replace(/\/root\.json$/i, "/");
    if (!url.pathname.endsWith("/")) {
      url.pathname = `${url.pathname}/`;
    }
    return url.toString();
  } catch {
    return undefined;
  }
}

function googleMapsApiKeyFromRootTilesetUrl(value: string): string | undefined {
  try {
    const url = new URL(value);
    return url.searchParams.get("key")?.trim() || undefined;
  } catch {
    return undefined;
  }
}

interface CanvasSessionFilesTab {
  sessionId: string;
  sessionName: string;
  filesLabel: string;
  active: boolean;
}

const FILE_KIND_FILTERS: Array<{ id: FileKindFilter; label: string; kinds?: ComposerUploadPreview["kind"][] }> = [
  { id: "all", label: "Tous les fichiers" },
  { id: "image", label: "Images et photos" },
  { id: "video", label: "Videos" },
  { id: "model3d", label: "Objets 3D" },
  { id: "document", label: "Documents", kinds: ["pdf", "spreadsheet", "text"] },
  { id: "chart", label: "Graphiques" },
  { id: "file", label: "Autres fichiers" }
];
function TerminalGlyph({ className = "canvasSplitIcon" }: { className?: string }) {
  return (
    <svg className={className} xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 24 24" aria-hidden="true">
      <g fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5">
        <rect width="18.5" height="15.5" x="2.75" y="4.25" rx="3.5" />
        <path d="m7.25 9l3 3l-3 3m5.5 0h4" />
      </g>
    </svg>
  );
}

function GoogleWordmarkMono() {
  return (
    <svg className="webExplorerWordmark" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 272 92" aria-hidden="true" focusable="false">
      <path fill="currentColor" d="M115.75 47.18c0 12.77-9.99 22.18-22.25 22.18s-22.25-9.41-22.25-22.18C71.25 34.32 81.24 25 93.5 25s22.25 9.32 22.25 22.18zm-9.74 0c0-7.98-5.79-13.44-12.51-13.44S80.99 39.2 80.99 47.18c0 7.9 5.79 13.44 12.51 13.44s12.51-5.55 12.51-13.44z" />
      <path fill="currentColor" d="M163.75 47.18c0 12.77-9.99 22.18-22.25 22.18s-22.25-9.41-22.25-22.18c0-12.85 9.99-22.18 22.25-22.18s22.25 9.32 22.25 22.18zm-9.74 0c0-7.98-5.79-13.44-12.51-13.44s-12.51 5.46-12.51 13.44c0 7.9 5.79 13.44 12.51 13.44s12.51-5.55 12.51-13.44z" />
      <path fill="currentColor" d="M209.75 26.34v39.82c0 16.38-9.66 23.07-21.08 23.07-10.75 0-17.22-7.19-19.66-13.07l8.48-3.53c1.51 3.61 5.21 7.87 11.17 7.87 7.31 0 11.84-4.51 11.84-13v-3.19h-.34c-2.18 2.69-6.38 5.04-11.68 5.04-11.09 0-21.25-9.66-21.25-22.09 0-12.52 10.16-22.26 21.25-22.26 5.29 0 9.49 2.35 11.68 4.96h.34v-3.61h9.25zm-8.56 20.92c0-7.81-5.21-13.52-11.84-13.52-6.72 0-12.35 5.71-12.35 13.52 0 7.73 5.63 13.36 12.35 13.36 6.63 0 11.84-5.63 11.84-13.36z" />
      <path fill="currentColor" d="M225 3v65h-9.5V3h9.5z" />
      <path fill="currentColor" d="M262.02 54.48l7.56 5.04c-2.44 3.61-8.32 9.83-18.48 9.83-12.6 0-22.01-9.74-22.01-22.18 0-13.19 9.49-22.18 20.92-22.18 11.51 0 17.14 9.16 18.98 14.11l1.01 2.52-29.65 12.28c2.27 4.45 5.8 6.72 10.75 6.72 4.96 0 8.4-2.44 10.92-6.14zm-23.27-7.98l19.82-8.23c-1.09-2.77-4.37-4.7-8.23-4.7-4.95 0-11.84 4.37-11.59 12.93z" />
      <path fill="currentColor" d="M35.29 41.41V32H67c.31 1.64.47 3.58.47 5.68 0 7.06-1.93 15.79-8.15 22.01-6.05 6.3-13.78 9.66-24.02 9.66C16.32 69.35.36 53.89.36 34.91.36 15.93 16.32.47 35.3.47c10.5 0 17.98 4.12 23.6 9.49l-6.64 6.64c-4.03-3.78-9.49-6.72-16.97-6.72-13.86 0-24.7 11.17-24.7 25.03 0 13.86 10.84 25.03 24.7 25.03 8.99 0 14.11-3.61 17.39-6.89 2.66-2.66 4.41-6.46 5.1-11.65l-22.49.01z" />
    </svg>
  );
}

function fileUrlFromPreviewPath(path: string): string | null {
  const trimmed = path.trim();
  if (!trimmed) {
    return null;
  }
  if (/^https?:\/\//i.test(trimmed)) {
    return trimmed;
  }
  const normalized = trimmed.replace(/\\/g, "/");
  const drivePath = normalized.match(/^([A-Za-z]):\/(.+)$/);
  if (drivePath) {
    const body = drivePath[2].split("/").map(encodeURIComponent).join("/");
    return `file:///${drivePath[1]}:/${body}`;
  }
  if (normalized.startsWith("//")) {
    return `file:${normalized.split("/").map((part, index) => index < 2 ? part : encodeURIComponent(part)).join("/")}`;
  }
  if (normalized.startsWith("/")) {
    return `file://${normalized.split("/").map((part, index) => index === 0 ? part : encodeURIComponent(part)).join("/")}`;
  }
  return null;
}

function CodingLivePreviewFrame({ preview }: { preview: CodingLivePreviewTarget }) {
  const [reloadTick, setReloadTick] = useState(0);
  const fileUrl = useMemo(() => fileUrlFromPreviewPath(preview.path), [preview.path]);
  useEffect(() => {
    if (!fileUrl) {
      return undefined;
    }
    const interval = window.setInterval(() => setReloadTick((tick) => tick + 1), 1400);
    return () => window.clearInterval(interval);
  }, [fileUrl]);
  const src = fileUrl ? `${fileUrl}${fileUrl.includes("?") ? "&" : "?"}ingenPreview=${preview.revision}-${reloadTick}` : "";
  const fileName = preview.path.split(/[\\/]/).filter(Boolean).at(-1) || preview.path;
  return (
    <div className="codingLivePreview" aria-label="Coding live preview">
      <div className="codingLivePreview__meta" aria-hidden="true">
        <span>{preview.kind === "unknown" ? "Live Preview" : `${preview.kind.toUpperCase()} Preview`}</span>
        <code>{fileName}</code>
      </div>
      {fileUrl ? (
        <iframe
          key={src}
          className="codingLivePreview__frame"
          src={src}
          sandbox="allow-scripts allow-forms allow-modals"
          referrerPolicy="no-referrer"
          title={`Live preview ${fileName}`}
        />
      ) : (
        <div className="codingLivePreview__empty">
          <strong>Preview path unavailable</strong>
          <span>{preview.path}</span>
        </div>
      )}
    </div>
  );
}

function GoogleEarthIcon() {
  return (
    <svg className="nativeBrowserPager__earth" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id="google-earth-ocean" x1="6" y1="5" x2="27" y2="28" gradientUnits="userSpaceOnUse">
          <stop stopColor="#46a3ff" />
          <stop offset="1" stopColor="#1a57d6" />
        </linearGradient>
      </defs>
      <circle cx="16" cy="16" r="13.5" fill="url(#google-earth-ocean)" />
      <path fill="#34A853" d="M5.8 13.7c2.3-5.1 7.1-8.5 12.2-8.2c-1.5 1.4-2.9 3-4.1 4.9c-1.5 2.4-3.6 3.2-8.1 3.3Zm7.3 12.2c-3.5-.7-6.4-3-7.8-6.1c2.3-.4 4.7-.1 7.3.9c2.6 1.1 5.4.9 8.5-.7c-1.6 3.3-4.3 5.7-8 5.9Z" />
      <path fill="#FBBC04" d="M23.6 7.9c3.1 2.2 5.1 5.8 4.9 9.8c-3-1.6-5.7-2.1-8.1-1.4c-2.1.6-4.1.2-6.2-1.1c2.6-2.2 5.6-4.8 9.4-7.3Z" />
      <path fill="#EA4335" d="M24.7 23.6c-2.1 2.4-5.2 3.9-8.7 3.9c1.3-1.4 2.4-3.1 3.3-5c1.5-3 4.7-3.4 8.1-2.4a11.7 11.7 0 0 1-2.7 3.5Z" />
      <path fill="none" stroke="rgba(255,255,255,.8)" strokeWidth="1.6" d="M4.6 18.4c7.4-2.7 14.9-2 22.8 2.1" />
    </svg>
  );
}

function NativeBrowserPager({
  activePage,
  onPageChange,
  webExplorerModuleId
}: {
  activePage: NativeBrowserPage;
  onPageChange: (page: NativeBrowserPage) => void;
  webExplorerModuleId?: string | null;
}) {
  const webLabel = webExplorerModuleId === "airbnb" ? "Airbnb" : "WebExplorer";
  return (
    <div className="nativeBrowserPager" aria-label="WebExplorer pages">
      <button
        type="button"
        className={activePage === "maps" ? "nativeBrowserPager__button nativeBrowserPager__button--active" : "nativeBrowserPager__button"}
        aria-label="Open Map page"
        aria-pressed={activePage === "maps"}
        title="Map"
        onClick={() => onPageChange("maps")}
      >
        <GoogleEarthIcon />
      </button>
      <button
        type="button"
        className={activePage === "webexplorer" ? "nativeBrowserPager__button nativeBrowserPager__button--active" : "nativeBrowserPager__button"}
        aria-label={`Open ${webLabel} page`}
        aria-pressed={activePage === "webexplorer"}
        title={webLabel}
        onClick={() => onPageChange("webexplorer")}
      >
        {webExplorerModuleId === "airbnb" ? <ModuleLogo id="airbnb" /> : <span className="shellIcon shellIcon--google" aria-hidden="true" />}
      </button>
    </div>
  );
}

type PlanetKind = "mercury" | "venus" | "earth" | "moon" | "mars" | "jupiter" | "saturn" | "uranus" | "neptune";

const PLANET_SNAPSHOT_URLS: Record<PlanetKind, string> = {
  mercury: "/shell-assets/planet-snapshots/mercury.svg",
  venus: "/shell-assets/planet-snapshots/venus.svg",
  earth: "/shell-assets/planet-snapshots/earth.svg",
  moon: "/shell-assets/planet-snapshots/moon.svg",
  mars: "/shell-assets/planet-snapshots/mars.svg",
  jupiter: "/shell-assets/planet-snapshots/jupiter.svg",
  saturn: "/shell-assets/planet-snapshots/saturn.svg",
  uranus: "/shell-assets/planet-snapshots/uranus.svg",
  neptune: "/shell-assets/planet-snapshots/neptune.svg"
};

function PlanetSnapshot({ planet }: { planet: PlanetKind }) {
  return <img className={`canvasPlanetSphere__svg canvasPlanetSphere__svg--${planet}`} src={PLANET_SNAPSHOT_URLS[planet]} alt="" decoding="async" draggable={false} />;
}

function CanvasPlanetRail() {
  const planets: Array<{ key: PlanetKind; label: string }> = [
    { key: "mercury", label: "Mercury" },
    { key: "venus", label: "Venus" },
    { key: "earth", label: "Earth" },
    { key: "moon", label: "Moon" },
    { key: "mars", label: "Mars" },
    { key: "jupiter", label: "Jupiter" },
    { key: "saturn", label: "Saturn" },
    { key: "uranus", label: "Uranus" },
    { key: "neptune", label: "Neptune" }
  ];

  return (
    <aside className="canvasPlanetRail" aria-label="3D planets">
      {planets.map((planet, index) => (
        <button type="button" className="canvasPlanetSphere" key={planet.key} style={{ "--planet-delay": `${40 + index * 90}ms` } as CSSProperties}>
          <PlanetSnapshot planet={planet.key} />
          <span>{planet.label}</span>
        </button>
      ))}
    </aside>
  );
}

function PaneTabs({
  activePane,
  openPanes,
  filesLabel,
  sessionName,
  sessionFilesTab,
  onActivatePane,
  onClosePane,
  onSessionFilesTabActivate,
  onSessionFilesTabClose,
  onSessionFilesGroupSelect,
  onTerminalOpen
}: PaneTabsProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [sessionFilesOpen, setSessionFilesOpen] = useState(false);
  const [sessionFilesExpanded, setSessionFilesExpanded] = useState(false);
  const [sessionFilesGroups, setSessionFilesGroups] = useState<SessionFilesGroup[]>([]);
  const [sessionFilesLoading, setSessionFilesLoading] = useState(false);
  const [sessionFilesError, setSessionFilesError] = useState("");

  const choose = (action: () => void) => {
    setMenuOpen(false);
    setSessionFilesOpen(false);
    setSessionFilesExpanded(false);
    action();
  };

  const chooseSessionFilesGroup = (group: SessionFilesGroup) => {
    choose(() => onSessionFilesGroupSelect?.(group));
  };

  const openSessionFiles = () => {
    if (sessionFilesOpen) {
      return;
    }
    setSessionFilesOpen(true);
    setSessionFilesExpanded(false);
    setSessionFilesLoading(true);
    setSessionFilesError("");
    void globalThis.window?.forgeShell?.getSessionFilesSnapshot?.()
      .then((snapshot) => {
        setSessionFilesGroups(snapshot?.groups ?? []);
      })
      .catch((error: unknown) => {
        setSessionFilesError(error instanceof Error ? error.message : "Session files unavailable.");
      })
      .finally(() => setSessionFilesLoading(false));
  };

  useEffect(() => {
    if (!sessionFilesOpen) {
      return undefined;
    }
    const expandTimer = window.setTimeout(() => setSessionFilesExpanded(true), 230);
    return () => window.clearTimeout(expandTimer);
  }, [sessionFilesOpen]);

  const menuClassName = [
    "canvasPaneTabs__menu",
    sessionFilesOpen ? "canvasPaneTabs__menu--sessionFiles" : "",
    sessionFilesExpanded ? "canvasPaneTabs__menu--sessionFilesExpanded" : ""
  ].filter(Boolean).join(" ");

  return (
    <div className="canvasPaneTabs" role="tablist" aria-label="Canvas tool tabs">
      <div className="canvasPaneTabs__tabs">
        {openPanes.map((pane, index) => {
          const selected = pane === activePane && !(pane === "files" && sessionFilesTab?.active);
          const title = pane === "files" ? "Files" : "Terminal";
          const detail = pane === "files" ? filesLabel : sessionName;
          return (
            <Fragment key={pane}>
              <div className={selected ? "canvasPaneTabs__tab canvasPaneTabs__tab--active" : "canvasPaneTabs__tab"} role="presentation">
                <button
                  type="button"
                  id={`canvas-${pane}-tab`}
                  className="canvasPaneTabs__tabButton"
                  role="tab"
                  aria-selected={selected}
                  aria-controls={`canvas-${pane}-pane`}
                  tabIndex={selected ? 0 : -1}
                  onClick={() => onActivatePane(pane)}
                >
                  <span className="canvasPaneTabs__title">{title}</span>
                  <span className="canvasPaneTabs__session">{detail}</span>
                </button>
                <button type="button" className="canvasPaneTabs__close" aria-label={`Close ${title}`} onClick={() => onClosePane(pane)}>x</button>
              </div>
              {pane === "files" && sessionFilesTab ? (
                <div className={sessionFilesTab.active ? "canvasPaneTabs__tab canvasPaneTabs__tab--active canvasPaneTabs__tab--sessionFiles" : "canvasPaneTabs__tab canvasPaneTabs__tab--sessionFiles"} role="presentation">
                  <button
                    type="button"
                    id="canvas-session-files-tab"
                    className="canvasPaneTabs__tabButton"
                    role="tab"
                    aria-selected={sessionFilesTab.active}
                    aria-controls="canvas-files-pane"
                    tabIndex={sessionFilesTab.active ? 0 : -1}
                    onClick={onSessionFilesTabActivate}
                  >
                    <span className="canvasPaneTabs__title">{sessionFilesTab.sessionName}</span>
                    <span className="canvasPaneTabs__session">{sessionFilesTab.filesLabel}</span>
                  </button>
                  <button type="button" className="canvasPaneTabs__close" aria-label={`Close ${sessionFilesTab.sessionName} files`} onClick={onSessionFilesTabClose}>x</button>
                </div>
              ) : null}
              {index === 0 ? (
                <div className="canvasPaneTabs__addWrap">
                  <button
                    type="button"
                    className="canvasPaneTabs__add"
                    aria-label="Open another canvas tool"
                    aria-expanded={menuOpen}
                    onClick={() => setMenuOpen((open) => !open)}
                  >
                    +
                  </button>
                  {menuOpen ? (
                    <div className={menuClassName} role="menu">
                      <button type="button" role="menuitem" aria-label="Open files from another session" onClick={openSessionFiles}>
                        <span className="shellIcon shellIcon--assets" aria-hidden="true" />
                        <span>Files from Another Session</span>
                      </button>
                      {sessionFilesOpen ? (
                        <SessionFilesMenuBody groups={sessionFilesGroups} loading={sessionFilesLoading} error={sessionFilesError} onSelectGroup={chooseSessionFilesGroup} />
                      ) : (
                        <button type="button" role="menuitem" onClick={() => choose(onTerminalOpen)}>
                          <TerminalGlyph className="canvasPaneTabs__menuIcon" />
                          <span>Open Terminal</span>
                        </button>
                      )}
                    </div>
                  ) : null}
                </div>
              ) : null}
            </Fragment>
          );
        })}
      </div>
    </div>
  );
}

function SessionFilesMenuBody({
  groups,
  loading,
  error,
  onSelectGroup
}: {
  groups: SessionFilesGroup[];
  loading: boolean;
  error: string;
  onSelectGroup: (group: SessionFilesGroup) => void;
}) {
  if (loading) {
    return <div className="canvasSessionFilesMenu__status">Loading session files...</div>;
  }
  if (error) {
    return <div className="canvasSessionFilesMenu__status">{error}</div>;
  }
  if (groups.length === 0) {
    return <div className="canvasSessionFilesMenu__status">No session files yet.</div>;
  }
  return (
    <div className="canvasSessionFilesMenu" role="menu" aria-label="Files grouped by session">
      {groups.map((group) => (
        <button
          type="button"
          className="canvasSessionFilesChoice"
          key={group.sessionId}
          role="menuitem"
          aria-label={`Open ${group.sessionName} files`}
          onClick={() => onSelectGroup(group)}
        >
          <span className="canvasSessionFilesChoice__previews" aria-hidden="true">
            {group.files.slice(0, 4).map((file, index) => (
              <span className={`canvasSessionFilesChoice__preview canvasSessionFilesChoice__preview--${file.kind}`} key={`${group.sessionId}-${file.id}-${index}`}>
                <CanvasFilePreview file={file} compact activeMotionPreview />
              </span>
            ))}
          </span>
          <span className="canvasSessionFilesChoice__meta">
            <strong>{group.sessionName}</strong>
            <span>{group.files.length === 1 ? "1 file" : `${group.files.length} files`}</span>
          </span>
        </button>
      ))}
    </div>
  );
}

function stageCanvasImageForEdit(file: ComposerUploadPreview) {
  void panelsChatBottomStore.dispatch({ kind: "stage_attachment_for_edit", attachmentIds: [file.id] }).then(() => {
    globalThis.window?.dispatchEvent(new CustomEvent(IMAGE_EDIT_STAGED_EVENT, { detail: { attachmentId: file.id } }));
  });
}

function CanvasFileFallbackPreview({ file }: { file: ComposerUploadPreview }) {
  return (
    <div className={`canvasFileTile__visualFallback canvasFileTile__visualFallback--${file.kind}`}>
      <span className="canvasFileTile__visualFallbackIcon" aria-hidden="true">
        <TranscriptAttachmentEventIcon kind={file.kind} />
      </span>
      <span className="canvasFileTile__visualFallbackName">{file.name}</span>
    </div>
  );
}

function CanvasFilePreview({
  file,
  compact = false,
  activeMotionPreview = false
}: {
  file: ComposerUploadPreview;
  compact?: boolean;
  activeMotionPreview?: boolean;
}) {
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setFailed(false);
  }, [file.id, file.url]);

  if (failed) {
    return <CanvasFileFallbackPreview file={file} />;
  }
  if (file.kind === "image") {
    return <img className="canvasFileTile__media" src={file.url} alt={file.name} draggable={false} onError={() => setFailed(true)} />;
  }
  if (file.kind === "video") {
    return (
      <video
        className="canvasFileTile__media"
        src={file.url}
        muted
        playsInline
        autoPlay={activeMotionPreview}
        loop={activeMotionPreview}
        preload={activeMotionPreview ? "auto" : "metadata"}
        onError={() => setFailed(true)}
      />
    );
  }
  if (compact && file.kind === "model3d" && activeMotionPreview) {
    return (
      <div className="canvasFileTile__model canvasFileTile__model--sessionChoice">
        <ThreeUploadPreview preview={file} />
      </div>
    );
  }
  if (compact) {
    return <CanvasFileFallbackPreview file={file} />;
  }
  if (file.kind === "model3d") {
    return (
      <div className="canvasFileTile__model">
        <UploadPreview preview={file} />
      </div>
    );
  }
  return (
    <div className="canvasFileTile__document">
      <UploadPreview preview={file} />
    </div>
  );
}

function AllFilesIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <rect x="4" y="4" width="6.2" height="6.2" rx="1.4" />
      <rect x="13.8" y="4" width="6.2" height="6.2" rx="1.4" />
      <rect x="4" y="13.8" width="6.2" height="6.2" rx="1.4" />
      <rect x="13.8" y="13.8" width="6.2" height="6.2" rx="1.4" />
    </svg>
  );
}

function EmptyFilesIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M4.65 6.28 12 3.92l7.35 2.36 2.72 4.69-5.65 1.81-1.72-2.98L12 10.67 9.3 9.8l-1.72 2.98-5.65-1.81 2.72-4.69Z" />
      <path d="M5.1 12.26v4.62c0 .68.44 1.28 1.09 1.49L12 20.23l5.81-1.86a1.57 1.57 0 0 0 1.09-1.49v-4.62" />
      <path d="M12 10.67v9.26" />
      <path d="m4.65 6.28 7.35 2.36 7.35-2.36" />
    </svg>
  );
}

function fileKindIconKind(kind: FileKindFilter): ComposerUploadPreview["kind"] {
  if (kind === "document") return "pdf";
  if (kind === "all") return "file";
  return kind;
}

function CanvasFilesPane({
  sessionName,
  files,
  activePane,
  openPanes,
  onActivatePane,
  onClosePane,
  onTerminalOpen
}: {
  sessionName: string;
  files: ComposerUploadPreview[];
  activePane: CanvasToolPane;
  openPanes: CanvasToolPane[];
  onActivatePane: (pane: CanvasToolPane) => void;
  onClosePane: (pane: CanvasToolPane) => void;
  onTerminalOpen: () => void;
}) {
  const [query, setQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<FileKindFilter>("all");
  const [selectedSessionFilesGroup, setSelectedSessionFilesGroup] = useState<SessionFilesGroup | null>(null);
  const [sessionFilesTabActive, setSessionFilesTabActive] = useState(false);
  const activeFiles = sessionFilesTabActive && selectedSessionFilesGroup ? selectedSessionFilesGroup.files : files;
  const activeFilesSessionName = sessionFilesTabActive && selectedSessionFilesGroup ? selectedSessionFilesGroup.sessionName : sessionName;
  const fileCountLabel = files.length === 1 ? "1 file" : `${files.length} files`;
  const sessionFilesTab = selectedSessionFilesGroup
    ? {
        sessionId: selectedSessionFilesGroup.sessionId,
        sessionName: selectedSessionFilesGroup.sessionName,
        filesLabel: selectedSessionFilesGroup.files.length === 1 ? "1 file" : `${selectedSessionFilesGroup.files.length} files`,
        active: sessionFilesTabActive
      }
    : null;
  const fileKindCounts = useMemo(() => {
    const counts = new Map<FileKindFilter, number>();
    counts.set("all", activeFiles.length);
    for (const file of activeFiles) {
      counts.set(file.kind, (counts.get(file.kind) ?? 0) + 1);
      if (file.kind === "pdf" || file.kind === "spreadsheet" || file.kind === "text") {
        counts.set("document", (counts.get("document") ?? 0) + 1);
      }
    }
    return counts;
  }, [activeFiles]);
  const visibleFiles = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return activeFiles.filter((file) => {
      const activeFilter = FILE_KIND_FILTERS.find((filter) => filter.id === kindFilter);
      const acceptedKinds = activeFilter?.kinds ?? (kindFilter === "all" ? undefined : [kindFilter as ComposerUploadPreview["kind"]]);
      if (acceptedKinds && !acceptedKinds.includes(file.kind)) {
        return false;
      }
      if (!normalized) {
        return true;
      }
      return `${file.name} ${file.kind}`.toLowerCase().includes(normalized);
    });
  }, [activeFiles, kindFilter, query]);

  const selectSessionFilesGroup = (group: SessionFilesGroup) => {
    setSelectedSessionFilesGroup(group);
    setSessionFilesTabActive(true);
    setQuery("");
    setKindFilter("all");
  };

  return (
    <aside id="canvas-files-pane" className="canvasFilesPane" aria-label="Project files" role="tabpanel" aria-labelledby="canvas-files-tab">
      <div className="canvasFilesPane__inner">
        <PaneTabs
          activePane={activePane}
          openPanes={openPanes}
          filesLabel={fileCountLabel}
          sessionName={sessionName}
          sessionFilesTab={sessionFilesTab}
          onActivatePane={(pane) => {
            if (pane === "files") {
              setSessionFilesTabActive(false);
            }
            onActivatePane(pane);
          }}
          onClosePane={onClosePane}
          onSessionFilesTabActivate={() => setSessionFilesTabActive(true)}
          onSessionFilesTabClose={() => {
            setSessionFilesTabActive(false);
            setSelectedSessionFilesGroup(null);
          }}
          onSessionFilesGroupSelect={selectSessionFilesGroup}
          onTerminalOpen={onTerminalOpen}
        />
        <label className="canvasFilesPane__search">
          <span className="shellIcon shellIcon--search" aria-hidden="true" />
          <input
            type="search"
            placeholder="search"
            aria-label="Search files"
            value={query}
            onChange={(event) => setQuery(event.currentTarget.value)}
          />
        </label>
        <div className="canvasFilesPane__filters" aria-label="Classer les fichiers par type">
          {FILE_KIND_FILTERS.map((filter) => {
            const count = fileKindCounts.get(filter.id) ?? 0;
            const selected = kindFilter === filter.id;
            return (
              <button
                type="button"
                className={selected ? "canvasFileTypeButton canvasFileTypeButton--active" : "canvasFileTypeButton"}
                aria-label={`${filter.label}: ${count}`}
                aria-pressed={selected}
                disabled={count === 0}
                key={filter.id}
                title={`${filter.label} (${count})`}
                onClick={() => setKindFilter(filter.id)}
              >
                {filter.id === "all" ? <AllFilesIcon /> : <TranscriptAttachmentEventIcon kind={fileKindIconKind(filter.id)} />}
                <span>{count}</span>
              </button>
            );
          })}
        </div>
        <div
          className={visibleFiles.length > 0 ? "canvasFilesPane__list" : "canvasFilesPane__list canvasFilesPane__list--empty"}
          role="list"
          aria-label={`${activeFilesSessionName} files`}
          key={sessionFilesTabActive && selectedSessionFilesGroup ? selectedSessionFilesGroup.sessionId : "current-session-files"}
        >
          {visibleFiles.length > 0 ? (
            visibleFiles.map((file, index) => (
              <figure
                className={`canvasFileTile canvasFileTile--${file.kind} canvasFileTile--shape${index % 5}`}
                role="listitem"
                key={file.id}
                style={{ "--canvas-file-enter-delay": `${Math.min(index, 12) * 46}ms` } as CSSProperties}
                title={file.name}
              >
                <div className="canvasFileTile__preview">
                  <CanvasFilePreview file={file} />
                  {file.kind === "image" ? (
                    <button
                      type="button"
                      className="imageEditButton imageEditButton--file"
                      aria-label={`Modifier ${file.name}`}
                      title="Modifier l'image"
                      onClick={(event) => {
                        event.stopPropagation();
                        stageCanvasImageForEdit(file);
                      }}
                    >
                      <EditImageGlyph />
                    </button>
                  ) : null}
                </div>
                <figcaption className="canvasFileTile__caption">
                  <strong>{file.name}</strong>
                  <span className="canvasFileTile__captionIcon" aria-label={file.kind} title={file.kind}>
                    <TranscriptAttachmentEventIcon kind={file.kind} />
                  </span>
                </figcaption>
              </figure>
            ))
          ) : (
            <div className="canvasFilesPane__empty" role="status">
              <EmptyFilesIcon />
              <strong>{sessionFilesTabActive ? "No files in this selected session." : "No files yet in this session."}</strong>
            </div>
          )}
        </div>
      </div>
    </aside>
  );
}

function CanvasTerminalPane({
  sessionName,
  activePane,
  openPanes,
  filesLabel,
  onActivatePane,
  onClosePane,
  onTerminalOpen
}: {
  sessionName: string;
  activePane: CanvasToolPane;
  openPanes: CanvasToolPane[];
  filesLabel: string;
  onActivatePane: (pane: CanvasToolPane) => void;
  onClosePane: (pane: CanvasToolPane) => void;
  onTerminalOpen: () => void;
}) {
  const terminalSlotRef = useRef<HTMLDivElement>(null);
  const [terminalError, setTerminalError] = useState("");

  useEffect(() => {
    const api = globalThis.window?.forgeShell;
    const slot = terminalSlotRef.current;
    if (!api?.showNativeTerminal || !api.updateNativeTerminalBounds || !api.hideNativeTerminal || !slot) {
      setTerminalError("Native terminal API unavailable.");
      return undefined;
    }
    const showNativeTerminal = api.showNativeTerminal;
    const updateNativeTerminalBounds = api.updateNativeTerminalBounds;
    const hideNativeTerminal = api.hideNativeTerminal;

    let animationFrame = 0;
    let firstSync = true;
    const syncBounds = () => {
      animationFrame = 0;
      const rect = slot.getBoundingClientRect();
      const bounds = {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height
      };
      if (bounds.width < 120 || bounds.height < 80) {
        return;
      }
      const command = firstSync ? showNativeTerminal : updateNativeTerminalBounds;
      firstSync = false;
      void command(bounds).then((result) => {
        setTerminalError(result.accepted ? "" : result.error?.message ?? "Native terminal failed.");
      });
    };
    const scheduleSync = () => {
      if (animationFrame === 0) {
        animationFrame = window.requestAnimationFrame(syncBounds);
      }
    };
    const observer = new ResizeObserver(scheduleSync);
    observer.observe(slot);
    window.addEventListener("resize", scheduleSync);
    scheduleSync();

    return () => {
      if (animationFrame !== 0) {
        window.cancelAnimationFrame(animationFrame);
      }
      observer.disconnect();
      window.removeEventListener("resize", scheduleSync);
      void hideNativeTerminal();
    };
  }, []);

  return (
    <aside id="canvas-terminal-pane" className="canvasTerminalPane" aria-label="Terminal" role="tabpanel" aria-labelledby="canvas-terminal-tab">
      <div className="canvasTerminalPane__inner">
        <PaneTabs
          activePane={activePane}
          openPanes={openPanes}
          filesLabel={filesLabel}
          sessionName={sessionName}
          onActivatePane={onActivatePane}
          onClosePane={onClosePane}
          onTerminalOpen={onTerminalOpen}
        />
        <div ref={terminalSlotRef} className="canvasTerminalPane__nativeHost" aria-label="Embedded Windows PowerShell">
          {terminalError ? <p className="canvasTerminalPane__nativeError">{terminalError}</p> : null}
        </div>
      </div>
    </aside>
  );
}

export function CanvasSurfacesSlice({
  split,
  actionsOpen,
  filesOpen,
  terminalOpen,
  activePane,
  planetsOpen,
  webExplorerOpen,
  webExplorerParallelIndex = 0,
  webExplorerModuleId = null,
  mapsOpen,
  mapsClosing = false,
  mapsParallelIndex = 0,
  mapsTarget = null,
  mapsSearchQuery,
  codingLivePreview,
  leftPanelOpen,
  parallelPrompts,
  removableParallelIndexes,
  sessionFiles,
  sessionName,
  onFilesOpen,
  onFilesClose,
  onTerminalOpen,
  onTerminalClose,
  onActivePaneChange,
  onPlanetsOpen,
  onPlanetsClose,
  onWebExplorerOpen,
  onWebExplorerClose,
  onMapsClose,
  onCodingLivePreviewClose,
  onParallelAdd,
  onParallelRemove
}: {
  split: boolean;
  actionsOpen: boolean;
  filesOpen: boolean;
  terminalOpen: boolean;
  activePane: CanvasToolPane | "";
  planetsOpen: boolean;
  webExplorerOpen: boolean;
  webExplorerParallelIndex?: number;
  webExplorerModuleId?: string | null;
  mapsOpen: boolean;
  mapsClosing?: boolean;
  mapsParallelIndex?: number;
  mapsUrl?: string;
  mapsTarget?: MapsViewportTarget | null;
  mapsSearchQuery?: string;
  codingLivePreview?: CodingLivePreviewTarget | null;
  leftPanelOpen: boolean;
  parallelPrompts: string[];
  removableParallelIndexes: boolean[];
  sessionFiles: ComposerUploadPreview[];
  sessionName: string;
  onFilesOpen: () => void;
  onFilesClose: () => void;
  onTerminalOpen: () => void;
  onTerminalClose: () => void;
  onActivePaneChange: (pane: CanvasToolPane) => void;
  onPlanetsOpen: () => void;
  onPlanetsClose: () => void;
  onWebExplorerOpen: () => void;
  onWebExplorerClose: () => void;
  onMapsClose: () => void;
  onCodingLivePreviewClose: () => void;
  onParallelAdd: () => void;
  onParallelRemove: (index: number) => void;
}) {
  const nativeWebExplorerSlotRef = useRef<HTMLDivElement>(null);
  const [nativeWebExplorerAccepted, setNativeWebExplorerAccepted] = useState(false);
  const [nativeWebExplorerStatus, setNativeWebExplorerStatus] = useState("native webview pending");
  const nativeWebExplorerAcceptedRef = useRef(false);
  const nativeWebExplorerLastBoundsRef = useRef("");
  const nativeWebExplorerMotionSyncRef = useRef<((durationMs?: number) => void) | null>(null);
  const [nativeBrowserPage, setNativeBrowserPage] = useState<NativeBrowserPage>("maps");
  const previousDualNativeBrowserOpenRef = useRef(false);
  const parallelOpen = parallelPrompts.length > 1;
  const openToolPanes = [
    filesOpen ? "files" : "",
    terminalOpen ? "terminal" : ""
  ].filter(Boolean) as CanvasToolPane[];
  const activeToolPane: CanvasToolPane | "" = activePane && openToolPanes.includes(activePane)
    ? activePane
    : openToolPanes[0] ?? "";
  const filesLabel = sessionFiles.length === 1 ? "1 file" : `${sessionFiles.length} files`;
  const closeToolPane = (pane: CanvasToolPane) => {
    if (pane === "files") {
      onFilesClose();
      return;
    }
    onTerminalClose();
  };
  const webExplorerCanvasOpen = split && webExplorerOpen && !parallelOpen;
  const parallelWebExplorerOpen = split && webExplorerOpen && parallelOpen;
  const mapsCanvasOpen = split && mapsOpen && !parallelOpen;
  const parallelMapsOpen = split && mapsOpen && parallelOpen;
  const codingLivePreviewOpen = split && codingLivePreview !== null && !parallelOpen;
  const dualNativeBrowserOpen = split && webExplorerOpen && mapsOpen;
  const boundedWebExplorerParallelIndex = Math.max(0, Math.min(webExplorerParallelIndex, parallelPrompts.length - 1));
  const boundedMapsParallelIndex = Math.max(0, Math.min(mapsParallelIndex, parallelPrompts.length - 1));
  const activeWebExplorerSlotOpen = (webExplorerCanvasOpen || parallelWebExplorerOpen) && (!dualNativeBrowserOpen || nativeBrowserPage === "webexplorer");
  const activeMapsSlotOpen = (mapsCanvasOpen || parallelMapsOpen) && (!dualNativeBrowserOpen || nativeBrowserPage === "maps");
  const activeNativeBrowserSlotOpen = activeWebExplorerSlotOpen || activeMapsSlotOpen;
  const surfaceClassName = [
    "canvasSurfaces",
    "canvasSurfaces--split",
    actionsOpen ? "canvasSurfaces--actionsOpen" : "",
    filesOpen ? "canvasSurfaces--filesOpen" : "",
    terminalOpen ? "canvasSurfaces--terminalOpen" : "",
    parallelOpen || webExplorerCanvasOpen || mapsCanvasOpen || codingLivePreviewOpen ? "canvasSurfaces--parallelOpen" : "",
    activeWebExplorerSlotOpen ? "canvasSurfaces--webExplorerOpen" : "",
    codingLivePreviewOpen ? "canvasSurfaces--codingLivePreviewOpen" : "",
    activeMapsSlotOpen ? "canvasSurfaces--mapsOpen" : "",
    mapsClosing ? "canvasSurfaces--mapsClosing" : "",
    dualNativeBrowserOpen ? "canvasSurfaces--nativePager" : ""
  ].filter(Boolean).join(" ");

  useEffect(() => {
    const wasDualOpen = previousDualNativeBrowserOpenRef.current;
    previousDualNativeBrowserOpenRef.current = dualNativeBrowserOpen;
    if (dualNativeBrowserOpen && !wasDualOpen) {
      setNativeBrowserPage("maps");
      return;
    }
    if (!dualNativeBrowserOpen) {
      if (mapsOpen) {
        setNativeBrowserPage("maps");
      } else if (webExplorerOpen) {
        setNativeBrowserPage("webexplorer");
      }
    }
  }, [dualNativeBrowserOpen, mapsOpen, webExplorerOpen]);

  useEffect(() => {
    const api = globalThis.window?.forgeShell;
    if (activeWebExplorerSlotOpen) {
      return;
    }
    nativeWebExplorerAcceptedRef.current = false;
    nativeWebExplorerLastBoundsRef.current = "";
    setNativeWebExplorerAccepted(false);
    setNativeWebExplorerStatus("native webview closed");
    void api?.hideNativeWebExplorer?.();
  }, [activeWebExplorerSlotOpen]);

  useEffect(() => {
    const api = globalThis.window?.forgeShell;
    if (activeMapsSlotOpen) {
      void api?.hideNativeMaps?.();
      return;
    }
    void api?.hideNativeMaps?.();
  }, [activeMapsSlotOpen]);

  useLayoutEffect(() => {
    const api = globalThis.window?.forgeShell;
    if (!activeWebExplorerSlotOpen) {
      return undefined;
    }
    if (!api?.showNativeWebExplorer || !api.updateNativeWebExplorerBounds) {
      setNativeWebExplorerAccepted(false);
      setNativeWebExplorerStatus("native webview bridge unavailable");
      return undefined;
    }
    const showNativeWebExplorer = api.showNativeWebExplorer;
    const updateNativeWebExplorerBounds = api.updateNativeWebExplorerBounds;

    let animationFrame = 0;
    let motionFrame = 0;
    let motionSyncUntil = 0;
    const settleTimers: number[] = [];
    let retryTimer = 0;
    let firstSync = true;
    const syncNativeWebExplorerBounds = (bounds: { x: number; y: number; width: number; height: number }) => {
      const roundedBounds = {
        x: Math.round(bounds.x),
        y: Math.round(bounds.y),
        width: Math.round(bounds.width),
        height: Math.round(bounds.height)
      };
      const boundsKey = `${roundedBounds.x}:${roundedBounds.y}:${roundedBounds.width}:${roundedBounds.height}`;
      if (nativeWebExplorerLastBoundsRef.current === boundsKey && !firstSync) {
        return;
      }
      nativeWebExplorerLastBoundsRef.current = boundsKey;
      const command = firstSync ? showNativeWebExplorer : updateNativeWebExplorerBounds;
      firstSync = false;
      void command(roundedBounds).then((result) => {
         if (result?.accepted === false) {
           nativeWebExplorerAcceptedRef.current = false;
           setNativeWebExplorerAccepted(false);
           // Audit anchor: [webexplorer] native view rejected
           setNativeWebExplorerStatus(result.error?.message ?? "native webview rejected");
        } else if (result?.accepted === true) {
          if (!nativeWebExplorerAcceptedRef.current) {
            nativeWebExplorerAcceptedRef.current = true;
            setNativeWebExplorerAccepted(true);
            setNativeWebExplorerStatus(`native webview accepted ${result.url}`);
          }
        }
      }).catch((error: unknown) => {
        nativeWebExplorerAcceptedRef.current = false;
        setNativeWebExplorerAccepted(false);
        setNativeWebExplorerStatus(error instanceof Error ? error.message : String(error));
      });
    };
    const syncBounds = () => {
      animationFrame = 0;
      const slot = nativeWebExplorerSlotRef.current;
      if (!slot) {
        retryTimer = window.setTimeout(scheduleSync, 50);
        setNativeWebExplorerStatus("native webview waiting for slot");
        return;
      }
      const rect = slot.getBoundingClientRect();
      const bounds = {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height
      };
      if (bounds.width < 80 || bounds.height < 80) {
        retryTimer = window.setTimeout(scheduleSync, 80);
        setNativeWebExplorerStatus(`native webview waiting for bounds ${Math.round(bounds.width)}x${Math.round(bounds.height)}`);
        return;
      }
      syncNativeWebExplorerBounds(bounds);
    };
    const scheduleSync = () => {
      if (animationFrame === 0) {
        animationFrame = window.requestAnimationFrame(syncBounds);
      }
    };
    const tickMotionSync = (now: number) => {
      motionFrame = 0;
      scheduleSync();
      if (now < motionSyncUntil) {
        motionFrame = window.requestAnimationFrame(tickMotionSync);
      }
    };
    const runMotionSync = (durationMs = 360) => {
      motionSyncUntil = Math.max(motionSyncUntil, window.performance.now() + durationMs);
      if (motionFrame === 0) {
        motionFrame = window.requestAnimationFrame(tickMotionSync);
      }
    };
    nativeWebExplorerMotionSyncRef.current = runMotionSync;
    const observer = new ResizeObserver(scheduleSync);
    const observedSlot = nativeWebExplorerSlotRef.current;
    if (observedSlot) {
      observer.observe(observedSlot);
    }
    window.addEventListener("resize", scheduleSync);
    scheduleSync();
    runMotionSync(420);
    for (const delay of [50, 80, 180, 360, 720, 1200]) {
      settleTimers.push(window.setTimeout(scheduleSync, delay));
    }

    return () => {
      if (animationFrame !== 0) {
        window.cancelAnimationFrame(animationFrame);
      }
      if (motionFrame !== 0) {
        window.cancelAnimationFrame(motionFrame);
      }
      for (const timer of settleTimers) {
        window.clearTimeout(timer);
      }
      if (retryTimer !== 0) {
        window.clearTimeout(retryTimer);
      }
      if (nativeWebExplorerMotionSyncRef.current === runMotionSync) {
        nativeWebExplorerMotionSyncRef.current = null;
      }
      observer.disconnect();
      window.removeEventListener("resize", scheduleSync);
    };
  }, [activeWebExplorerSlotOpen, webExplorerParallelIndex, parallelPrompts.length]);

  useLayoutEffect(() => {
    if (!activeWebExplorerSlotOpen) {
      return;
    }
    nativeWebExplorerMotionSyncRef.current?.(420);
  }, [activeWebExplorerSlotOpen, leftPanelOpen]);

  useLayoutEffect(() => {
    if (!activeMapsSlotOpen) {
      return;
    }
    void globalThis.window?.forgeShell?.hideNativeMaps?.();
  }, [activeMapsSlotOpen, leftPanelOpen]);

  if (!split) return null;

  return (
    <section id="split-canvas" className={surfaceClassName} aria-label="Split canvas">
      <div className="canvasSurfaces__primary" aria-hidden={!parallelOpen && !activeNativeBrowserSlotOpen}>
        {parallelOpen ? (
          <div className={`parallelCanvasGrid parallelCanvasGrid--count${parallelPrompts.length}`}>
            {parallelPrompts.map((_prompt, index) => {
              const hostsWebExplorer = parallelWebExplorerOpen && index === boundedWebExplorerParallelIndex;
              const hostsMaps = parallelMapsOpen && index === boundedMapsParallelIndex;
              const canRemove = removableParallelIndexes[index] === true;
              return (
                <section
                  className={[
                    "parallelCanvasPane",
                    hostsWebExplorer || hostsMaps ? "webExplorerCanvasPane" : "",
                    hostsMaps ? "mapsCanvasPane" : "",
                    canRemove ? "parallelCanvasPane--removable" : ""
                  ].filter(Boolean).join(" ")}
                  key={`parallel-canvas-${index}`}
                  aria-label={hostsMaps ? `Parallel conversation ${index + 1} Maps canvas` : hostsWebExplorer ? `Parallel conversation ${index + 1} Web Explorer canvas` : `Parallel conversation ${index + 1}`}
                >
                  {canRemove ? (
                    <button
                      type="button"
                      className="parallelCanvasClose"
                      aria-label={`Remove parallel conversation ${index + 1}`}
                      onClick={() => onParallelRemove(index)}
                    >
                      <span aria-hidden="true" />
                    </button>
                  ) : null}
                  {hostsWebExplorer ? (
                    <>
                      <div className="webExplorerChrome" aria-hidden="true" />
                      {hostsMaps ? (
                        <NativeBrowserPager activePage={nativeBrowserPage} onPageChange={setNativeBrowserPage} webExplorerModuleId={webExplorerModuleId} />
                      ) : null}
                      <button
                        type="button"
                        className="webExplorerClose"
                        aria-label={hostsMaps && nativeBrowserPage === "maps" ? "Close Maps" : "Close Web Explorer"}
                        onClick={hostsMaps && nativeBrowserPage === "maps" ? onMapsClose : onWebExplorerClose}
                      >
                        <span aria-hidden="true" />
                      </button>
                      {hostsMaps && nativeBrowserPage === "maps" ? (
                        <BangerSphereNativeViewport searchQuery={mapsSearchQuery} target={mapsTarget} />
                      ) : (
                        <div
                          ref={nativeWebExplorerSlotRef}
                          className={nativeWebExplorerAccepted ? "webExplorerNativeSlot webExplorerNativeSlot--accepted" : "webExplorerNativeSlot"}
                        >
                          {nativeWebExplorerAccepted ? null : (
                            <>
                              <GoogleWordmarkMono />
                              <span className="webExplorerNativeStatus">{nativeWebExplorerStatus}</span>
                            </>
                          )}
                        </div>
                      )}
                    </>
                  ) : null}
                  {hostsMaps && !hostsWebExplorer ? (
                    <>
                      <div className="webExplorerChrome" aria-hidden="true" />
                      <button type="button" className="webExplorerClose" aria-label="Close Maps" onClick={onMapsClose}>
                        <span aria-hidden="true" />
                      </button>
                      <BangerSphereNativeViewport searchQuery={mapsSearchQuery} target={mapsTarget} />
                    </>
                  ) : null}
                </section>
              );
            })}
          </div>
        ) : codingLivePreviewOpen && codingLivePreview ? (
          <div className="parallelCanvasGrid parallelCanvasGrid--count2 codingLivePreviewGrid">
            <section className="parallelCanvasPane" aria-label="Primary canvas" />
            <section className="parallelCanvasPane codingLivePreviewPane" aria-label="Coding live preview canvas">
              <button type="button" className="webExplorerClose codingLivePreview__close" aria-label="Close coding live preview" onClick={onCodingLivePreviewClose}>
                <span aria-hidden="true" />
              </button>
              <CodingLivePreviewFrame preview={codingLivePreview} />
            </section>
          </div>
        ) : webExplorerCanvasOpen && mapsCanvasOpen ? (
          <div className={["parallelCanvasGrid parallelCanvasGrid--count2 webExplorerCanvasGrid mapsCanvasGrid", nativeBrowserPage === "maps" ? "mapsCanvasGrid--earthActive" : ""].filter(Boolean).join(" ")}>
            <section className="parallelCanvasPane" aria-label="Primary canvas" />
            <section className="parallelCanvasPane webExplorerCanvasPane mapsCanvasPane" aria-label="Maps and Airbnb canvas">
              <div className="webExplorerChrome" aria-hidden="true" />
              <NativeBrowserPager activePage={nativeBrowserPage} onPageChange={setNativeBrowserPage} webExplorerModuleId={webExplorerModuleId} />
              <button
                type="button"
                className="webExplorerClose"
                aria-label={nativeBrowserPage === "maps" ? "Close Maps" : "Close Web Explorer"}
                onClick={nativeBrowserPage === "maps" ? onMapsClose : onWebExplorerClose}
              >
                <span aria-hidden="true" />
              </button>
              {nativeBrowserPage === "maps" ? (
                <BangerSphereNativeViewport searchQuery={mapsSearchQuery} target={mapsTarget} />
              ) : (
                <div
                  ref={nativeWebExplorerSlotRef}
                  className={nativeWebExplorerAccepted ? "webExplorerNativeSlot webExplorerNativeSlot--accepted" : "webExplorerNativeSlot"}
                >
                  {nativeWebExplorerAccepted ? null : (
                    <>
                      <GoogleWordmarkMono />
                      <span className="webExplorerNativeStatus">{nativeWebExplorerStatus}</span>
                    </>
                  )}
                </div>
              )}
            </section>
          </div>
        ) : webExplorerCanvasOpen ? (
          <div className="parallelCanvasGrid parallelCanvasGrid--count2 webExplorerCanvasGrid">
            <section className="parallelCanvasPane" aria-label="Primary canvas" />
            <section className="parallelCanvasPane webExplorerCanvasPane" aria-label="Web Explorer canvas">
              <div className="webExplorerChrome" aria-hidden="true" />
              <button type="button" className="webExplorerClose" aria-label="Close Web Explorer" onClick={onWebExplorerClose}>
                <span aria-hidden="true" />
              </button>
              <div
                ref={nativeWebExplorerSlotRef}
                className={nativeWebExplorerAccepted ? "webExplorerNativeSlot webExplorerNativeSlot--accepted" : "webExplorerNativeSlot"}
              >
                {nativeWebExplorerAccepted ? null : (
                  <>
                    <GoogleWordmarkMono />
                    <span className="webExplorerNativeStatus">{nativeWebExplorerStatus}</span>
                  </>
                )}
              </div>
            </section>
          </div>
        ) : mapsCanvasOpen ? (
          <div className="parallelCanvasGrid parallelCanvasGrid--count2 webExplorerCanvasGrid mapsCanvasGrid mapsCanvasGrid--earthActive">
            <section className="parallelCanvasPane" aria-label="Primary canvas" />
            <section className="parallelCanvasPane webExplorerCanvasPane mapsCanvasPane" aria-label="Maps canvas">
              <div className="webExplorerChrome" aria-hidden="true" />
              <button type="button" className="webExplorerClose" aria-label="Close Maps" onClick={onMapsClose}>
                <span aria-hidden="true" />
              </button>
              <BangerSphereNativeViewport searchQuery={mapsSearchQuery} target={mapsTarget} />
            </section>
          </div>
        ) : null}
      </div>
      {planetsOpen ? (
        <CanvasPlanetRail />
      ) : actionsOpen ? (
        <aside className="canvasSplitPane" aria-label="Parallel canvas actions">
          <button type="button" className="canvasSplitCard" onClick={onFilesOpen} aria-expanded={filesOpen} aria-controls="canvas-files-pane">
            <span className="shellIcon shellIcon--assets" aria-hidden="true" />
            <strong>Files</strong>
            <small>Browse project files</small>
          </button>
          <button type="button" className="canvasSplitCard" onClick={onParallelAdd} disabled={parallelPrompts.length >= 4}>
            <svg className="canvasSplitIcon" xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 24 24" aria-hidden="true">
              <path fill="currentColor" d="M14 3v2H4v13.385L5.763 17H20v-7h2v8a1 1 0 0 1-1 1H6.455L2 22.5V4a1 1 0 0 1 1-1h11Zm5 0V0h2v3h3v2h-3v3h-2V5h-3V3h3Z" />
            </svg>
            <strong>Parallel Conversation</strong>
            <small>Start a parallel conversation</small>
          </button>
          <button type="button" className="canvasSplitCard" onClick={onPlanetsOpen} aria-expanded={planetsOpen}>
            <span className="shellIcon shellIcon--nav-web" aria-hidden="true" />
            <strong>3D Planets</strong>
            <small>Open planetary globe views</small>
          </button>
          <button type="button" className="canvasSplitCard" onClick={onWebExplorerOpen} aria-expanded={webExplorerOpen}>
            <span className="shellIcon shellIcon--google" aria-hidden="true" />
            <strong>Web Explorer</strong>
            <small>Search the web with Google</small>
          </button>
          <button type="button" className="canvasSplitCard" onClick={onTerminalOpen} aria-expanded={terminalOpen} aria-controls="canvas-terminal-pane">
            <svg className="canvasSplitIcon" xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 24 24" aria-hidden="true">
              <g fill="none" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5">
                <rect width="18.5" height="15.5" x="2.75" y="4.25" rx="3.5" />
                <path d="m7.25 9l3 3l-3 3m5.5 0h4" />
              </g>
            </svg>
            <strong>Open Terminal</strong>
            <small>Open a command terminal</small>
          </button>
        </aside>
      ) : null}
      {activeToolPane === "files" ? (
        <CanvasFilesPane
          sessionName={sessionName}
          files={sessionFiles}
          activePane={activeToolPane}
          openPanes={openToolPanes}
          onActivatePane={onActivePaneChange}
          onClosePane={closeToolPane}
          onTerminalOpen={onTerminalOpen}
        />
      ) : null}
      {activeToolPane === "terminal" ? (
        <CanvasTerminalPane
          sessionName={sessionName}
          activePane={activeToolPane}
          openPanes={openToolPanes}
          filesLabel={filesLabel}
          onActivatePane={onActivePaneChange}
          onClosePane={closeToolPane}
          onTerminalOpen={onTerminalOpen}
        />
      ) : null}
    </section>
  );
}
