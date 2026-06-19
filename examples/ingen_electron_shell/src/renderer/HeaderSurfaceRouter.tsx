import { useCallback, useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import type {
  BangerPresentLoopBootstrapResult,
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

function shortProof(value?: string | null): string {
  return value ? value.slice(0, 16) : "pending";
}

function bangerGateLabel(presentLoop: BangerPresentLoopBootstrapResult | null): string {
  if (!presentLoop) return "pending";
  if (!presentLoop.ok) return presentLoop.error?.code ?? "blocked";
  if (!presentLoop.mapsVisualGate) return "no gate";
  return presentLoop.mapsVisualGate.ok ? "visible" : "blocked";
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
  const [refreshTick, setRefreshTick] = useState(0);
  const [bootstrapStatus, setBootstrapStatus] = useState("pending");

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
  }, [refreshTick, slotBounds]);

  const presentLoopFrameDataUrl = presentLoop?.ok === true ? presentLoop.previewFrameDataUrl ?? "" : "";
  const nativeFrameDataUrl = presentLoopFrameDataUrl;
  const hasNativeFrame = nativeFrameDataUrl.length > 0;
  const renderPath = presentLoopFrameDataUrl
    ? "rust_banger_wgpu_maps_sphere_present_loop_rgba8_to_bmp_data_url"
    : "rust-banger-wgpu-maps-sphere-child-window";

  return (
    <section className="surface surface--banger" aria-label={surface.label}>
      <div
        ref={slotRef}
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
          <div className="bangerSphereNativeFrame__fallback" aria-hidden="true">
            <span className="bangerSphereNativeFrame__fallbackSphere" />
          </div>
        )}
      </div>
      <aside className="bangerFrameLedger" aria-label="Banger native frame ledger">
        <div>
          <span>route</span>
          <code>{presentLoop?.routeStatus ?? bootstrapStatus}</code>
        </div>
        <div>
          <span>gate</span>
          <code>{bangerGateLabel(presentLoop)}</code>
        </div>
        <div>
          <span>draw</span>
          <code>
            {presentLoop?.drawCallCount ?? 0}/{presentLoop?.indexCount ?? 0}/{presentLoop?.instanceCount ?? 0}
          </code>
        </div>
        <div>
          <span>frame</span>
          <code>{shortProof(presentLoop?.mapsVisualGate?.frameHash ?? presentLoop?.frameHash)}</code>
        </div>
        <div>
          <span>mesh</span>
          <code>{shortProof(presentLoop?.sceneMeshHash)}</code>
        </div>
      </aside>
      <div className="surfaceActionRow">
        <button type="button" onClick={() => setRefreshTick((tick) => tick + 1)}>
          Refresh
        </button>
      </div>
      <SurfaceProof surface={surface} />
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
