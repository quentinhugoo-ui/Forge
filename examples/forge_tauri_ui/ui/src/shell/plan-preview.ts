type PlanPreviewChannel = "forge" | "alpha";

type PlanPreviewPlan = {
  total_planned?: number;
  unique_after_fnv?: number;
  input_dedup_pct?: number;
  distinct_programs?: number;
  n_cse_classes?: number;
  atlas_hits?: number;
  atlas_hit_pct?: number;
  atlas_result_hits?: number;
  atlas_result_pct?: number;
  truly_novel?: number;
};

type PlanPreviewReport = {
  kind?: string;
  node_count?: number;
  opcodes?: string[];
  cse_node_delta?: number;
  plan?: PlanPreviewPlan | null;
  recurring_ops?: string[];
};

export interface ForgePlanPreviewRuntimeOptions {
  canInvoke: () => boolean;
  invoke: (command: string, args?: Record<string, unknown>, options?: Record<string, unknown>) => Promise<unknown>;
  alphaTrace?: (stage: string, details?: unknown) => void;
  getAlphaFileBytes?: (file: File | null) => Promise<number[] | null> | number[] | null;
}

const PRESTART_REPORT_CACHE_VERSION = "alpha-prestart-v1";

function previewCacheKey(kind: string, file: File | null) {
  if (!kind || !file) return null;
  return `${PRESTART_REPORT_CACHE_VERSION}:${kind}:${file.name}:${file.size}:${file.lastModified}`;
}

function loadPreviewReportCache(kind: string, file: File | null) {
  const key = previewCacheKey(kind, file);
  if (!key) return null;
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? parsed : null;
  } catch (_) {
    return null;
  }
}

function persistPreviewReportCache(kind: string, file: File | null, report: unknown) {
  const key = previewCacheKey(kind, file);
  if (!key || !report) return;
  try {
    localStorage.setItem(key, JSON.stringify(report));
  } catch (_) {
    // Best-effort UX cache only.
  }
}

function replayCachedReport(report: unknown) {
  const replay = (typeof structuredClone === "function"
    ? structuredClone(report)
    : JSON.parse(JSON.stringify(report))) as PlanPreviewReport;
  if (Array.isArray(replay.recurring_ops)) {
    replay.recurring_ops = replay.recurring_ops.filter(
      (line: string) => !String(line).startsWith("inspect elapsed :"),
    );
    replay.recurring_ops.push("inspect elapsed : 0 ms (persisted frontend cache hit for same file)");
  }
  return replay;
}

export function logPlanReport(logFn: (line: string) => void, report: unknown) {
  if (!logFn || !report) return;
  const preview = report as PlanPreviewReport;
  const nodeCount = Number(preview.node_count || 0);
  const opcodes = Array.isArray(preview.opcodes) ? preview.opcodes : [];
  const cseNodeDelta = Number(preview.cse_node_delta || 0);
  logFn(`--- plan ${preview.kind || "unknown"} ---`);
  if (nodeCount > 0) {
    logFn(`structure : ${nodeCount} nodes | opcodes [${opcodes.join(", ")}]`);
  }
  if (cseNodeDelta !== 0) {
    logFn(`CSE on stored program : -${cseNodeDelta} nodes after canonical merge`);
  }
  const p = preview.plan;
  if (p) {
    const totalPlanned = Number(p.total_planned || 0);
    const uniqueAfterFnv = Number(p.unique_after_fnv || 0);
    const inputDedupPct = Number(p.input_dedup_pct || 0);
    const distinctPrograms = Number(p.distinct_programs || 0);
    const cseClasses = Number(p.n_cse_classes || 0);
    const atlasHits = Number(p.atlas_hits || 0);
    const atlasHitPct = Number(p.atlas_hit_pct || 0);
    const atlasResultHits = Number(p.atlas_result_hits || 0);
    const atlasResultPct = Number(p.atlas_result_pct || 0);
    const trulyNovel = Number(p.truly_novel || 0);
    logFn(
      `inputs : ${totalPlanned.toLocaleString()} planned -> ${uniqueAfterFnv.toLocaleString()} unique (${inputDedupPct.toFixed(1)}% identical, eliminated pre-dispatch)`,
    );
    if (distinctPrograms > 1) {
      const collapsed = distinctPrograms - cseClasses;
      const pct = distinctPrograms > 0 ? (100 * collapsed) / distinctPrograms : 0;
      logFn(
        `candidates : ${distinctPrograms.toLocaleString()} raw -> ${cseClasses.toLocaleString()} CSE classes (${collapsed.toLocaleString()} redundant programs identified, ${pct.toFixed(1)}%)`,
      );
    }
    if (atlasHits > 0) {
      logFn(
        `atlas peek : ${atlasHits.toLocaleString()} (program x input) already resolvable without the interpreter (${atlasHitPct.toFixed(1)}%)`,
      );
    }
    if (atlasResultHits > 0) {
      logFn(
        `atlas RESULT : ${atlasResultHits.toLocaleString()} / ${uniqueAfterFnv.toLocaleString()} calls already computed (${atlasResultPct.toFixed(1)}%) - Start short-circuited, zero recompute`,
      );
    } else {
      logFn(
        `atlas RESULT : 0 / ${uniqueAfterFnv.toLocaleString()} pre-Start (synth path - results persisted after each Start, reused cross-sessions)`,
      );
    }
    const novelLabel = trulyNovel === 0
      ? "atlas warm - no new computation in the candidate space tested"
      : `${trulyNovel.toLocaleString()} structurally new computations`;
    logFn(`truly novel : ${trulyNovel.toLocaleString()} calls actually to compute [${novelLabel}]`);
  } else {
    logFn("(no file uploaded yet - upload to measure real redundancy)");
  }
  for (const note of preview.recurring_ops || []) {
    logFn(`  - ${note}`);
  }
}

export function createForgePlanPreviewRuntime(options: ForgePlanPreviewRuntimeOptions) {
  let forgeToken = 0;
  let alphaToken = 0;

  async function fileBytes(file: File | null, channel: PlanPreviewChannel) {
    if (!file) return null;
    if (channel === "alpha" && options.getAlphaFileBytes) {
      const bytes = await options.getAlphaFileBytes(file);
      if (bytes) return bytes;
    }
    const buf = await file.arrayBuffer();
    return Array.from(new Uint8Array(buf));
  }

  async function refresh(kind: string, file: File | null, logFn: (line: string) => void, channel: PlanPreviewChannel = "forge") {
    if (!kind || !logFn) return null;
    if (!options.canInvoke()) return null;
    const trace = options.alphaTrace || (() => {});
    if (channel === "alpha") {
      trace("prestart.inspect.enter", {
        kind,
        file: file ? file.name : null,
        size: file ? file.size : 0,
      });
    }
    const token = channel === "alpha" ? ++alphaToken : ++forgeToken;
    const current = () => (channel === "alpha" ? alphaToken : forgeToken);
    try {
      const cachedReport = channel !== "alpha" && file ? loadPreviewReportCache(kind, file) : null;
      if (cachedReport) {
        const replay = replayCachedReport(cachedReport);
        logPlanReport(logFn, replay);
        if (channel === "alpha") return replay;
      }
      const bytes = await fileBytes(file, channel);
      if (token !== current()) return null;
      if (channel === "alpha") {
        trace("prestart.inspect.invoke", {
          kind,
          bytes: bytes ? bytes.length : 0,
        });
      }
      const report = await options.invoke(
        "inspect_program_map",
        { kind, bytes },
        { section: channel === "alpha" ? "alpha" : "forge", timeoutMs: 15000 },
      );
      if (token !== current()) return null;
      if (file && channel !== "alpha") persistPreviewReportCache(kind, file, report);
      const preview = report as PlanPreviewReport;
      if (channel === "alpha") {
        trace("prestart.inspect.done", {
          kind,
          hasPlan: !!preview.plan,
          recurringOps: Array.isArray(preview.recurring_ops) ? preview.recurring_ops.length : 0,
        });
      }
      logPlanReport(logFn, report);
      return report;
    } catch (err) {
      if (token !== current()) return null;
      if (channel === "alpha") trace("prestart.inspect.error", String(err));
      logFn(`inspect_program_map error: ${err}`);
      return null;
    }
  }

  return { refresh };
}
