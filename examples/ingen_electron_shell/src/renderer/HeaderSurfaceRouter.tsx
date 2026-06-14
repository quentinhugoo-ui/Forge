import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import "cesium/Build/Cesium/Widgets/widgets.css";
import type {
  BangerGoogleTilesConfigResult,
  BangerPreviewFrameResult,
  HeaderSurfaceContract,
  HeaderSurfaceSnapshot
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
  const cesiumHostRef = useRef<HTMLDivElement | null>(null);
  const [frame, setFrame] = useState<BangerPreviewFrameResult | null>(null);
  const [tilesConfig, setTilesConfig] = useState<BangerGoogleTilesConfigResult | null>(null);
  const [tilesFailed, setTilesFailed] = useState(false);

  useEffect(() => {
    let active = true;
    void globalThis.window?.forgeShell?.getBangerPreviewFrame?.()
      .then((result) => {
        if (active) {
          setFrame(result ?? null);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    void globalThis.window?.forgeShell?.getBangerGoogleTilesConfig?.()
      .then((result) => {
        if (active) {
          setTilesConfig(result ?? null);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const host = cesiumHostRef.current;
    if (!host || !tilesConfig?.accepted || !tilesConfig.rootTilesetUrl) {
      return;
    }

    let cancelled = false;
    let viewer: { destroy: () => void; isDestroyed?: () => boolean } | null = null;
    setTilesFailed(false);

    void import("cesium")
      .then(async (Cesium) => {
        if (cancelled) return;
        Cesium.RequestScheduler.requestsByServer["tile.googleapis.com:443"] = tilesConfig.requestBudget;
        viewer = new Cesium.Viewer(host, {
          animation: false,
          baseLayerPicker: false,
          fullscreenButton: false,
          geocoder: false,
          globe: false,
          homeButton: false,
          infoBox: false,
          navigationHelpButton: false,
          requestRenderMode: true,
          scene3DOnly: true,
          sceneModePicker: false,
          selectionIndicator: false,
          shouldAnimate: false,
          timeline: false
        });
        const tilesetFactory = Cesium.Cesium3DTileset as unknown as {
          fromUrl?: (url: string, options: Record<string, unknown>) => Promise<unknown>;
          new(options: Record<string, unknown>): unknown;
        };
        const tilesetOptions = {
          showCreditsOnScreen: tilesConfig.showCreditsOnScreen,
          skipLevelOfDetail: true,
          dynamicScreenSpaceError: true
        };
        const tileset = tilesetFactory.fromUrl
          ? await tilesetFactory.fromUrl(tilesConfig.rootTilesetUrl, tilesetOptions)
          : new tilesetFactory({ url: tilesConfig.rootTilesetUrl, ...tilesetOptions });
        if (cancelled) return;
        const cesiumViewer = viewer as any;
        cesiumViewer.scene.primitives.add(tileset);
        const { longitude, latitude, heightMeters, headingDegrees, pitchDegrees, rollDegrees } = tilesConfig.initialView;
        cesiumViewer.camera.setView({
          destination: Cesium.Cartesian3.fromDegrees(longitude, latitude, heightMeters),
          orientation: {
            heading: Cesium.Math.toRadians(headingDegrees),
            pitch: Cesium.Math.toRadians(pitchDegrees),
            roll: Cesium.Math.toRadians(rollDegrees)
          }
        });
        cesiumViewer.scene.requestRender();
      })
      .catch(() => {
        if (!cancelled) {
          setTilesFailed(true);
        }
      });

    return () => {
      cancelled = true;
      if (viewer && (!viewer.isDestroyed || !viewer.isDestroyed())) {
        viewer.destroy();
      }
    };
  }, [tilesConfig]);

  const acceptedFrame = frame?.accepted && frame.frameDataUrl ? frame : null;
  const showGoogleTiles = Boolean(tilesConfig?.accepted && tilesConfig.rootTilesetUrl && !tilesFailed);

  return (
    <section className="surface surface--banger" aria-label={surface.label}>
      {showGoogleTiles ? <div ref={cesiumHostRef} className="bangerCesiumViewport" aria-label="Banger Google Photorealistic 3D Tiles viewport" /> : null}
      {!showGoogleTiles && acceptedFrame ? (
        <img
          className="nativeViewportSlot__frame"
          src={acceptedFrame.frameDataUrl}
          alt="Banger Gaussian splat native preview frame"
          width={acceptedFrame.width}
          height={acceptedFrame.height}
        />
      ) : null}
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
