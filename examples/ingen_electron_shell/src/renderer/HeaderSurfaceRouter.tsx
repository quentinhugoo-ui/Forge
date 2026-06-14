import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import "cesium/Build/Cesium/Widgets/widgets.css";
import type {
  BangerGoogleTilesConfigResult,
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
  const [tilesConfig, setTilesConfig] = useState<BangerGoogleTilesConfigResult | null>(null);
  const [cesiumLive, setCesiumLive] = useState(false);

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
    if (!host) {
      return;
    }

    let cancelled = false;
    let viewer: { destroy: () => void; isDestroyed?: () => boolean } | null = null;
    let removePostRender: (() => void) | null = null;

    setCesiumLive(false);
    void import("cesium")
      .then(async (Cesium) => {
        if (cancelled) return;
        const baseLayer = Cesium.ImageryLayer.fromProviderAsync(
          Cesium.TileMapServiceImageryProvider.fromUrl(
            Cesium.buildModuleUrl("Assets/Textures/NaturalEarthII")
          )
        );
        Cesium.RequestScheduler.requestsByServer["tile.googleapis.com:443"] = tilesConfig?.requestBudget ?? 18;
        viewer = new Cesium.Viewer(host, {
          animation: false,
          baseLayer,
          baseLayerPicker: false,
          fullscreenButton: false,
          geocoder: false,
          homeButton: false,
          infoBox: false,
          navigationHelpButton: false,
          requestRenderMode: false,
          scene3DOnly: true,
          sceneModePicker: false,
          selectionIndicator: false,
          shouldAnimate: false,
          skyAtmosphere: false,
          skyBox: false,
          timeline: false
        });
        const cesiumViewer = viewer as any;
        cesiumViewer.scene.backgroundColor = Cesium.Color.fromCssColorString("#05070a");
        cesiumViewer.scene.globe.baseColor = Cesium.Color.fromCssColorString("#2f6f9f");
        cesiumViewer.scene.globe.enableLighting = false;
        cesiumViewer.scene.globe.show = true;
        cesiumViewer.scene.renderError.addEventListener((_scene: unknown, error: unknown) => {
          console.error("Banger Cesium render failed.", error);
          setCesiumLive(false);
        });
        const markLiveWhenGlobeIsVisible = () => {
          if (cancelled) return;
          const visibleRectangle = cesiumViewer.camera.computeViewRectangle(Cesium.Ellipsoid.WGS84);
          if (visibleRectangle) {
            setCesiumLive(true);
            if (removePostRender) {
              removePostRender();
              removePostRender = null;
            }
          }
        };
        removePostRender = cesiumViewer.scene.postRender.addEventListener(markLiveWhenGlobeIsVisible);
        cesiumViewer.resize();
        cesiumViewer.camera.setView({
          destination: Cesium.Rectangle.fromDegrees(-180, -75, 180, 75)
        });
        cesiumViewer.scene.requestRender();
        if (!tilesConfig?.accepted || !tilesConfig.rootTilesetUrl) {
          return;
        }
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
        cesiumViewer.scene.primitives.add(tileset);
        cesiumViewer.scene.requestRender();
      })
      .catch((error) => {
        console.error("Banger Cesium viewport failed to mount.", error);
        setCesiumLive(false);
        // Keep the Banger canvas available even if the external tiles provider is still cold.
      });

    return () => {
      cancelled = true;
      if (removePostRender) {
        removePostRender();
      }
      if (viewer && (!viewer.isDestroyed || !viewer.isDestroyed())) {
        viewer.destroy();
      }
    };
  }, [tilesConfig]);

  return (
    <section className="surface surface--banger" aria-label={surface.label}>
      <div
        ref={cesiumHostRef}
        className={cesiumLive ? "bangerCesiumViewport bangerCesiumViewport--live" : "bangerCesiumViewport"}
        aria-label="Banger Cesium geospatial viewport"
      />
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
