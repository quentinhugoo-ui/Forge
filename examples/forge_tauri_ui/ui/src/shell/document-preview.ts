export interface ForgeDocumentPreviewOptions {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  fileList: HTMLElement;
  resultLogs: string[];
  forgeLogs: string[];
  getActiveTab: () => string;
  humanSize: (bytes: number) => string;
  appendForge: (line: string) => void;
}

type Candle = {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
};

type DocumentPreviewMode = "dna" | "candles";

type RunPhase = "idle" | "ready" | "running" | "done" | "error";

type RunReport = {
  kind?: string;
  total_kmers?: number;
  distinct?: number;
  elapsed_ms?: number;
  ns_per_kmer?: number;
};

type DocumentPreviewState = {
  fileName: string;
  fileSize: number;
  rawText: string;
  wrappedLines: string[];
  lineStartOffsets: number[];
  charWidth: number;
  lineHeight: number;
  leftPad: number;
  topPad: number;
  scrollLines: number;
  dnaSequence: string;
  dnaRawOffsets: number[];
  mode: DocumentPreviewMode;
  candles: Candle[];
  candleViewCount: number;
  candleViewEnd: number;
  candleHover: number;
};

type RunState = {
  phase: RunPhase;
  chunkDone: number;
  chunkTotal: number;
  report: RunReport | null;
};

const MASK64 = (1n << 64n) - 1n;
const SM_C0 = 0x9e3779b97f4a7c15n;
const SM_C1 = 0xbf58476d1ce4e5b9n;
const SM_C2 = 0x94d049bb133111ebn;
const FLAP_CHARS = "ACGT01abcdef23456789.";
const CYCLE_FRAMES = 14;
const SCRAMBLE_DUR = 4;
const GROUP_SIZE = 4;

export function tryParseCandleCsv(txt: string): Candle[] | null {
  if (txt.charCodeAt(0) === 0xfeff) txt = txt.slice(1);
  const newline = txt.indexOf("\n");
  if (newline < 0) return null;
  const header = txt.slice(0, newline).trim().toLowerCase();
  const headerCols = header.split(",").map((s) => s.trim());
  const expected = ["time", "open", "high", "low", "close"];
  for (let i = 0; i < expected.length; i += 1) {
    if (headerCols[i] !== expected[i]) return null;
  }
  const hasVolume = headerCols[5] === "volume";
  const out: Candle[] = [];
  for (const raw of txt.split(/\r?\n/).slice(1)) {
    const line = raw.trim();
    if (!line) continue;
    const cols = line.split(",");
    if (cols.length < 5) continue;
    const rawTime = (cols[0] || "").trim();
    const time = /^\d+$/.test(rawTime) ? Number(rawTime) : Date.parse(rawTime);
    const open = Number.parseFloat(cols[1] || "");
    const high = Number.parseFloat(cols[2] || "");
    const low = Number.parseFloat(cols[3] || "");
    const close = Number.parseFloat(cols[4] || "");
    const volume = hasVolume && cols[5] ? Number.parseFloat(cols[5]) : 0;
    if (![time, open, high, low, close].every(Number.isFinite)) continue;
    out.push({ time, open, high, low, close, volume: Number.isFinite(volume) ? volume : 0 });
  }
  return out.length >= 2 ? out : null;
}

export function createForgeDocumentPreview(options: ForgeDocumentPreviewOptions) {
  const { canvas, ctx, fileList, resultLogs, forgeLogs, getActiveTab, humanSize, appendForge } = options;
  let dpr = window.devicePixelRatio || 1;
  let width = 0;
  let height = 0;
  let previewToken = 0;
  let doneTimestamp = 0;
  let frameCount = 0;
  let candleDragStartX = 0;
  let candleDragStartViewEnd = 0;
  let candleDragging = false;

  const docState: DocumentPreviewState = {
    fileName: "",
    fileSize: 0,
    rawText: "",
    wrappedLines: [] as string[],
    lineStartOffsets: [] as number[],
    charWidth: 8.2,
    lineHeight: 16,
    leftPad: 18,
    topPad: 16,
    scrollLines: 0,
    dnaSequence: "",
    dnaRawOffsets: [] as number[],
    mode: "dna",
    candles: [] as Candle[],
    candleViewCount: 200,
    candleViewEnd: 0,
    candleHover: -1,
  };

  const runState: RunState = {
    phase: "idle",
    chunkDone: 0,
    chunkTotal: 0,
    report: null,
  };

  function nucBits(c: string) {
    if (c === "A" || c === "a") return 0;
    if (c === "C" || c === "c") return 1;
    if (c === "G" || c === "g") return 2;
    if (c === "T" || c === "t") return 3;
    return 255;
  }

  function packKmer(kmer: string) {
    let value = 0n;
    let reset = false;
    for (let i = 0; i < kmer.length; i += 1) {
      const bits = nucBits(kmer[i] || "");
      if (bits === 255) {
        reset = true;
        value = 0n;
        continue;
      }
      value = ((value << 2n) | BigInt(bits)) & MASK64;
    }
    return { value, reset };
  }

  function splitmix64Steps(x0: bigint) {
    const after_add = (x0 + SM_C0) & MASK64;
    const after_mul1 = ((after_add ^ (after_add >> 30n)) * SM_C1) & MASK64;
    const after_mul2 = ((after_mul1 ^ (after_mul1 >> 27n)) * SM_C2) & MASK64;
    const final_hash = (after_mul2 ^ (after_mul2 >> 31n)) & MASK64;
    return { after_add, after_mul1, after_mul2, final_hash };
  }

  function pad2(n: number) {
    return n < 10 ? `0${n}` : String(n);
  }

  function easeOut(t: number) {
    return 1 - Math.pow(1 - Math.min(1, t), 3);
  }

  function wrapLineByWidth(line: string, maxChars: number) {
    if (line.length <= maxChars) return [line];
    const out: string[] = [];
    for (let i = 0; i < line.length; i += maxChars) out.push(line.slice(i, i + maxChars));
    return out;
  }

  function visibleDocLines() {
    return Math.max(1, Math.floor((height - docState.topPad * 2) / docState.lineHeight));
  }

  function recomputeWrap() {
    if (!docState.rawText) {
      docState.wrappedLines = [];
      docState.lineStartOffsets = [];
      return;
    }
    const usable = Math.max(20, width - docState.leftPad * 2 - 12);
    const maxChars = Math.max(12, Math.floor(usable / docState.charWidth));
    const wrapped: string[] = [];
    const starts: number[] = [];
    let globalOffset = 0;
    for (const srcLine of docState.rawText.split(/\r?\n/)) {
      const chunks = wrapLineByWidth(srcLine, maxChars);
      if (chunks.length === 0) {
        wrapped.push("");
        starts.push(globalOffset);
      } else {
        for (let i = 0; i < chunks.length; i += 1) {
          wrapped.push(chunks[i] || "");
          starts.push(globalOffset + i * maxChars);
        }
      }
      globalOffset += srcLine.length + 1;
    }
    docState.wrappedLines = wrapped;
    docState.lineStartOffsets = starts;
    const maxScroll = Math.max(0, wrapped.length - visibleDocLines());
    docState.scrollLines = Math.min(docState.scrollLines, maxScroll);
  }

  function extractDnaProjection(raw: string) {
    let sequence = "";
    const offsets: number[] = [];
    const upper = raw.toUpperCase();
    for (let i = 0; i < upper.length; i += 1) {
      const c = upper[i];
      if (c === "A" || c === "C" || c === "G" || c === "T" || c === "N") {
        sequence += c;
        offsets.push(i);
      }
    }
    return { sequence, offsets };
  }

  function mapCharToVisiblePosition(globalCharIndex: number) {
    if (docState.wrappedLines.length === 0) return null;
    let lineIndex = -1;
    for (let i = 0; i < docState.lineStartOffsets.length; i += 1) {
      const start = docState.lineStartOffsets[i] ?? 0;
      const end = start + (docState.wrappedLines[i] || "").length;
      if (globalCharIndex >= start && globalCharIndex <= end) {
        lineIndex = i;
        break;
      }
    }
    if (lineIndex < 0) return null;
    const row = lineIndex - docState.scrollLines;
    if (row < 0 || row >= visibleDocLines()) return null;
    const col = Math.max(0, globalCharIndex - (docState.lineStartOffsets[lineIndex] ?? 0));
    return {
      x: docState.leftPad + col * docState.charWidth,
      y: docState.topPad + row * docState.lineHeight,
      lineIndex,
    };
  }

  function findWrappedLineIndex(globalCharIndex: number) {
    for (let i = 0; i < docState.lineStartOffsets.length; i += 1) {
      const start = docState.lineStartOffsets[i] ?? 0;
      const end = start + (docState.wrappedLines[i] || "").length;
      if (globalCharIndex >= start && globalCharIndex <= end) return i;
    }
    return -1;
  }

  function drawMicroGrid(time: number) {
    const spacing = 18;
    const pulse = time * 0.00018;
    ctx.fillStyle = "#0e1011";
    ctx.fillRect(0, 0, width, height);
    for (let gy = spacing; gy < height; gy += spacing) {
      for (let gx = spacing; gx < width; gx += spacing) {
        const phase = gx * 0.004 + gy * 0.003 + pulse;
        const alpha = 0.07 + Math.sin(phase) * 0.03;
        ctx.fillStyle = `rgba(210, 224, 230, ${Math.max(0.02, alpha).toFixed(3)})`;
        ctx.fillRect(gx, gy, 1.45, 1.45);
      }
    }
  }

  function drawPlainLogs(lines: string[], placeholder: string) {
    ctx.font = "12px Consolas, Monaco, monospace";
    ctx.textBaseline = "top";
    ctx.textAlign = "left";
    const x = 18;
    const top = 16;
    const lineHeight = 18;
    const visible = Math.max(1, Math.floor((height - top * 2) / lineHeight));
    const start = Math.max(0, lines.length - visible);
    if (lines.length === 0) {
      ctx.fillStyle = "rgba(138,150,156,0.75)";
      ctx.fillText(placeholder, x, top);
      return;
    }
    ctx.fillStyle = "rgba(225,234,238,0.94)";
    for (let i = start; i < lines.length; i += 1) {
      ctx.fillText(lines[i] || "", x, top + (i - start) * lineHeight);
    }
  }

  function drawDocumentText() {
    const lines = docState.wrappedLines;
    if (lines.length === 0) return;
    const first = docState.scrollLines;
    const visible = visibleDocLines();
    const last = Math.min(lines.length, first + visible);
    ctx.font = "12px Consolas, Monaco, monospace";
    ctx.textBaseline = "top";
    ctx.fillStyle = "rgba(223,233,237,0.95)";
    for (let i = first; i < last; i += 1) {
      ctx.fillText(lines[i] || "", docState.leftPad, docState.topPad + (i - first) * docState.lineHeight);
    }
    const barH = Math.max(24, (visible / lines.length) * (height - 28));
    const maxScroll = Math.max(1, lines.length - visible);
    const barY = 14 + (docState.scrollLines / maxScroll) * Math.max(1, height - 28 - barH);
    ctx.fillStyle = "rgba(57,69,75,0.85)";
    ctx.fillRect(width - 8, 12, 2, height - 24);
    ctx.fillStyle = "rgba(152,174,183,0.9)";
    ctx.fillRect(width - 9, barY, 4, barH);
  }

  function flapRand(frame: number, charIdx: number) {
    let x = (frame * 2654435761 + charIdx * 40503) >>> 0;
    x = Math.imul(x ^ (x >>> 16), 0x45d9f3b) >>> 0;
    return (x ^ (x >>> 16)) >>> 0;
  }

  function drawComputingView() {
    frameCount += 1;
    const seqLen = docState.dnaSequence.length;
    if (seqLen < 32) return;
    const cycleFrame = frameCount % CYCLE_FRAMES;
    const kmerOrdinal = Math.floor(frameCount / CYCLE_FRAMES);
    const idx = kmerOrdinal % Math.max(1, seqLen - 32);
    const kmer = docState.dnaSequence.slice(idx, idx + 32);
    const { value: packed } = packKmer(kmer);
    const hexHash = splitmix64Steps(packed).final_hash.toString(16).padStart(16, "0");
    const rawIdx = docState.dnaRawOffsets[Math.min(idx, docState.dnaRawOffsets.length - 1)] ?? 0;
    let p = mapCharToVisiblePosition(rawIdx);
    if (!p) {
      const lineIndex = findWrappedLineIndex(rawIdx);
      if (lineIndex >= 0) {
        const visible = visibleDocLines();
        docState.scrollLines = Math.min(
          Math.max(0, docState.wrappedLines.length - visible),
          Math.max(0, lineIndex - Math.floor(visible * 0.35)),
        );
        p = mapCharToVisiblePosition(rawIdx);
      }
    }
    if (!p || p.x + 32 * docState.charWidth > width - 4) return;
    ctx.font = "12px Consolas, Monaco, monospace";
    ctx.textBaseline = "top";
    ctx.fillStyle = "#0e1011";
    ctx.fillRect(p.x - 1, p.y - 1, 32 * docState.charWidth + 2, docState.lineHeight + 2);
    for (let i = 0; i < 32; i += 1) {
      const group = Math.floor(i / GROUP_SIZE);
      const lockAt = group + SCRAMBLE_DUR;
      const cx = p.x + i * docState.charWidth;
      if (cycleFrame < group) {
        ctx.fillStyle = "rgba(223,233,237,0.72)";
        ctx.fillText(kmer[i] || "", cx, p.y);
      } else if (cycleFrame < lockAt) {
        const r = flapRand(frameCount, i) % FLAP_CHARS.length;
        ctx.fillStyle = "rgba(200,215,222,0.50)";
        ctx.fillText(FLAP_CHARS[r] || "", cx, p.y);
      } else {
        ctx.fillStyle = "rgba(140,232,170,0.96)";
        ctx.fillText(hexHash[Math.floor(i / 2) % 16] || "", cx, p.y);
      }
    }
    const allLocked = cycleFrame >= Math.ceil(32 / GROUP_SIZE) - 1 + SCRAMBLE_DUR;
    ctx.font = "9px Segoe UI, sans-serif";
    ctx.fillStyle = allLocked ? "rgba(140,232,170,0.55)" : "rgba(133,196,223,0.45)";
    ctx.fillText(allLocked ? "hash" : "SplitMix64", p.x, p.y - 10);
  }

  function logLineStyle(line: string) {
    if (line.startsWith("===") || line.startsWith("--")) return { color: "rgba(133,196,223,0.9)", font: "11px Segoe UI, sans-serif" };
    if (/^\s*class\s+\d+/.test(line)) return { color: "rgba(184,224,240,0.85)", font: "11px Consolas, Monaco, monospace" };
    if (/^[ACGT]{8}/.test(line.trim())) return { color: "rgba(200,215,222,0.82)", font: "11px Consolas, Monaco, monospace" };
    if (/^\d/.test(line.trim()) || /k-mers/.test(line)) return { color: "rgba(170,185,195,0.80)", font: "11px Consolas, Monaco, monospace" };
    return { color: "rgba(140,155,165,0.75)", font: "11px Consolas, Monaco, monospace" };
  }

  function drawResultsDoneView(time: number) {
    if (doneTimestamp === 0) doneTimestamp = time;
    const age = (time - doneTimestamp) / 1000;
    const r = runState.report;
    if (!r) return;
    const pad = 22;
    const alpha = easeOut(age / 0.35);
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    ctx.font = "11px Segoe UI, sans-serif";
    const totalKmers = Number(r.total_kmers || 0);
    const distinct = Number(r.distinct || 0);
    const elapsedMs = Number(r.elapsed_ms || 0);
    const nsPerKmer = Number(r.ns_per_kmer || 0);
    const divFrac = totalKmers > 0 ? distinct / totalKmers : 0;
    const statLine = [
      { label: r.kind || "run", color: `rgba(133,196,223,${alpha})` },
      { label: "|", color: `rgba(60,75,85,${alpha})` },
      { label: `${totalKmers.toLocaleString("en-GB")} k-mers`, color: `rgba(200,215,222,${alpha})` },
      { label: "|", color: `rgba(60,75,85,${alpha})` },
      { label: `${distinct.toLocaleString("en-GB")} distinct`, color: `rgba(200,215,222,${alpha})` },
      { label: "|", color: `rgba(60,75,85,${alpha})` },
      { label: `${(divFrac * 100).toFixed(1)}% diversity`, color: `rgba(140,232,170,${alpha * 0.9})` },
      { label: "|", color: `rgba(60,75,85,${alpha})` },
      { label: `${elapsedMs.toFixed(0)} ms`, color: `rgba(200,215,222,${alpha})` },
      { label: `(${nsPerKmer.toFixed(0)} ns/k-mer)`, color: `rgba(120,140,152,${alpha * 0.8})` },
    ];
    let sx = pad;
    for (const { label, color } of statLine) {
      ctx.fillStyle = color;
      ctx.fillText(label, sx, 12);
      sx += ctx.measureText(label).width + 6;
    }
    const barY = 28;
    const barW = Math.min(300, width - pad * 2);
    const animFrac = divFrac * easeOut(Math.max(0, (age - 0.2) / 0.5));
    ctx.fillStyle = `rgba(30,40,48,${alpha})`;
    ctx.fillRect(pad, barY, barW, 1);
    ctx.fillStyle = `rgba(140,232,170,${alpha})`;
    ctx.fillRect(pad, barY, barW * animFrac, 1);
    const sepY = 38;
    ctx.fillStyle = `rgba(35,47,56,${alpha})`;
    ctx.fillRect(pad, sepY, width - pad * 2, 1);
    const logsY = sepY + 8;
    const lineH = 15;
    const maxLines = Math.max(1, Math.floor((height - logsY - 8) / lineH));
    const startLine = Math.max(0, resultLogs.length - maxLines);
    for (let i = startLine; i < resultLogs.length; i += 1) {
      const line = resultLogs[i] || "";
      if (!line.trim()) continue;
      const lineAge = Math.max(0, age - 0.25 - (i - startLine) * 0.018);
      const entryAlpha = easeOut(lineAge / 0.2);
      const { color, font } = logLineStyle(line);
      ctx.font = font;
      ctx.fillStyle = color.replace(/,([\d.]+)\)$/, (_, a) => `,${(Number.parseFloat(a) * entryAlpha).toFixed(2)})`);
      ctx.fillText(line, pad, logsY + (i - startLine) * lineH);
    }
  }

  function drawResultFooter() {
    ctx.font = "10px Segoe UI, sans-serif";
    ctx.fillStyle = "rgba(120,135,145,0.8)";
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    if (!docState.fileName) return;
    ctx.fillText(`${docState.fileName} (${humanSize(docState.fileSize)})`, 18, height - 18);
  }

  function drawCandlesChart() {
    const candles = docState.candles;
    if (!candles.length) return;
    const W = canvas.clientWidth;
    const H = canvas.clientHeight;
    const padTop = 36;
    const padBottom = 24;
    const padLeft = 12;
    const padRight = 64;
    const chartW = Math.max(50, W - padLeft - padRight);
    const chartH = Math.max(50, H - padTop - padBottom);
    const viewCount = Math.max(5, Math.min(docState.candleViewCount, candles.length));
    const viewEnd = Math.max(viewCount - 1, Math.min(docState.candleViewEnd, candles.length - 1));
    const visible = candles.slice(viewEnd - viewCount + 1, viewEnd + 1);
    let yMin = Infinity;
    let yMax = -Infinity;
    for (const c of visible) {
      yMin = Math.min(yMin, c.low);
      yMax = Math.max(yMax, c.high);
    }
    if (!Number.isFinite(yMin) || !Number.isFinite(yMax) || yMin === yMax) {
      yMin = (yMin || 0) - 1;
      yMax = (yMax || 0) + 1;
    }
    const yRange = yMax - yMin;
    yMin -= yRange * 0.05;
    yMax += yRange * 0.05;
    const xOf = (i: number) => padLeft + (i + 0.5) * (chartW / viewCount);
    const yOf = (price: number) => padTop + (1 - (price - yMin) / (yMax - yMin)) * chartH;
    const candleW = Math.max(1, (chartW / viewCount) * 0.7);
    ctx.fillStyle = "rgba(8, 10, 14, 1)";
    ctx.fillRect(0, 0, W, H);
    ctx.strokeStyle = "rgba(255, 255, 255, 0.06)";
    ctx.lineWidth = 1;
    ctx.font = "10px ui-monospace, monospace";
    ctx.fillStyle = "rgba(180, 180, 180, 0.85)";
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    for (let i = 0; i <= 6; i += 1) {
      const price = yMin + (yMax - yMin) * (i / 6);
      const y = yOf(price);
      ctx.beginPath();
      ctx.moveTo(padLeft, y);
      ctx.lineTo(padLeft + chartW, y);
      ctx.stroke();
      ctx.fillText(price.toFixed(3), padLeft + chartW + 6, y);
    }
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (let i = 0; i < 5; i += 1) {
      const idx = Math.floor((visible.length - 1) * (i / 4));
      const c = visible[idx];
      if (!c) continue;
      const d = new Date(c.time);
      const label = viewCount > 100
        ? `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())}`
        : `${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())} ${pad2(d.getUTCHours())}:00`;
      ctx.fillText(label, xOf(idx), padTop + chartH + 4);
    }
    for (let i = 0; i < visible.length; i += 1) {
      const c = visible[i];
      if (!c) continue;
      const x = xOf(i);
      const color = c.close >= c.open ? "rgba(80, 200, 130, 1)" : "rgba(220, 90, 90, 1)";
      ctx.strokeStyle = color;
      ctx.beginPath();
      ctx.moveTo(x, yOf(c.high));
      ctx.lineTo(x, yOf(c.low));
      ctx.stroke();
      const yOpen = yOf(c.open);
      const yClose = yOf(c.close);
      ctx.fillStyle = color;
      ctx.fillRect(x - candleW / 2, Math.min(yOpen, yClose), candleW, Math.max(1, Math.abs(yClose - yOpen)));
    }
    ctx.fillStyle = "rgba(220, 220, 220, 1)";
    ctx.font = "bold 13px ui-monospace, monospace";
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    ctx.fillText(docState.fileName.replace(/\.csv$/i, ""), padLeft, 8);
    ctx.font = "11px ui-monospace, monospace";
    ctx.fillStyle = "rgba(160, 160, 160, 1)";
    ctx.textAlign = "right";
    ctx.fillText(`${candles.length} bars - view ${viewCount} - [${yMin.toFixed(3)}, ${yMax.toFixed(3)}]`, W - padRight - 6, 10);
    if (docState.candleHover >= 0 && docState.candleHover < visible.length) {
      const c = visible[docState.candleHover];
      if (!c) return;
      const x = xOf(docState.candleHover);
      ctx.strokeStyle = "rgba(255, 255, 255, 0.18)";
      ctx.setLineDash([3, 3]);
      ctx.beginPath();
      ctx.moveTo(x, padTop);
      ctx.lineTo(x, padTop + chartH);
      ctx.stroke();
      ctx.setLineDash([]);
      const colorAccent = c.close >= c.open ? "rgba(80, 200, 130, 1)" : "rgba(220, 90, 90, 1)";
      const tip: string[] = [
        new Date(c.time).toISOString().replace("T", " ").slice(0, 16),
        `O ${c.open.toFixed(3)}`,
        `H ${c.high.toFixed(3)}`,
        `L ${c.low.toFixed(3)}`,
        `C ${c.close.toFixed(3)}`,
        c.volume ? `V ${c.volume}` : "",
      ].filter(Boolean) as string[];
      const tipPad = 8;
      const tipLineH = 14;
      const tipW = 150;
      const tipH = tipPad * 2 + tip.length * tipLineH;
      let tipX = x + 12;
      if (tipX + tipW > W - padRight) tipX = x - tipW - 12;
      const tipY = padTop + 12;
      ctx.fillStyle = "rgba(20, 24, 30, 0.92)";
      ctx.fillRect(tipX, tipY, tipW, tipH);
      ctx.strokeStyle = colorAccent;
      ctx.strokeRect(tipX + 0.5, tipY + 0.5, tipW - 1, tipH - 1);
      ctx.font = "11px ui-monospace, monospace";
      ctx.fillStyle = "rgba(220, 220, 220, 1)";
      ctx.textAlign = "left";
      for (let i = 0; i < tip.length; i += 1) {
        ctx.fillText(tip[i] || "", tipX + tipPad, tipY + tipPad + i * tipLineH);
      }
    }
    ctx.textAlign = "left";
  }

  function drawResultsView(time: number) {
    if (docState.mode === "candles" && docState.candles.length > 0) {
      drawCandlesChart();
    } else if (runState.phase === "running") {
      drawDocumentText();
      drawComputingView();
    } else if (runState.phase === "done") {
      drawResultsDoneView(time);
      drawResultFooter();
    } else {
      drawDocumentText();
      drawResultFooter();
    }
  }

  function render(time: number) {
    drawMicroGrid(time);
    if (getActiveTab() === "forge") {
      drawPlainLogs(forgeLogs, "No Forge logs - upload a file then click Start.");
    } else {
      drawResultsView(time);
    }
    requestAnimationFrame(render);
  }

  function resize() {
    dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    width = Math.floor(rect.width);
    height = Math.floor(rect.height);
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    recomputeWrap();
  }

  function renderFiles(files: File[]) {
    fileList.innerHTML = "";
    if (!files.length) {
      fileList.innerHTML = '<li class="file-item muted">No file loaded</li>';
      return;
    }
    for (const file of files) {
      const li = document.createElement("li");
      li.className = "file-item";
      li.textContent = `${file.name} (${humanSize(file.size)})`;
      fileList.appendChild(li);
    }
  }

  async function loadFile(file: File) {
    const token = ++previewToken;
    try {
      const txt = await file.text();
      if (token !== previewToken) return;
      docState.fileName = file.name;
      docState.fileSize = file.size;
      docState.rawText = txt;
      const candles = tryParseCandleCsv(txt);
      if (candles && candles.length >= 2) {
        docState.mode = "candles";
        docState.candles = candles;
        docState.candleViewEnd = candles.length - 1;
        docState.candleViewCount = Math.min(200, candles.length);
        docState.candleHover = -1;
        docState.dnaSequence = "";
        docState.dnaRawOffsets = [];
        const firstCandle = candles[0];
        const lastCandle = candles[candles.length - 1];
        appendForge(
          firstCandle && lastCandle
            ? `candles loaded : ${candles.length} bars, ${new Date(firstCandle.time).toISOString().slice(0, 10)} -> ${new Date(lastCandle.time).toISOString().slice(0, 10)}`
            : `candles loaded : ${candles.length} bars`,
        );
      } else {
        docState.mode = "dna";
        docState.candles = [];
        const dna = extractDnaProjection(txt);
        docState.dnaSequence = dna.sequence;
        docState.dnaRawOffsets = dna.offsets;
      }
      docState.scrollLines = 0;
      recomputeWrap();
      runState.phase = "ready";
    } catch (err) {
      if (token !== previewToken) return;
      appendForge(`ERROR reading file: ${err}`);
    }
  }

  function clear() {
    docState.fileName = "";
    docState.fileSize = 0;
    docState.rawText = "";
    docState.dnaSequence = "";
    docState.dnaRawOffsets = [];
    docState.wrappedLines = [];
    docState.lineStartOffsets = [];
    docState.candles = [];
    docState.candleHover = -1;
    runState.phase = "idle";
  }

  function trackChunkProgress(line: string) {
    const match = line.match(/chunk\s+(\d+)\/(\d+)/i);
    if (match) {
      runState.chunkDone = Number.parseInt(match[1] || "", 10) || 0;
      runState.chunkTotal = Number.parseInt(match[2] || "", 10) || 0;
    }
    if (line.startsWith("final :")) runState.phase = "done";
    if (line.startsWith("ERROR")) runState.phase = "error";
  }

  function beginRun() {
    runState.phase = "running";
    runState.chunkDone = 0;
    runState.chunkTotal = 0;
    runState.report = null;
    doneTimestamp = 0;
  }

  function finishRun(report: unknown) {
    runState.report = report as RunReport;
    runState.phase = "done";
  }

  function failRun() {
    runState.phase = "error";
  }

  function bindCanvasInteractions() {
    canvas.addEventListener(
      "wheel",
      (event) => {
        if (getActiveTab() !== "results") return;
        if (docState.mode === "candles" && docState.candles.length > 0) {
          event.preventDefault();
          const factor = event.deltaY > 0 ? 1.25 : 0.8;
          const next = Math.round(docState.candleViewCount * factor);
          docState.candleViewCount = Math.max(10, Math.min(2000, Math.min(docState.candles.length, next)));
          docState.candleViewEnd = Math.max(
            docState.candleViewCount - 1,
            Math.min(docState.candleViewEnd, docState.candles.length - 1),
          );
          return;
        }
        const maxScroll = Math.max(0, docState.wrappedLines.length - visibleDocLines());
        if (maxScroll <= 0) return;
        event.preventDefault();
        const step = event.deltaY > 0 ? 3 : -3;
        docState.scrollLines = Math.max(0, Math.min(maxScroll, docState.scrollLines + step));
      },
      { passive: false },
    );
    canvas.addEventListener("mousedown", (event) => {
      if (docState.mode !== "candles" || docState.candles.length === 0) return;
      if (getActiveTab() !== "results") return;
      candleDragging = true;
      candleDragStartX = event.clientX;
      candleDragStartViewEnd = docState.candleViewEnd;
      canvas.style.cursor = "grabbing";
    });
    canvas.addEventListener("mousemove", (event) => {
      if (docState.mode !== "candles" || docState.candles.length === 0) return;
      if (getActiveTab() !== "results") return;
      const rect = canvas.getBoundingClientRect();
      const padLeft = 12;
      const padRight = 64;
      const chartW = Math.max(50, canvas.clientWidth - padLeft - padRight);
      const viewCount = docState.candleViewCount;
      if (candleDragging) {
        const candleWidth = chartW / viewCount;
        const shift = Math.round(-(event.clientX - candleDragStartX) / candleWidth);
        docState.candleViewEnd = Math.max(
          viewCount - 1,
          Math.min(docState.candles.length - 1, candleDragStartViewEnd + shift),
        );
      } else {
        const xRel = event.clientX - rect.left - padLeft;
        if (xRel < 0 || xRel > chartW) {
          docState.candleHover = -1;
          canvas.style.cursor = "default";
        } else {
          const idx = Math.floor(xRel / (chartW / viewCount));
          docState.candleHover = Math.max(0, Math.min(viewCount - 1, idx));
          canvas.style.cursor = "crosshair";
        }
      }
    });
    const endDrag = () => {
      candleDragging = false;
      docState.candleHover = -1;
      canvas.style.cursor = "default";
    };
    canvas.addEventListener("mouseup", () => {
      if (candleDragging) {
        candleDragging = false;
        canvas.style.cursor = "default";
      }
    });
    canvas.addEventListener("mouseleave", endDrag);
  }

  return {
    resize,
    renderFiles,
    loadFile,
    clear,
    trackChunkProgress,
    beginRun,
    finishRun,
    failRun,
    bindCanvasInteractions,
    startRenderLoop: () => requestAnimationFrame(render),
  };
}
