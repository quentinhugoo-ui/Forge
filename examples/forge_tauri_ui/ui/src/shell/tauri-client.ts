import type { ForgeTauriClient } from "./types.js";

type ForgeTauriCommandOptions = {
  readonly bootSafe?: boolean;
  readonly requiresActiveSection?: boolean;
  readonly section?: string;
  readonly timeoutMs?: number;
  readonly trace?: boolean;
  readonly dedupeKey?: string;
};

declare global {
  interface Window {
    ForgeTauriBridge?: ForgeTauriClient & {
      debugLog?: (stage: string, details?: unknown) => Promise<boolean>;
      isAvailable?: () => boolean;
      getStats?: () => Record<string, unknown>;
      bootBlockedCommands?: () => string[];
    };
  }
}

export const forgeTauriCommandContracts: Readonly<Record<string, ForgeTauriCommandOptions>> = Object.freeze({
  forge_kernel: { bootSafe: true, requiresActiveSection: false, timeoutMs: 2500 },
  get_hardware_info: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "hardware" },
  gpu_report: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "gpu-report" },
  gpu_startup_alert: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "gpu-alert" },
  list_forge_jobs: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "jobs" },
  forge_job_runtime_snapshot: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "job-ledger" },
  create_forge_pending_job: { bootSafe: true, requiresActiveSection: false, timeoutMs: 15000 },
  update_forge_job: { bootSafe: true, requiresActiveSection: false, timeoutMs: 8000 },
  read_forge_job_log: { bootSafe: true, requiresActiveSection: false, timeoutMs: 8000 },
  read_forge_job_manifest: { bootSafe: true, requiresActiveSection: false, timeoutMs: 8000 },
  read_forge_job_file: { bootSafe: true, requiresActiveSection: false, timeoutMs: 10000 },
  prepare_alpha_backend: { bootSafe: true, requiresActiveSection: false, timeoutMs: 20000, dedupeKey: "prepare-alpha" },
  start_computation: { section: "alpha", requiresActiveSection: true, timeoutMs: 120000 },
  start_alpha_synthesis: { section: "alpha", requiresActiveSection: true, timeoutMs: 120000 },
  webexplorer_native_present: { section: "webexplorer", requiresActiveSection: true, timeoutMs: 15000, dedupeKey: "present" },
  webexplorer_native_hide: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "hide" },
  bloomberg_live_native_present: { section: "trading", requiresActiveSection: true, timeoutMs: 15000, dedupeKey: "present" },
  bloomberg_live_native_hide: { bootSafe: true, requiresActiveSection: false, timeoutMs: 5000, dedupeKey: "hide" },
  real_estate_harvester_snapshot: { section: "real-estate", timeoutMs: 6000, dedupeKey: "snapshot" },
  real_estate_onboarding_state: { section: "real-estate", timeoutMs: 6000, dedupeKey: "onboarding-state" },
  real_estate_onboarding_answer: { section: "real-estate", timeoutMs: 8000 },
  real_estate_tool_command_context: { section: "real-estate", timeoutMs: 8000 },
  minimize_main_window: { bootSafe: true, requiresActiveSection: false, timeoutMs: 2500 },
  toggle_maximize_main_window: { bootSafe: true, requiresActiveSection: false, timeoutMs: 2500 },
  close_main_window: { bootSafe: true, requiresActiveSection: false, timeoutMs: 2500 },
  drag_main_window: { bootSafe: true, requiresActiveSection: false, timeoutMs: 700 },
});

function optionsFor(command: string, options: Record<string, unknown>): ForgeTauriCommandOptions {
  return Object.freeze({
    ...(forgeTauriCommandContracts[command] || {}),
    ...options,
  });
}

export function createForgeTauriClient(): ForgeTauriClient {
  return {
    async invoke<T>(command: string, args: Record<string, unknown> = {}, options: Record<string, unknown> = {}) {
      const resolvedOptions = optionsFor(command, options);
      if (window.ForgeTauriBridge?.invoke) {
        return window.ForgeTauriBridge.invoke<T>(command, args, resolvedOptions as Record<string, unknown>);
      }
      throw new Error(`Tauri command unavailable: ${command}`);
    },
  };
}
