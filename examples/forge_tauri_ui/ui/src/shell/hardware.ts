type ForgeGpuInfo = {
  readonly name?: string;
  readonly backend?: string;
};

type ForgeHardwareInfo = {
  readonly cpu_brand?: string;
  readonly cpu_threads?: number;
  readonly os?: string;
  readonly arch?: string;
  readonly gpus?: readonly ForgeGpuInfo[];
  readonly cuda_enabled?: boolean;
  readonly nvrtc_available?: boolean;
};

type ForgeHardwareCellDeps = {
  canInvoke(): boolean;
  invoke<T = unknown>(command: string, args?: Record<string, unknown>, options?: Record<string, unknown>): Promise<T>;
  trace(stage: string, details?: unknown): void;
  appendLog(line: string): void;
  isFrench(): boolean;
  dispatchHardware(info: ForgeHardwareInfo | null): void;
  onUpdate(info: ForgeHardwareInfo | null): void;
  invalidateProofPanel(): void;
};

type ForgeHardwareCell = {
  install(deps: ForgeHardwareCellDeps): void;
  load(): Promise<void>;
  ensureStartupDiagnostics(): Promise<void>;
  formatGpuName(gpu: ForgeGpuInfo): string;
};

declare global {
  interface Window {
    ForgeHardwareCell?: ForgeHardwareCell;
  }
}

let retryTimer = 0;
let activeDeps: ForgeHardwareCellDeps | null = null;
let startupDiagnosticsPromise: Promise<void> | null = null;
let currentAlert: Record<string, unknown> | null = null;

function escapeAttr(value: unknown): string {
  return String(value).replace(/[&<>"']/g, (char) => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    "\"": "&quot;",
    "'": "&#39;",
  })[char] || char);
}

function formatGpuName(gpu: ForgeGpuInfo): string {
  const name = (gpu?.name || "Unknown GPU").trim();
  const tag = gpu?.backend === "cuda" ? "CUDA" : gpu?.backend === "wgpu" ? "WGPU" : "idle";
  return `${name}  ·  ${tag}`;
}

function renderGpuRows(gpus: readonly ForgeGpuInfo[] = [], french = false): void {
  const rowsRoot = document.getElementById("panelHardwareGpuRows");
  if (!rowsRoot) return;
  if (!gpus.length) {
    rowsRoot.innerHTML = `
      <div class="panel-hardware-row">
        <span class="panel-hardware-label">GPU</span>
        <span class="panel-hardware-text">${french ? "Aucun GPU détecté" : "No GPU detected"}</span>
      </div>`;
    return;
  }
  rowsRoot.innerHTML = gpus.map((gpu) => `
    <div class="panel-hardware-row">
      <span class="panel-hardware-label">GPU</span>
      <span class="panel-hardware-text" title="${escapeAttr(gpu.name)} (${escapeAttr(gpu.backend)})">${escapeAttr(formatGpuName(gpu))}</span>
    </div>`).join("");
}

function showGpuAlert(alert: Record<string, unknown>): void {
  currentAlert = alert;
  const overlay = document.getElementById("gpuAlertOverlay");
  const title = document.getElementById("gpuAlertTitle");
  const body = document.getElementById("gpuAlertBody");
  const status = document.getElementById("gpuAlertSwitchStatus");
  const install = document.getElementById("gpuAlertInstallBtn") as HTMLElement | null;
  if (title) title.textContent = String(alert.title || "");
  if (body) body.textContent = String(alert.body || "");
  if (status) {
    status.hidden = true;
    status.textContent = "";
  }
  if (install) install.hidden = !alert.download_url;
  if (overlay) overlay.hidden = false;
}

function hideGpuAlert(): void {
  const overlay = document.getElementById("gpuAlertOverlay");
  if (overlay) overlay.hidden = true;
  currentAlert = null;
}

async function ensureStartupDiagnostics(): Promise<void> {
  const deps = activeDeps;
  if (!deps?.canInvoke()) return;
  if (startupDiagnosticsPromise) {
    await startupDiagnosticsPromise;
    return;
  }
  startupDiagnosticsPromise = (async () => {
    try {
      const lines = await deps.invoke<readonly string[]>("gpu_report", {}, {
        section: "hardware",
        bootSafe: true,
        timeoutMs: 5000,
        dedupeKey: "gpu-report",
      });
      for (const line of lines) deps.appendLog(String(line));
    } catch (err) {
      deps.appendLog(`gpu_report failed: ${err}`);
    }

    try {
      const alert = await deps.invoke<Record<string, unknown> | null>("gpu_startup_alert", {}, {
        section: "hardware",
        bootSafe: true,
        timeoutMs: 5000,
        dedupeKey: "gpu-startup-alert",
      });
      if (alert) showGpuAlert(alert);
    } catch (err) {
      deps.appendLog(`gpu_startup_alert failed: ${err}`);
    }

    deps.appendLog("awaiting start signal");
  })();
  await startupDiagnosticsPromise;
}

async function load(): Promise<void> {
  const deps = activeDeps;
  if (!deps) return;
  const cpu = document.getElementById("panelHardwareCpu");
  const gpu = document.getElementById("panelHardwareGpu");
  if (!deps.canInvoke()) {
    if (cpu) cpu.textContent = deps.isFrench() ? "info indisponible" : "info unavailable";
    if (gpu) gpu.textContent = deps.isFrench() ? "info indisponible" : "info unavailable";
    return;
  }
  try {
    deps.trace("hardware.begin");
    const info = await deps.invoke<ForgeHardwareInfo>("get_hardware_info", {}, {
      section: "shell",
      bootSafe: true,
      requiresActiveSection: false,
      timeoutMs: 5000,
      dedupeKey: "hardware",
    });
    deps.trace("hardware.done", {
      cpu: info?.cpu_brand || "",
      gpuCount: Array.isArray(info?.gpus) ? info.gpus.length : 0,
    });
    if (retryTimer) {
      window.clearTimeout(retryTimer);
      retryTimer = 0;
    }
    const normalized = info || null;
    deps.onUpdate(normalized);
    deps.dispatchHardware(normalized);
    deps.invalidateProofPanel();
    if (cpu) {
      const threads = info?.cpu_threads ? `  ·  ${info.cpu_threads} threads` : "";
      cpu.textContent = `${info?.cpu_brand || "Unknown CPU"}${threads}`;
      cpu.title = `${info?.cpu_brand || "Unknown CPU"} (${info?.os || "?"}/${info?.arch || "?"})`;
    }
    renderGpuRows(info?.gpus || [], deps.isFrench());
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    deps.trace("hardware.error", message);
    console.warn("[hardware-info] failed", err);
    deps.onUpdate(null);
    deps.dispatchHardware(null);
    deps.invalidateProofPanel();
    if (cpu) cpu.textContent = deps.isFrench() ? "détection échouée" : "detection failed";
    if (gpu) gpu.textContent = deps.isFrench() ? "détection échouée" : "detection failed";
    if (!retryTimer) {
      retryTimer = window.setTimeout(() => {
        retryTimer = 0;
        void load();
      }, 4000);
    }
  }
}

function scheduleBoot(): void {
  const start = () => {
    window.setTimeout(() => {
      const cpu = document.getElementById("panelHardwareCpu");
      const gpu = document.getElementById("panelHardwareGpu");
      const rows = document.getElementById("panelHardwareGpuRows");
      const stillPending = (cpu?.textContent || "").includes("Detecting")
        || (cpu?.textContent || "").includes("détection")
        || (gpu?.textContent || "").includes("Detecting")
        || (gpu?.textContent || "").includes("détection")
        || !rows?.children?.length;
      if (stillPending) void load();
    }, 2200);
  };
  if (document.readyState === "complete") start();
  else window.addEventListener("load", start, { once: true });
}

function install(deps: ForgeHardwareCellDeps): void {
  activeDeps = deps;
  const install = document.getElementById("gpuAlertInstallBtn");
  const wgpu = document.getElementById("gpuAlertWgpuBtn");
  const dismiss = document.getElementById("gpuAlertDismissBtn");
  const status = document.getElementById("gpuAlertSwitchStatus");

  install?.addEventListener("click", async () => {
    if (!currentAlert?.download_url) return;
    try {
      await deps.invoke("open_url", { url: String(currentAlert.download_url) }, { section: "shell", timeoutMs: 5000 });
      deps.appendLog(`opened in browser: ${currentAlert.download_url}`);
    } catch (err) {
      if (status) {
        status.textContent = `failed to open browser: ${err}`;
        status.hidden = false;
      }
    }
  });

  wgpu?.addEventListener("click", async () => {
    if (status) {
      status.hidden = false;
      status.textContent = "checking WGPU availability...";
    }
    try {
      const result = await deps.invoke<{ readonly message?: string; readonly rebuild_command?: string; readonly status?: string }>("try_switch_to_wgpu", {}, {
        section: "hardware",
        timeoutMs: 15000,
      });
      let text = result.message || "";
      if (result.rebuild_command) text += `\n\n$ ${result.rebuild_command}`;
      if (status) status.textContent = text;
      deps.appendLog(`wgpu switch: ${result.status || ""}`);
    } catch (err) {
      if (status) status.textContent = `wgpu switch failed: ${err}`;
    }
  });

  dismiss?.addEventListener("click", () => {
    deps.appendLog("user dismissed GPU alert — continuing with CPU fallback");
    hideGpuAlert();
  });

  scheduleBoot();
}

window.ForgeHardwareCell = Object.freeze({
  install,
  load,
  ensureStartupDiagnostics,
  formatGpuName,
});

export {};
