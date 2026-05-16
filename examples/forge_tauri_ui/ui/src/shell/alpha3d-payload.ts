// @ts-nocheck
import { attachAlpha3dMetadata } from "./alpha3d-geometry.js";

export function computeAlpha3dZSeries(candles, metric) {
  const n = candles.length;
  const out = new Float32Array(n);
  switch (metric) {
    case "volatility": {
      for (let i = 0; i < n; i++) {
        const c = candles[i];
        out[i] = c.close > 0 ? (c.high - c.low) / c.close : 0;
      }
      break;
    }
    case "hour": {
      for (let i = 0; i < n; i++) {
        const t = candles[i].time || 0;
        out[i] = ((t / 3600000) % 24);
      }
      break;
    }
    case "rsi": {
      const period = 14;
      let avgGain = 0, avgLoss = 0;
      for (let i = 1; i <= period && i < n; i++) {
        const d = candles[i].close - candles[i-1].close;
        if (d > 0) avgGain += d; else avgLoss -= d;
      }
      avgGain /= period; avgLoss /= period;
      for (let i = 0; i < n; i++) {
        if (i <= period) { out[i] = 50; continue; }
        const d = candles[i].close - candles[i-1].close;
        const g = d > 0 ? d : 0;
        const l = d < 0 ? -d : 0;
        avgGain = (avgGain * (period - 1) + g) / period;
        avgLoss = (avgLoss * (period - 1) + l) / period;
        const rs = avgLoss > 0 ? avgGain / avgLoss : 100;
        out[i] = 100 - 100 / (1 + rs);
      }
      break;
    }
    case "cvd": {
      let cum = 0;
      for (let i = 0; i < n; i++) {
        const c = candles[i];
        const sign = c.close > c.open ? 1 : (c.close < c.open ? -1 : 0);
        cum += (c.volume || 0) * sign;
        out[i] = cum;
      }
      break;
    }
    case "volume":
    default: {
      for (let i = 0; i < n; i++) out[i] = candles[i].volume || 0;
      break;
    }
  }
  return out;
}

export function alpha3dCssColorTriplet(color, fallback = [0.72, 0.74, 0.70], gain = 1) {
  const match = String(color || "").match(/rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/i);
  const base = match
    ? [Number(match[1]) / 255, Number(match[2]) / 255, Number(match[3]) / 255]
    : fallback.slice(0, 3);
  return base.map((value) => Math.max(0, Math.min(1, value * gain)));
}

function alpha3dColorWithAlpha(color, alpha = 0.18, fallback = "rgba(158,180,214,0.18)") {
  const match = String(color || "").match(/rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)/i);
  if (!match) return fallback;
  return `rgba(${Math.round(Number(match[1]))}, ${Math.round(Number(match[2]))}, ${Math.round(Number(match[3]))}, ${Math.max(0, Math.min(1, alpha))})`;
}

function alpha3dPushFaceTriangles(store, corners, color) {
  const [a, b, c, d] = corners;
  store.positions.push(
    a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z,
    a.x, a.y, a.z, c.x, c.y, c.z, d.x, d.y, d.z,
  );
  for (let i = 0; i < 6; i += 1) {
    store.colors.push(color[0], color[1], color[2]);
  }
}

function alpha3dPushLineEdges(store, corners, color) {
  const edges = [
    [corners[0], corners[1]], [corners[1], corners[2]],
    [corners[2], corners[3]], [corners[3], corners[0]],
  ];
  for (const [a, b] of edges) {
    store.positions.push(a.x, a.y, a.z, b.x, b.y, b.z);
    store.colors.push(color[0], color[1], color[2], color[0], color[1], color[2]);
  }
}

export function computeAlpha3dPressurePayload(model) {
  const points = Array.isArray(model?.points) ? model.points : [];
  if (!points.length) {
    return {
      positions: new Float32Array(0),
      colors: new Float32Array(0),
      sizes: new Float32Array(0),
      linePositions: new Float32Array(0),
      lineColors: new Float32Array(0),
      lineSizes: new Float32Array(0),
      metadata: [],
      guide: null,
      drawMode: "triangles",
      lineDrawMode: "linepairs",
      pointSize: 1,
    };
  }

  const count = points.length;
  const maxTickVolume = Math.max(1, ...points.map((point) => Number(point?.tickVolume || 0)));
  const maxThreshold = Math.max(1, ...points.map((point) => Number(point?.volumeThreshold || 0)));
  const maxVolumeScale = Math.max(maxTickVolume, maxThreshold);
  const halfW = 0.86;
  const halfD = 0.86;
  const floorZ = 0;
  const ceilingZ = 1.1;
  const stepX = (halfW * 2) / Math.max(1, count);
  const towerWidth = Math.max(0.012, stepX * 0.54);
  const towerDepth = 0.055;
  const rowsGuide = 4;
  const columnsGuide = Math.max(3, Math.min(6, Math.round(count / 14)));
  const veilHeight = floorZ + (maxThreshold / maxVolumeScale) * ceilingZ;
  const veilColor = [0.73, 0.78, 0.87];
  const gridColor = [0.31, 0.32, 0.31];
  const faceStore = { positions: [], colors: [] };
  const lineStore = { positions: [], colors: [] };
  const metadata = [];
  const slotTimes = [];

  const xOf = (index) => -halfW + index * stepX + stepX * 0.5;
  const yOf = (pressure) => -halfD + ((clampUnitInterval(pressure) + 1) * 0.5) * (halfD * 2);
  const zOf = (volume) => floorZ + (Math.max(0, Number(volume) || 0) / maxVolumeScale) * ceilingZ;

  for (let i = 0; i <= count; i += 1) {
    const x = -halfW + i * stepX;
    lineStore.positions.push(x, -halfD, floorZ, x, halfD, floorZ);
    lineStore.colors.push(gridColor[0], gridColor[1], gridColor[2], gridColor[0], gridColor[1], gridColor[2]);
  }
  for (let i = 0; i <= rowsGuide; i += 1) {
    const y = -halfD + (i / rowsGuide) * (halfD * 2);
    lineStore.positions.push(-halfW, y, floorZ, halfW, y, floorZ);
    lineStore.colors.push(gridColor[0], gridColor[1], gridColor[2], gridColor[0], gridColor[1], gridColor[2]);
  }

  for (let i = 0; i < count; i += 1) {
    const point = points[i] || {};
    const pressure = Number(point.pressureScore || 0);
    const tickVolume = Number(point.tickVolume || 0);
    const x = xOf(i);
    const y = yOf(pressure);
    const z = zOf(tickVolume);
    const x0 = x - towerWidth * 0.5;
    const x1 = x + towerWidth * 0.5;
    const y0 = y - towerDepth * 0.5;
    const y1 = y + towerDepth * 0.5;
    const base = [
      { x: x0, y: y0, z: floorZ },
      { x: x1, y: y0, z: floorZ },
      { x: x1, y: y1, z: floorZ },
      { x: x0, y: y1, z: floorZ },
    ];
    const top = [
      { x: x0, y: y0, z },
      { x: x1, y: y0, z },
      { x: x1, y: y1, z },
      { x: x0, y: y1, z },
    ];
    const faceColor = pressure >= 0 ? [0.29, 0.77, 0.43] : [0.88, 0.34, 0.31];
    const edgeColor = pressure >= 0 ? [0.76, 0.98, 0.82] : [0.99, 0.78, 0.74];
    alpha3dPushFaceTriangles(faceStore, top, faceColor);
    alpha3dPushFaceTriangles(faceStore, [base[0], base[1], top[1], top[0]], faceColor);
    alpha3dPushFaceTriangles(faceStore, [base[1], base[2], top[2], top[1]], faceColor);
    alpha3dPushFaceTriangles(faceStore, [base[2], base[3], top[3], top[2]], faceColor);
    alpha3dPushFaceTriangles(faceStore, [base[3], base[0], top[0], top[3]], faceColor);
    alpha3dPushLineEdges(lineStore, top, edgeColor);
    alpha3dPushLineEdges(lineStore, base, edgeColor);
    for (let edge = 0; edge < 4; edge += 1) {
      const a = base[edge];
      const b = top[edge];
      lineStore.positions.push(a.x, a.y, a.z, b.x, b.y, b.z);
      lineStore.colors.push(edgeColor[0], edgeColor[1], edgeColor[2], edgeColor[0], edgeColor[1], edgeColor[2]);
    }
    slotTimes.push(point.time || "");
    metadata.push({
      role: "pressure-tower",
      time: point.time || "",
      value: tickVolume,
      signal: point.signal || "NO_SIGNAL",
      pressureScore: pressure,
    });
  }

  const veilBase = [
    { x: -halfW, y: -halfD, z: veilHeight },
    { x: halfW, y: -halfD, z: veilHeight },
    { x: halfW, y: halfD, z: veilHeight },
    { x: -halfW, y: halfD, z: veilHeight },
  ];
  alpha3dPushFaceTriangles(faceStore, veilBase, veilColor);
  alpha3dPushLineEdges(lineStore, veilBase, [0.96, 0.97, 0.99]);

  return {
    positions: new Float32Array(faceStore.positions),
    colors: new Float32Array(faceStore.colors),
    sizes: new Float32Array(faceStore.positions.length / 3).fill(1),
    linePositions: new Float32Array(lineStore.positions),
    lineColors: new Float32Array(lineStore.colors),
    lineSizes: new Float32Array(lineStore.positions.length / 3).fill(1),
    metadata,
    guide: {
      origin: { x: -halfW, y: -halfD, z: floorZ },
      timeEnd: { x: halfW, y: -halfD, z: floorZ },
      priceGroundEnd: { x: -halfW, y: halfD, z: floorZ },
      volumeEnd: { x: -halfW, y: -halfD, z: ceilingZ },
      floorZ,
      ceilingZ,
      priceLo: -1,
      priceHi: 1,
      volumeLo: 0,
      volumeHi: maxVolumeScale,
      cols: count,
      rows: rowsGuide + 1,
      count,
      slotLayout: [],
      slotTimes,
      veils: [
        {
          kind: "plane",
          fill: "rgba(196, 208, 232, 0.34)",
          stroke: "rgba(236, 240, 248, 0.92)",
          corners: veilBase,
        },
      ],
    },
    drawMode: "triangles",
    lineDrawMode: "linepairs",
    pointSize: 1.0,
  };
}

function clampUnitInterval(value) {
  const numeric = Number(value) || 0;
  return Math.max(-1, Math.min(1, numeric));
}

export function computeAlpha3dCandles(candles, options = {}) {
  const n = candles.length;
  if (n < 1) return { positions: new Float32Array(0), colors: new Float32Array(0), sizes: new Float32Array(0), drawMode: "points", pointSize: 4.0 };

  const maxBars = 180;
  const stride = Math.max(1, Math.ceil(n / maxBars));
  const sampled = [];
  for (let i = 0; i < n; i += stride) sampled.push({ candle: candles[i], index: i });
  if (!sampled.length) return { positions: new Float32Array(0), colors: new Float32Array(0), sizes: new Float32Array(0), drawMode: "points", pointSize: 4.0 };

  let priceLo = Infinity;
  let priceHi = -Infinity;
  for (const entry of sampled) {
    const c = entry.candle || {};
    const low = Number(c.low);
    const high = Number(c.high);
    if (Number.isFinite(low)) priceLo = Math.min(priceLo, low);
    if (Number.isFinite(high)) priceHi = Math.max(priceHi, high);
  }
  if (!Number.isFinite(priceLo) || !Number.isFinite(priceHi) || priceHi <= priceLo) {
    priceLo = 0;
    priceHi = 1;
  }
  const priceRange = Math.max(1e-6, priceHi - priceLo);
  const count = sampled.length;
  const priceBins = Math.max(20, Math.min(34, Math.round(20 + count / 16)));
  const binSize = priceRange / priceBins;
  const floorZ = 0;
  const ceilingZ = 0.94;
  const gridWidth = 1.72;
  const gridDepth = 1.72;
  const stepX = gridWidth / Math.max(1, count);
  const stepY = gridDepth / Math.max(1, priceBins);
  const halfW = gridWidth * 0.5;
  const halfD = gridDepth * 0.5;
  const towerHalfX = Math.max(0.0035, stepX * 0.34);
  const towerHalfY = Math.max(0.0035, stepY * 0.34);
  const gridColor = [0.34, 0.35, 0.33];
  const positions = [];
  const colors = [];
  const sizes = [];
  const metadata = [];
  const activeIndicators3d = Array.isArray(options.activeIndicators) ? options.activeIndicators : [];
  const cloudIndicators3d = activeIndicators3d.filter((indicator) => {
    if (indicator?.visible === false) return false;
    const plots = typeof options.overlaySeries === "function" ? options.overlaySeries(candles, indicator) : [];
    return plots.some((plot) => plot?.kind === "cloud");
  });
  const towerGrid = Array.from({ length: count }, () => new Float32Array(priceBins));
  const barDirections = new Array(count).fill(true);
  const binCenter = (bin) => priceLo + (bin + 0.5) * binSize;
  const clampBin = (value) => Math.max(0, Math.min(priceBins - 1, Math.floor((value - priceLo) / binSize)));
  let maxCellVolume = 0;
  for (let t = 0; t < count; t += 1) {
    const candle = sampled[t]?.candle || {};
    const open = Number(candle.open);
    const close = Number(candle.close);
    const high = Number(candle.high);
    const low = Number(candle.low);
    const volume = Math.max(0, Number(candle.volume) || 0);
    barDirections[t] = close >= open;
    if (!Number.isFinite(low) || !Number.isFinite(high) || volume <= 0) continue;
    const startBin = clampBin(Math.min(low, high));
    const endBin = clampBin(Math.max(low, high));
    const typical = Number.isFinite(open) && Number.isFinite(close)
      ? (open + high + low + close) / 4
      : ((high + low) * 0.5);
    const bodyLo = Number.isFinite(open) && Number.isFinite(close) ? Math.min(open, close) : typical;
    const bodyHi = Number.isFinite(open) && Number.isFinite(close) ? Math.max(open, close) : typical;
    const span = Math.max(binSize, high - low);
    let weightSum = 0;
    const weights = [];
    for (let bin = startBin; bin <= endBin; bin += 1) {
      const center = binCenter(bin);
      const proximity = 1 - Math.min(1, Math.abs(center - typical) / Math.max(binSize, span * 0.5));
      const bodyBonus = center >= (bodyLo - binSize * 0.35) && center <= (bodyHi + binSize * 0.35) ? 0.9 : 0;
      const closeBonus = 0.35 * (1 - Math.min(1, Math.abs(center - close) / Math.max(binSize, span * 0.3)));
      const weight = 0.35 + proximity * 0.75 + Math.max(0, bodyBonus) + Math.max(0, closeBonus);
      weights.push({ bin, weight });
      weightSum += weight;
    }
    if (weightSum <= 0) continue;
    for (const entry of weights) {
      const allocated = volume * (entry.weight / weightSum);
      towerGrid[t][entry.bin] += allocated;
      if (towerGrid[t][entry.bin] > maxCellVolume) maxCellVolume = towerGrid[t][entry.bin];
    }
  }
  maxCellVolume = Math.max(1, maxCellVolume);
  const zOfVolume = (value) => floorZ + (Math.max(0, Number(value) || 0) / maxCellVolume) * ceilingZ;
  const xOfBar = (barIndex) => -halfW + barIndex * stepX + stepX * 0.5;
  const yOfPriceBin = (priceBin) => -halfD + priceBin * stepY + stepY * 0.5;
  const pushVertex = (x, y, z, color, meta) => {
    positions.push(x, y, z);
    colors.push(color[0], color[1], color[2]);
    sizes.push(1.0);
    metadata.push(meta);
  };
  const pushLine = (a, b, color, meta) => {
    pushVertex(a.x, a.y, a.z, color, meta);
    pushVertex(b.x, b.y, b.z, color, meta);
  };
  const slotLayout = [];
  const columnTimes = sampled.map((entry) => entry?.candle?.time || "");
  for (let row = 0; row < priceBins; row += 1) {
    for (let col = 0; col < count; col += 1) {
      slotLayout.push({
        slot: slotLayout.length,
        index: row * count + col,
        time: columnTimes[col] || "",
        x: xOfBar(col),
        y: yOfPriceBin(row),
        col,
        row,
      });
    }
  }
  const rollingWindow = Math.max(10, Math.min(28, Math.floor(count * 0.16)));
  const avgGrid = Array.from({ length: count }, () => new Float32Array(priceBins));
  const valueAreaLower = new Int16Array(count);
  const valueAreaUpper = new Int16Array(count);
  for (let t = 0; t < count; t += 1) {
    const start = Math.max(0, t - rollingWindow + 1);
    const barsInWindow = t - start + 1;
    const aggregate = new Float32Array(priceBins);
    let totalAgg = 0;
    let pocBin = 0;
    let pocValue = -1;
    for (let w = start; w <= t; w += 1) {
      for (let bin = 0; bin < priceBins; bin += 1) {
        const value = towerGrid[w][bin];
        aggregate[bin] += value;
      }
    }
    for (let bin = 0; bin < priceBins; bin += 1) {
      totalAgg += aggregate[bin];
      avgGrid[t][bin] = aggregate[bin] / barsInWindow;
      if (aggregate[bin] > pocValue) {
        pocValue = aggregate[bin];
        pocBin = bin;
      }
    }
    if (totalAgg <= 0) {
      valueAreaLower[t] = 0;
      valueAreaUpper[t] = priceBins - 1;
      continue;
    }
    const accepted = new Set([pocBin]);
    let cum = Math.max(0, aggregate[pocBin]);
    let left = pocBin - 1;
    let right = pocBin + 1;
    const target = totalAgg * 0.7;
    while (cum < target && (left >= 0 || right < priceBins)) {
      const leftValue = left >= 0 ? aggregate[left] : -1;
      const rightValue = right < priceBins ? aggregate[right] : -1;
      if (rightValue > leftValue) {
        if (right < priceBins) {
          accepted.add(right);
          cum += Math.max(0, aggregate[right]);
          right += 1;
        } else if (left >= 0) {
          accepted.add(left);
          cum += Math.max(0, aggregate[left]);
          left -= 1;
        }
      } else {
        if (left >= 0) {
          accepted.add(left);
          cum += Math.max(0, aggregate[left]);
          left -= 1;
        } else if (right < priceBins) {
          accepted.add(right);
          cum += Math.max(0, aggregate[right]);
          right += 1;
        }
      }
    }
    valueAreaLower[t] = Math.max(0, Math.min(...accepted));
    valueAreaUpper[t] = Math.min(priceBins - 1, Math.max(...accepted));
  }
  const canopyGrid = Array.from({ length: count }, () => new Float32Array(priceBins));
  for (let t = 0; t < count; t += 1) {
    for (let bin = 0; bin < priceBins; bin += 1) {
      const insideValueArea = bin >= valueAreaLower[t] && bin <= valueAreaUpper[t];
      canopyGrid[t][bin] = avgGrid[t][bin] * (insideValueArea ? 0.92 : 0.22);
    }
  }
  for (let pass = 0; pass < 2; pass += 1) {
    const nextGrid = Array.from({ length: count }, () => new Float32Array(priceBins));
    for (let t = 0; t < count; t += 1) {
      for (let bin = 0; bin < priceBins; bin += 1) {
        let sum = canopyGrid[t][bin] * 2.2;
        let weight = 2.2;
        for (let dt = -1; dt <= 1; dt += 1) {
          for (let db = -1; db <= 1; db += 1) {
            if (!dt && !db) continue;
            const tt = t + dt;
            const bb = bin + db;
            if (tt < 0 || tt >= count || bb < 0 || bb >= priceBins) continue;
            const w = dt === 0 || db === 0 ? 0.65 : 0.35;
            sum += canopyGrid[tt][bb] * w;
            weight += w;
          }
        }
        nextGrid[t][bin] = sum / weight;
      }
    }
    for (let t = 0; t < count; t += 1) canopyGrid[t] = nextGrid[t];
  }
  const veilDescriptors = [];
  const signalCells = new Set();
  if (cloudIndicators3d.length) {
    for (const indicator of cloudIndicators3d) {
      const plots = typeof options.overlaySeries === "function" ? options.overlaySeries(candles, indicator) : [];
      const cloudPlot = plots.find((plot) => plot?.kind === "cloud");
      if (!cloudPlot) continue;
      const upperSeries = new Array(slotLayout.length).fill(NaN);
      const lowerSeries = new Array(slotLayout.length).fill(NaN);
      for (let row = 0; row < priceBins; row += 1) {
        for (let col = 0; col < count; col += 1) {
          const slotIndex = row * count + col;
          const baseVolume = canopyGrid[col][row];
          const upperVolume = baseVolume * 1.08 + maxCellVolume * 0.018;
          const lowerVolume = Math.max(0, baseVolume * 0.88);
          upperSeries[slotIndex] = upperVolume;
          lowerSeries[slotIndex] = lowerVolume;
          const towerVolume = towerGrid[col][row];
          const outsideValueArea = row < valueAreaLower[col] || row > valueAreaUpper[col];
          if (outsideValueArea && towerVolume > upperVolume * 1.02 && towerVolume > maxCellVolume * 0.045) {
            signalCells.add(slotIndex);
          }
        }
      }
      veilDescriptors.push({
        id: indicator?.id || "",
        command: indicator?.command || "",
        label: cloudPlot.label || indicator?.label || indicator?.id || "",
        fill: alpha3dColorWithAlpha(cloudPlot.color, 0.22, "rgba(146,170,208,0.22)"),
        stroke: alpha3dColorWithAlpha(cloudPlot.color, 0.68, "rgba(146,170,208,0.68)"),
        upperSeries,
        lowerSeries,
        zMetric: "volume",
        signalMeaning: "value-acceptance canopy",
      });
    }
  }
  for (let col = 0; col <= count; col += 1) {
    const x = -halfW + col * stepX;
    pushLine({ x, y: -halfD, z: floorZ }, { x, y: halfD, z: floorZ }, gridColor, { role: "grid x", cell: `${col},*` });
  }
  for (let row = 0; row <= priceBins; row += 1) {
    const y = -halfD + row * stepY;
    pushLine({ x: -halfW, y, z: floorZ }, { x: halfW, y, z: floorZ }, gridColor, { role: "grid y", cell: `*,${row}` });
  }

  for (let col = 0; col < count; col += 1) {
    const bullish = barDirections[col];
    for (let row = 0; row < priceBins; row += 1) {
      const towerVolume = towerGrid[col][row];
      if (!(towerVolume > maxCellVolume * 0.01)) continue;
      const signalSlot = row * count + col;
      const lineColor = signalCells.has(signalSlot)
        ? [0.95, 0.75, 0.36]
        : (bullish ? [0.33, 0.76, 0.47] : [0.90, 0.39, 0.36]);
      const capColor = signalCells.has(signalSlot)
        ? [1.0, 0.88, 0.62]
        : (bullish ? [0.86, 0.98, 0.90] : [1.0, 0.84, 0.82]);
      const x = xOfBar(col);
      const y = yOfPriceBin(row);
      const x0 = x - towerHalfX;
      const x1 = x + towerHalfX;
      const y0 = y - towerHalfY;
      const y1 = y + towerHalfY;
      const zTop = zOfVolume(towerVolume);
      const barIndex = sampled[col]?.index ?? col;
      const candle = sampled[col]?.candle || {};
      const priceValue = binCenter(row);
      const meta = {
        role: signalCells.has(signalSlot) ? "signal tower" : "volume tower",
        bar_index: barIndex,
        point_index: signalSlot,
        cell: `${col},${row}`,
        time: candle.time || null,
        price_level: priceValue,
        value: towerVolume,
      };
      const bottomRing = [
        { x: x0, y: y0, z: floorZ },
        { x: x1, y: y0, z: floorZ },
        { x: x1, y: y1, z: floorZ },
        { x: x0, y: y1, z: floorZ },
      ];
      const topRing = [
        { x: x0, y: y0, z: zTop },
        { x: x1, y: y0, z: zTop },
        { x: x1, y: y1, z: zTop },
        { x: x0, y: y1, z: zTop },
      ];
      for (let i = 0; i < 4; i += 1) {
        pushLine(bottomRing[i], topRing[i], lineColor, meta);
        pushLine(topRing[i], topRing[(i + 1) % 4], capColor, { ...meta, role: "tower cap" });
      }
    }
  }

  return {
    positions: new Float32Array(positions),
    colors: new Float32Array(colors),
    sizes: new Float32Array(sizes),
    metadata,
    guide: {
      origin: { x: -halfW, y: -halfD, z: floorZ },
      timeEnd: { x: halfW, y: -halfD, z: floorZ },
      priceGroundEnd: { x: -halfW, y: halfD, z: floorZ },
      volumeEnd: { x: -halfW, y: -halfD, z: ceilingZ },
      floorZ,
      ceilingZ,
      priceLo,
      priceHi,
      volumeLo: 0,
      volumeHi: maxCellVolume,
      cols: count,
      rows: priceBins,
      count,
      slotLayout,
      slotTimes: columnTimes,
      veils: veilDescriptors,
    },
    drawMode: "linepairs",
    pointSize: 1.0,
  };
}

export function computeAlpha3dPayloadForMode(mode, candles, options = {}) {
  if (options.isPressureActive && options.pressureData?.points?.length) {
    return computeAlpha3dPressurePayload(options.pressureData);
  }
  const payload = computeAlpha3dCandles(candles, options);
  return attachAlpha3dMetadata("candles3d", candles, payload);
}


