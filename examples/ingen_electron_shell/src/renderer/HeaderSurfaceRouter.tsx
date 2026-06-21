import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import type {
  BangerGoogleTilesConfigResult,
  BangerPresentLoopBootstrapResult,
  HeaderSurfaceContract,
  HeaderSurfaceSnapshot,
  NativeWebExplorerResult
} from "../shared/ipc-contract";

function statusLabel(surface: HeaderSurfaceContract): string {
  if (surface.status === "native_pending") return "native pending";
  if (surface.status === "native_ready") return "native ready";
  if (surface.status === "delegated_to_parallel_slice") return "delegated";
  return "shadow";
}

function SurfaceProof({ surface }: { surface: HeaderSurfaceContract }) {
  return (
    <div className="surfaceProof" aria-label={`${surface.label} proof`}>
      <span>{statusLabel(surface)}</span>
      <code>{surface.proofHash.slice(0, 16)}</code>
    </div>
  );
}

const GOOGLE_PHOTOREALISTIC_3D_TILES_ION_ASSET_ID = 2275207;

function createBangerCesiumTilesSrcDoc(config: BangerGoogleTilesConfigResult): string {
  const view = {
    latitude: config.initialView.latitude ?? 48.8584,
    longitude: config.initialView.longitude ?? 2.2945,
    heightMeters: config.initialView.heightMeters ?? 1800
  };
  const bootstrapConfig = {
    rootTilesetUrl: config.rootTilesetUrl,
    cesiumIonAccessToken: config.cesiumIonAccessToken ?? "",
    cesiumIonAccessTokenUrl: config.cesiumIonAccessTokenUrl ?? "",
    ionAssetId: GOOGLE_PHOTOREALISTIC_3D_TILES_ION_ASSET_ID,
    view
  };
  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <script>window.CESIUM_BASE_URL = "https://ajax.googleapis.com/ajax/libs/cesiumjs/1.105/Build/Cesium/";</script>
  <link rel="stylesheet" href="https://ajax.googleapis.com/ajax/libs/cesiumjs/1.105/Build/Cesium/Widgets/widgets.css" />
  <style>
    html,
    body,
    #cesiumContainer {
      width: 100%;
      height: 100%;
      margin: 0;
      overflow: hidden;
      background: #020508;
    }
    .cesium-viewer-toolbar,
    .cesium-viewer-animationContainer,
    .cesium-viewer-timelineContainer,
    .cesium-viewer-fullscreenContainer,
    .cesium-viewer-bottom {
      display: none !important;
    }
    .cesium-widget-credits {
      opacity: 0.62;
      transform: scale(0.82);
      transform-origin: left bottom;
    }
  </style>
</head>
<body>
  <div id="cesiumContainer"></div>
  <script src="https://ajax.googleapis.com/ajax/libs/cesiumjs/1.105/Build/Cesium/Cesium.js"></script>
  <script>
    (async () => {
      const config = ${JSON.stringify(bootstrapConfig)};
      async function resolveCesiumIonToken() {
        if (config.cesiumIonAccessToken) {
          return config.cesiumIonAccessToken;
        }
        if (!config.cesiumIonAccessTokenUrl) {
          return "";
        }
        const response = await fetch(config.cesiumIonAccessTokenUrl, { cache: "no-store" });
        if (!response.ok) {
          throw new Error("Cesium ion token broker rejected " + response.status);
        }
        const payload = await response.json();
        return payload.token || payload.accessToken || payload.cesiumIonAccessToken || payload.ionToken || "";
      }
      const viewer = new Cesium.Viewer("cesiumContainer", {
        animation: false,
        baseLayerPicker: false,
        fullscreenButton: false,
        geocoder: false,
        homeButton: false,
        infoBox: false,
        navigationHelpButton: false,
        sceneModePicker: false,
        selectionIndicator: false,
        timeline: false,
        skyBox: false,
        requestRenderMode: false
      });
      viewer.scene.globe.show = true;
      viewer.scene.fog.enabled = true;
      viewer.scene.screenSpaceCameraController.enableCollisionDetection = false;
      const rootTilesetUrl = config.rootTilesetUrl;
      const isIonTileset = String(rootTilesetUrl).startsWith("ion://");
      const token = isIonTileset ? await resolveCesiumIonToken() : "";
      if (token) {
        Cesium.Ion.defaultAccessToken = token;
      }
      viewer.imageryLayers.removeAll();
      async function installGoogleEarthStyleGlobeImagery() {
        if (Cesium.createWorldImageryAsync) {
          const provider = await Cesium.createWorldImageryAsync({
            style: Cesium.IonWorldImageryStyle ? Cesium.IonWorldImageryStyle.AERIAL : undefined
          });
          viewer.imageryLayers.addImageryProvider(provider);
          return;
        }
        if (Cesium.IonImageryProvider?.fromAssetId && Cesium.ImageryLayer?.fromProviderAsync) {
          viewer.imageryLayers.add(
            Cesium.ImageryLayer.fromProviderAsync(Cesium.IonImageryProvider.fromAssetId(2))
          );
        }
      }
      async function installPhotorealisticTilesWhenClose() {
        const tileset = isIonTileset
          ? await Cesium.Cesium3DTileset.fromIonAssetId(config.ionAssetId, { showCreditsOnScreen: true })
          : Cesium.Cesium3DTileset.fromUrl
            ? await Cesium.Cesium3DTileset.fromUrl(rootTilesetUrl, { showCreditsOnScreen: true })
            : new Cesium.Cesium3DTileset({ url: rootTilesetUrl, showCreditsOnScreen: true });
        tileset.show = false;
        viewer.scene.primitives.add(tileset);
        const syncTilesetVisibility = () => {
          tileset.show = viewer.camera.positionCartographic.height < 1400000;
        };
        viewer.camera.changed.addEventListener(syncTilesetVisibility);
        syncTilesetVisibility();
      }
      await installGoogleEarthStyleGlobeImagery();
      const latitude = Number(config.view.latitude);
      const longitude = Number(config.view.longitude);
      const heightMeters = Number(config.view.heightMeters);
      if (Number.isFinite(latitude) && Number.isFinite(longitude)) {
        viewer.camera.setView({
          destination: Cesium.Cartesian3.fromDegrees(
            longitude,
            latitude,
            Math.max(16000000, Number.isFinite(heightMeters) ? heightMeters * 6200 : 16000000)
          ),
          orientation: {
            heading: 0,
            pitch: Cesium.Math.toRadians(-90),
            roll: 0
          }
        });
      } else {
        viewer.camera.setView({
          destination: Cesium.Cartesian3.fromDegrees(0, 12, 18000000),
          orientation: {
            heading: 0,
            pitch: Cesium.Math.toRadians(-90),
            roll: 0
          }
        });
      }
      void installPhotorealisticTilesWhenClose().catch((tilesetError) => {
        console.error("Banger Cesium 3D Tiles close-range overlay failed.", tilesetError);
      });
      window.parent.postMessage({ type: "forge:banger-cesium-tiles", status: "tiles_loaded" }, "*");
    })().catch((error) => {
      console.error("Banger Cesium 3D Tiles bootstrap failed.", error);
      window.parent.postMessage({ type: "forge:banger-cesium-tiles", status: "failed" }, "*");
    });
  </script>
</body>
</html>`;
}

function WebExplorerSurface({ surfaces }: { surfaces: HeaderSurfaceContract[] }) {
  const webview = surfaces.find((surface) => surface.kind === "webexplorer_webview");
  const atlas = surfaces.find((surface) => surface.kind === "webexplorer_atlas");
  return (
    <section className="surface surface--web" aria-label="WebExplorer native surface contract">
      <div className="webChrome">
        <code>{webview?.nativeContract ?? "rust-owned-webview-policy-host"}</code>
      </div>
      <div className="webStage">
        <div>
          <strong>Rust WebView host</strong>
          <span>navigation policy / capture / DOM RAM / AX tree / proof hashes</span>
        </div>
      </div>
      <aside className="atlasRail" aria-label="RAM DOM Atlas contract">
        <strong>RAM DOM Atlas</strong>
        <p>{atlas?.summary}</p>
        {atlas ? <SurfaceProof surface={atlas} /> : null}
      </aside>
      {webview ? <SurfaceProof surface={webview} /> : null}
    </section>
  );
}

function BangerSurface({ surface }: { surface: HeaderSurfaceContract }) {
  const slotRef = useRef<HTMLDivElement | null>(null);
  const requestIdRef = useRef(0);
  const [presentLoop, setPresentLoop] = useState<BangerPresentLoopBootstrapResult | null>(null);
  const [slotBounds, setSlotBounds] = useState<{ x: number; y: number; width: number; height: number } | null>(null);
  const [bootstrapStatus, setBootstrapStatus] = useState("pending");
  const [nativeSurfaceStatus, setNativeSurfaceStatus] = useState("pending");
  const [tilesConfig, setTilesConfig] = useState<BangerGoogleTilesConfigResult | null>(null);
  const nativeSurfaceAttachedRef = useRef(false);

  useEffect(() => {
    const getTilesConfig = globalThis.window?.forgeShell?.getBangerGoogleTilesConfig;
    if (!getTilesConfig) {
      return undefined;
    }
    let active = true;
    void getTilesConfig()
      .then((result) => {
        if (active) {
          setTilesConfig(result);
        }
      })
      .catch(() => {
        if (active) {
          setTilesConfig(null);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const nativeMapsTarget = useMemo(() => {
    if (!tilesConfig?.accepted) {
      return { target: "Cesium 3D Tiles sphere" };
    }
    return {
      target: "Google Photorealistic 3D Tiles",
      latitude: tilesConfig.initialView.latitude,
      longitude: tilesConfig.initialView.longitude,
      heightMeters: tilesConfig.initialView.heightMeters
    };
  }, [tilesConfig]);

  const measureSlot = useCallback(() => {
    const rect = slotRef.current?.getBoundingClientRect();
    if (!rect) return;
    const next = {
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height)
    };
    if (next.width < 80 || next.height < 80) return;
    setSlotBounds((current) => {
      if (
        current &&
        current.x === next.x &&
        current.y === next.y &&
        current.width === next.width &&
        current.height === next.height
      ) {
        return current;
      }
      return next;
    });
  }, []);

  useEffect(() => {
    const slot = slotRef.current;
    if (!slot) return undefined;
    let animationFrame = 0;
    const scheduleMeasure = () => {
      if (animationFrame !== 0) return;
      animationFrame = window.requestAnimationFrame(() => {
        animationFrame = 0;
        measureSlot();
      });
    };
    scheduleMeasure();
    const observer = new ResizeObserver(scheduleMeasure);
    observer.observe(slot);
    window.addEventListener("resize", scheduleMeasure);
    return () => {
      if (animationFrame !== 0) {
        window.cancelAnimationFrame(animationFrame);
      }
      observer.disconnect();
      window.removeEventListener("resize", scheduleMeasure);
    };
  }, [measureSlot]);

  useEffect(() => {
    const showNativeBanger = globalThis.window?.forgeShell?.showNativeBanger;
    const updateNativeBangerBounds = globalThis.window?.forgeShell?.updateNativeBangerBounds;
    let active = true;
    if (!showNativeBanger) {
      setNativeSurfaceStatus("bridge unavailable");
      return () => {
        active = false;
      };
    }
    if (!slotBounds) {
      setNativeSurfaceStatus("measuring");
      return () => {
        active = false;
      };
    }

    const bounds = {
      x: slotBounds.x,
      y: slotBounds.y,
      width: slotBounds.width,
      height: slotBounds.height,
      sceneKind: "maps_sphere" as const,
      ...nativeMapsTarget
    };
    const canUpdateExistingSurface = nativeSurfaceAttachedRef.current && updateNativeBangerBounds;
    const request = canUpdateExistingSurface ? updateNativeBangerBounds(bounds) : showNativeBanger(bounds);
    setNativeSurfaceStatus(canUpdateExistingSurface ? "positioning" : "opening");
    void request
      .then((result: NativeWebExplorerResult) => {
        if (!active) return;
        if (result.accepted) {
          nativeSurfaceAttachedRef.current = true;
          setNativeSurfaceStatus("native live");
          return;
        }
        setNativeSurfaceStatus(result.error?.code ?? "blocked");
      })
      .catch((error) => {
        console.error("Banger native surface failed.", error);
        if (active) {
          setNativeSurfaceStatus("failed");
        }
      });

    return () => {
      active = false;
    };
  }, [nativeMapsTarget, slotBounds]);

  useEffect(() => {
    return () => {
      void globalThis.window?.forgeShell?.hideNativeBanger?.();
    };
  }, []);

  useEffect(() => {
    const getBootstrap = globalThis.window?.forgeShell?.getBangerPresentLoopBootstrap as
      | ((request?: {
          x?: number;
          y?: number;
          width?: number;
          height?: number;
          sceneKind?: "maps_sphere";
        }) => Promise<BangerPresentLoopBootstrapResult>)
      | undefined;
    let active = true;
    const requestId = ++requestIdRef.current;
    if (!getBootstrap) {
      setBootstrapStatus("bridge unavailable");
      return () => {
        active = false;
      };
    }
    if (!slotBounds) {
      setBootstrapStatus("measuring");
      return () => {
        active = false;
      };
    }
    setBootstrapStatus("booting");
    void getBootstrap?.({
      x: slotBounds.x,
      y: slotBounds.y,
      width: slotBounds.width,
      height: slotBounds.height,
      sceneKind: "maps_sphere"
    })
      .then((result) => {
        if (active && requestId === requestIdRef.current) {
          setPresentLoop(result ?? null);
          setBootstrapStatus(result?.ok ? "live" : result?.error?.code ?? "blocked");
        }
      })
      .catch((error) => {
        console.error("Banger native present loop failed to bootstrap.", error);
        if (active && requestId === requestIdRef.current) {
          setBootstrapStatus("failed");
        }
      });
    return () => {
      active = false;
    };
  }, [slotBounds]);

  const presentLoopFrameDataUrl = presentLoop?.ok === true ? presentLoop.previewFrameDataUrl ?? "" : "";
  const nativeFrameDataUrl = presentLoopFrameDataUrl;
  const hasNativeSurface = nativeSurfaceStatus === "native live";
  const hasNativeFrame = nativeFrameDataUrl.length > 0;
  const cesiumSrcDoc = !hasNativeSurface && !hasNativeFrame && tilesConfig?.accepted && tilesConfig.rootTilesetUrl
    ? createBangerCesiumTilesSrcDoc(tilesConfig)
    : "";
  const renderPath = presentLoopFrameDataUrl
    ? "rust_banger_wgpu_maps_sphere_present_loop_rgba8_to_bmp_data_url"
    : cesiumSrcDoc
      ? "cesiumjs_google_earth_style_globe_with_photorealistic_tiles_overlay"
      : "rust-banger-wgpu-maps-sphere-child-window";

  return (
    <section className="surface surface--banger" aria-label={surface.label}>
      <div
        ref={slotRef}
        className={hasNativeSurface || hasNativeFrame ? "nativeViewportSlot nativeViewportSlot--live" : "nativeViewportSlot"}
        style={cesiumSrcDoc ? { pointerEvents: "auto" } : undefined}
        aria-label="Banger native renderer surface"
        data-native-contract={surface.nativeContract}
        data-present-loop={presentLoop?.routeStatus ?? "pending"}
        data-bootstrap-status={bootstrapStatus}
        data-native-surface-status={nativeSurfaceStatus}
        data-render-path={renderPath}
        data-active-renderer="banger_wgpu_native_cesium_3d_tiles_sphere"
        data-tileset-provider={tilesConfig?.provider ?? "google_photorealistic_3d_tiles"}
        data-tileset-renderer-model={tilesConfig?.rendererModel ?? "cesium_for_unreal_style_3d_tileset"}
        data-tileset-georeference={
          tilesConfig
            ? `${tilesConfig.georeference.ellipsoid}:${tilesConfig.georeference.originLatitude.toFixed(5)}:${tilesConfig.georeference.originLongitude.toFixed(5)}`
            : "WGS84:pending"
        }
        data-native-streamer={tilesConfig?.nativeStreamer.schema ?? "forge.banger.native_3d_tiles_streamer.v1"}
      >
        {hasNativeFrame && !hasNativeSurface ? (
          <img
            className="nativeViewportSlot__frame"
            src={nativeFrameDataUrl}
            alt=""
            draggable={false}
          />
        ) : cesiumSrcDoc ? (
          <iframe
            className="nativeViewportSlot__frame"
            title=""
            srcDoc={cesiumSrcDoc}
            sandbox="allow-scripts allow-same-origin allow-popups allow-forms"
            style={{ border: 0, display: "block", width: "100%", height: "100%" }}
          />
        ) : (
          null
        )}
      </div>
    </section>
  );
}

function ProductSurface({ surface }: { surface: HeaderSurfaceContract }) {
  return (
    <section className="surface surface--product" aria-label="Product section surface contract">
      <header>
        <strong>{surface.route === "trading" ? "Forge Trading" : "Product section"}</strong>
        <span>{surface.summary}</span>
      </header>
      <div className="productMetrics">
        <div>
          <span>service</span>
          <code>{surface.nativeContract}</code>
        </div>
        <div>
          <span>authority</span>
          <code>{surface.authority}</code>
        </div>
        <div>
          <span>route</span>
          <code>{surface.route}</code>
        </div>
      </div>
      <div className="surfaceActionRow">
        <button type="button">Market proof</button>
        <button type="button">Backtest</button>
        <button type="button">Alerts</button>
      </div>
      <SurfaceProof surface={surface} />
    </section>
  );
}

function DropSurface({ surface }: { surface: HeaderSurfaceContract }) {
  return (
    <section className="surface surface--drop" aria-label="Forge canvas shadow contract">
      <strong>{surface.label}</strong>
      <span>{surface.summary}</span>
      <SurfaceProof surface={surface} />
    </section>
  );
}

function DelegatedSurface({ surface }: { surface: HeaderSurfaceContract }) {
  return (
    <section className="surface surface--delegated" aria-label={`${surface.label} delegated contract`}>
      <strong>{surface.label}</strong>
      <span>{surface.summary}</span>
      <SurfaceProof surface={surface} />
    </section>
  );
}

export function HeaderSurfaceRouter({ snapshot }: { snapshot: HeaderSurfaceSnapshot }) {
  const [primary] = snapshot.surfaces;
  if (!primary) return null;

  const style = primary.kind === "banger_native_child"
    ? ({
        "--surface-left": "0px",
        "--surface-top": "0px",
        "--surface-width": "100vw",
        "--surface-height": "100vh"
      } as CSSProperties)
    : ({
        "--surface-left": `${primary.slot.x}px`,
        "--surface-top": `${primary.slot.y + 58}px`,
        "--surface-width": `${primary.slot.width}px`,
        "--surface-height": `${Math.max(320, primary.slot.height - 58)}px`
      } as CSSProperties);

  let content: ReactNode;
  if (snapshot.activeSection === "webexplorer" && snapshot.profileCanvas === "") {
    content = <WebExplorerSurface surfaces={snapshot.surfaces} />;
  } else if (primary.kind === "banger_native_child") {
    content = <BangerSurface surface={primary} />;
  } else if (primary.kind === "product_section") {
    content = <ProductSurface surface={primary} />;
  } else if (primary.kind === "delegated") {
    content = <DelegatedSurface surface={primary} />;
  } else {
    content = <DropSurface surface={primary} />;
  }

  return (
    <section className="surfaceRouter" style={style} aria-label="Header opened surface router">
      {content}
    </section>
  );
}
