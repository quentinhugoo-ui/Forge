import { useCallback, useEffect, useState, type CSSProperties, type ReactNode } from "react";
import type { BangerPreviewFrameResult, HeaderSurfaceContract, HeaderSurfaceSnapshot } from "../shared/ipc-contract";

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
  const [frame, setFrame] = useState<BangerPreviewFrameResult | null>(null);
  const [loading, setLoading] = useState(false);
  const loadFrame = useCallback(async () => {
    setLoading(true);
    try {
      const result = await globalThis.window?.forgeShell?.getBangerPreviewFrame?.();
      setFrame(result ?? null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void globalThis.window?.forgeShell?.getBangerPreviewFrame?.()
      .then((result) => {
        if (active) {
          setFrame(result ?? null);
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });
    return () => {
      active = false;
    };
  }, []);

  const acceptedFrame = frame?.accepted && frame.frameDataUrl ? frame : null;
  const frameProof = acceptedFrame?.proofHash ?? surface.proofHash;

  return (
    <section className="surface surface--banger" aria-label="Banger native child surface contract">
      <div className={acceptedFrame ? "nativeViewportSlot nativeViewportSlot--live" : "nativeViewportSlot"}>
        {acceptedFrame ? (
          <img
            className="nativeViewportSlot__frame"
            src={acceptedFrame.frameDataUrl}
            alt="Banger Gaussian splat native preview frame"
            width={acceptedFrame.width}
            height={acceptedFrame.height}
          />
        ) : (
          <div className="nativeViewportSlot__empty">
            <strong>Banger native viewport</strong>
            <span>{loading ? "native raster frame loading" : frame?.error?.message ?? "wgpu child window / frame hash / residency proof"}</span>
          </div>
        )}
      </div>
      <div className="surfaceActionRow">
        <button type="button" onClick={() => void loadFrame()}>{loading ? "Rasterizing" : "Frame proof"}</button>
        <button type="button">Scene graph</button>
      </div>
      <div className="bangerFrameLedger" aria-label="Banger frame metrics">
        <div>
          <span>splats</span>
          <code>{frame?.metrics.splatCount ?? 0}/{frame?.metrics.projectedSplatCount ?? 0}</code>
        </div>
        <div>
          <span>pixels</span>
          <code>{frame?.metrics.shadedPixelCount ?? 0}</code>
        </div>
        <div>
          <span>frame</span>
          <code>{acceptedFrame?.frameHash.slice(0, 16) ?? "pending"}</code>
        </div>
      </div>
      <SurfaceProof surface={{ ...surface, proofHash: frameProof }} />
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

  const style = {
    "--surface-left": `${primary.slot.x}px`,
    "--surface-top": `${primary.slot.y + 58}px`,
    "--surface-width": `${primary.slot.width}px`,
    "--surface-height": `${Math.max(320, primary.slot.height - 58)}px`
  } as CSSProperties;

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
