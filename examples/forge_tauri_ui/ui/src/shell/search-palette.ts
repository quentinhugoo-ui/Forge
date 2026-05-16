type ForgeJob = Record<string, unknown>;

type SearchItem = {
  readonly type: "session" | "program";
  readonly title: string;
  readonly folder: string;
  readonly timestamp: number;
  readonly action: () => void;
};

type ForgeSearchPaletteDeps = {
  canInvoke(): boolean;
  invoke<T = unknown>(command: string, args?: Record<string, unknown>, options?: Record<string, unknown>): Promise<T>;
  jobs(): readonly ForgeJob[];
  jobLabel(job: ForgeJob): string;
  selectJob(jobId: string, job: ForgeJob): void;
  openPrograms(): void;
  runtime?: {
    registerAction?(name: "open-search" | "close-search" | "toggle-search", handler: () => void): unknown;
  } | null;
};

type ForgeSearchPalette = {
  install(deps: ForgeSearchPaletteDeps): void;
  open(): Promise<void>;
  close(): void;
  toggle(): void;
};

declare global {
  interface Window {
    __forgeOpenSearch?: () => void;
    ForgeSearchPalette?: ForgeSearchPalette;
  }
}

const ICONS = Object.freeze({
  session: '<svg class="forge-search-row-icon" viewBox="0 0 24 24"><path d="m8 6-5 6 5 6"/><path d="m16 6 5 6-5 6"/><path d="m14 4-4 16"/></svg>',
  program: '<svg class="forge-search-row-icon" viewBox="0 0 24 24"><rect x="4" y="4" width="16" height="16" rx="2"/><path d="M8 8h8M8 12h8M8 16h5"/></svg>',
  document: '<svg class="forge-search-row-icon" viewBox="0 0 24 24"><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/></svg>',
});

let deps: ForgeSearchPaletteDeps | null = null;
let activeIdx = 0;
let programs: string[] = [];
let visibleRows: SearchItem[] = [];

function $(id: string): HTMLElement | null {
  return document.getElementById(id);
}

function input(): HTMLInputElement | null {
  const node = $("forgeSearchInput");
  return node instanceof HTMLInputElement ? node : null;
}

function relativeTime(ms: number): string {
  if (!ms) return "";
  const diff = Date.now() - ms;
  if (diff < 60_000) return "Just now";
  if (diff < 3_600_000) return "Last hour";
  if (diff < 86_400_000) return "Today";
  if (diff < 7 * 86_400_000) return "This week";
  if (diff < 30 * 86_400_000) return "Last month";
  return new Date(ms).toISOString().slice(0, 10);
}

function extractFolder(job: ForgeJob): string {
  const path = job.filePath || job.file_path || job.input_path || job.path || "";
  if (!path) return "";
  const parts = String(path).replace(/\\/g, "/").split("/").filter(Boolean);
  return parts.length < 2 ? "" : (parts[parts.length - 2] || "");
}

function jobTimestamp(job: ForgeJob): number {
  return Number(job.updated_at_ms || job.updatedAt || job.created_at_ms || job.createdAt || 0);
}

function jobId(job: ForgeJob): string {
  return String(job.jobId || job.job_id || "");
}

function buildItems(): SearchItem[] {
  const d = deps;
  if (!d) return [];
  const items: SearchItem[] = [];
  for (const job of d.jobs()) {
    if (!job) continue;
    items.push({
      type: "session",
      title: d.jobLabel(job),
      folder: extractFolder(job),
      timestamp: jobTimestamp(job),
      action: () => {
        const id = jobId(job);
        if (id) d.selectJob(id, job);
      },
    });
  }
  for (const name of programs) {
    items.push({
      type: "program",
      title: name,
      folder: "",
      timestamp: 0,
      action: () => d.openPrograms(),
    });
  }
  return items;
}

function selectRow(index: number): void {
  const item = visibleRows[index];
  if (!item) return;
  close();
  try {
    item.action();
  } catch (err) {
    console.error("[forge-search] action failed", err);
  }
}

function render(query: string): void {
  const results = $("forgeSearchResults");
  const empty = $("forgeSearchEmpty");
  if (!results) return;
  const q = (query || "").trim().toLowerCase();
  let items = buildItems();
  if (q) items = items.filter((item) => item.title.toLowerCase().includes(q) || item.folder.toLowerCase().includes(q));
  const byFolder = new Map<string, SearchItem[]>();
  for (const item of items) {
    const key = item.folder || "__root__";
    const bucket = byFolder.get(key) || [];
    bucket.push(item);
    byFolder.set(key, bucket);
  }
  const keys = [...byFolder.keys()].sort((a, b) => {
    if (a === "__root__") return 1;
    if (b === "__root__") return -1;
    return a.localeCompare(b);
  });
  results.innerHTML = "";
  visibleRows = [];
  let rowIdx = 0;
  for (const key of keys) {
    if (key !== "__root__") {
      const section = document.createElement("div");
      section.className = "forge-search-section";
      section.textContent = key;
      results.appendChild(section);
    }
    for (const item of byFolder.get(key) || []) {
      const currentIdx = rowIdx;
      const row = document.createElement("div");
      row.className = `forge-search-row${currentIdx === activeIdx ? " is-active" : ""}`;
      row.dataset.idx = String(currentIdx);
      row.setAttribute("role", "option");
      row.innerHTML = ICONS[item.type] || ICONS.document;
      const text = document.createElement("span");
      text.className = "forge-search-row-text";
      text.textContent = item.title;
      row.appendChild(text);
      const meta = document.createElement("span");
      meta.className = "forge-search-row-meta";
      meta.textContent = currentIdx === activeIdx ? "Enter" : relativeTime(item.timestamp);
      row.appendChild(meta);
      row.addEventListener("click", () => selectRow(currentIdx));
      results.appendChild(row);
      visibleRows.push(item);
      rowIdx += 1;
    }
  }
  if (empty) empty.hidden = visibleRows.length > 0;
}

async function open(): Promise<void> {
  const d = deps;
  const overlay = $("forgeSearchOverlay");
  if (!d || !overlay) return;
  overlay.hidden = false;
  overlay.setAttribute("aria-hidden", "false");
  activeIdx = 0;
  try {
    if (d.canInvoke()) {
      const names = await d.invoke<unknown>("list_programs", {}, {
        section: "programs",
        timeoutMs: 5000,
        dedupeKey: "search-programs",
      });
      programs = Array.isArray(names) ? names.map(String) : [];
    }
  } catch (err) {
    console.warn("[forge-search] list_programs failed", err);
  }
  render(input()?.value || "");
  window.setTimeout(() => input()?.focus(), 30);
}

function close(): void {
  const overlay = $("forgeSearchOverlay");
  const searchInput = input();
  const results = $("forgeSearchResults");
  if (!overlay) return;
  overlay.hidden = true;
  overlay.setAttribute("aria-hidden", "true");
  if (searchInput) searchInput.value = "";
  activeIdx = 0;
  visibleRows = [];
  if (results) results.innerHTML = "";
}

function toggle(): void {
  const overlay = $("forgeSearchOverlay");
  if (overlay?.hidden) void open();
  else close();
}

function install(nextDeps: ForgeSearchPaletteDeps): void {
  deps = nextDeps;
  window.__forgeOpenSearch = () => { void open(); };
  nextDeps.runtime?.registerAction?.("open-search", () => { void open(); });
  nextDeps.runtime?.registerAction?.("close-search", () => close());
  nextDeps.runtime?.registerAction?.("toggle-search", () => toggle());
  input()?.addEventListener("input", () => {
    activeIdx = 0;
    render(input()?.value || "");
  });
  input()?.addEventListener("keydown", (event) => {
    const searchInput = input();
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (activeIdx < visibleRows.length - 1) activeIdx += 1;
      render(searchInput?.value || "");
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      if (activeIdx > 0) activeIdx -= 1;
      render(searchInput?.value || "");
    } else if (event.key === "Enter") {
      event.preventDefault();
      selectRow(activeIdx);
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  });
}

window.ForgeSearchPalette = Object.freeze({
  install,
  open,
  close,
  toggle,
});

export {};
