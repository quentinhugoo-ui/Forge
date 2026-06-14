import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import type {
  BangerPresentLoopBootstrapResult,
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
  const [previewFrame, setPreviewFrame] = useState<BangerPreviewFrameResult | null>(null);
  const [presentLoop, setPresentLoop] = useState<BangerPresentLoopBootstrapResult | null>(null);

  useEffect(() => {
    let active = true;
    void globalThis.window?.forgeShell?.getBangerPreviewFrame?.()
      .then((result) => {
        if (active) {
          setPreviewFrame(result ?? null);
        }
      })
      .catch((error) => {
        console.error("Banger native preview frame failed to load.", error);
      });
    void globalThis.window?.forgeShell?.getBangerPresentLoopBootstrap?.()
      .then((result) => {
        if (active) {
          setPresentLoop(result ?? null);
        }
      })
      .catch((error) => {
        console.error("Banger native present loop failed to bootstrap.", error);
      });
    return () => {
      active = false;
    };
  }, []);

  const presentLoopFrameDataUrl = presentLoop?.ok === true ? presentLoop.previewFrameDataUrl ?? "" : "";
  const splatFrameDataUrl = previewFrame?.accepted === true ? previewFrame.frameDataUrl : "";
  const nativeFrameDataUrl = presentLoopFrameDataUrl || splatFrameDataUrl;
  const hasNativeFrame = nativeFrameDataUrl.length > 0;
  const renderPath = presentLoopFrameDataUrl
    ? "rust_banger_wgpu_present_loop_rgba8_to_bmp_data_url"
    : previewFrame?.metrics.renderPath ?? "rust-banger-wgpu-child-window";

  return (
    <section className="surface surface--banger" aria-label={surface.label}>
      <div
        className={hasNativeFrame ? "nativeViewportSlot nativeViewportSlot--live" : "nativeViewportSlot"}
        aria-label="Banger native renderer surface"
        data-native-contract={surface.nativeContract}
        data-present-loop={presentLoop?.routeStatus ?? "pending"}
        data-render-path={renderPath}
      >
        {hasNativeFrame ? (
          <img
            className="nativeViewportSlot__frame"
            src={nativeFrameDataUrl}
            alt=""
            draggable={false}
          />
        ) : (
          <div className="nativeViewportSlot__empty" aria-hidden="true" />
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
