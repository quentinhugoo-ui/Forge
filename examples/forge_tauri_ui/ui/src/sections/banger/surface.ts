// @ts-nocheck
import "./controller.js";

// Banger — minimal Blender-style 3D viewport (WebGL2)
// Self-contained; wires the BOOM titlebar button and the overlay shell.

(function () {
  "use strict";

  const $ = (id) => document.getElementById(id);

  const els = {
    boomBtn: $("bangerBoomBtn"),
    view:    $("bangerView"),
    canvas:  $("bangerCanvas"),
    statVerts: $("bangerStatVerts"),
    statFaces: $("bangerStatFaces"),
    statFps: $("bangerStatFps"),
    gizmo:   $("bangerGizmo"),
    exitBtn: $("bangerExitBtn"),
    stage:   null,
    content: document.querySelector("#alphaSection .content"),
    leftPanel: document.querySelector("#alphaSection .left-panel"),
  };

  if (!els.boomBtn || !els.view || !els.canvas) {
    console.warn("[banger] required elements missing — aborting wire-up");
    return;
  }
  els.stage = els.view.closest(".canvas-stage");
  let bangerController = null;

  // ---------- shaders ----------
  const {
    M4,
    AXIS_RGB,
    AXIS_HEX,
    makeCube,
    makeGrid,
    VS_MESH,
    FS_MESH,
    VS_LINE,
    FS_LINE,
    VS_SDF,
    FS_SDF,
  } = window.ForgeBangerCatalog || {};
  function compile(gl, type, src) {
    const sh = gl.createShader(type);
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      console.error("[banger] shader compile error:", gl.getShaderInfoLog(sh));
      return null;
    }
    return sh;
  }
  function link(gl, vs, fs) {
    const p = gl.createProgram();
    gl.attachShader(p, vs); gl.attachShader(p, fs);
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
      console.error("[banger] program link error:", gl.getProgramInfoLog(p));
      return null;
    }
    return p;
  }

  function isBoom3dFileName(name) {
    return /\.(obj|stl|ply|off|gltf|glb)$/i.test(String(name || ""));
  }

  function isBoomAnimationFileName(name) {
    return /\.(boom\.json|boom\.js|anim\.json|anim\.js|js|json)$/i.test(String(name || ""));
  }

  function isBoom3dCandidateName(name) {
    return /\.(obj|stl|ply|off|gltf|glb|fbx|dae|3ds|3mf|usd|usda|usdc|usdz|abc|x3d|wrl)$/i.test(String(name || ""));
  }

  function isBoomSceneBridgeFileName(name) {
    return isBoom3dCandidateName(name) || isBoomAnimationFileName(name);
  }

  function computeFaceNormal(ax, ay, az, bx, by, bz, cx, cy, cz) {
    const abx = bx - ax, aby = by - ay, abz = bz - az;
    const acx = cx - ax, acy = cy - ay, acz = cz - az;
    let nx = aby * acz - abz * acy;
    let ny = abz * acx - abx * acz;
    let nz = abx * acy - aby * acx;
    const len = Math.hypot(nx, ny, nz) || 1;
    nx /= len; ny /= len; nz /= len;
    return [nx, ny, nz];
  }

  function createBoomBoundsTracker() {
    return {
      min: [Infinity, Infinity, Infinity],
      max: [-Infinity, -Infinity, -Infinity],
      count: 0,
    };
  }

  function trackBoomBoundsPoint(bounds, point) {
    if (!bounds || !point) return;
    for (let axis = 0; axis < 3; axis += 1) {
      const value = Number(point[axis] || 0);
      if (value < bounds.min[axis]) bounds.min[axis] = value;
      if (value > bounds.max[axis]) bounds.max[axis] = value;
    }
    bounds.count += 1;
  }

  function boomBoundsHintFromTracker(bounds) {
    if (!bounds?.count) return null;
    const min = bounds.min.map((value) => Number.isFinite(value) ? value : 0);
    const max = bounds.max.map((value) => Number.isFinite(value) ? value : 0);
    const center = [
      (min[0] + max[0]) * 0.5,
      (min[1] + max[1]) * 0.5,
      (min[2] + max[2]) * 0.5,
    ];
    const span = [
      max[0] - min[0],
      max[1] - min[1],
      max[2] - min[2],
    ];
    const maxSpan = Math.max(span[0], span[1], span[2]) || 1;
    return { min, max, center, span, scale: 6 / maxSpan, count: bounds.count };
  }

  function validBoomBoundsHint(bounds) {
    return Array.isArray(bounds?.min)
      && Array.isArray(bounds?.max)
      && bounds.min.length >= 3
      && bounds.max.length >= 3;
  }

  function pushTriangle(pos, nrm, a, b, c, normal = null, bounds = null) {
    const faceNormal = normal || computeFaceNormal(a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]);
    pos.push(...a, ...b, ...c);
    nrm.push(...faceNormal, ...faceNormal, ...faceNormal);
    trackBoomBoundsPoint(bounds, a);
    trackBoomBoundsPoint(bounds, b);
    trackBoomBoundsPoint(bounds, c);
  }

  function boomFloat32Array(values) {
    return values instanceof Float32Array ? values : new Float32Array(values || []);
  }

  function boomHashArrayBuffer(buffer, label = "bytes") {
    const bytes = new Uint8Array(buffer || new ArrayBuffer(0));
    let hash = boomHashText(2166136261, label);
    hash = boomHashInt(hash, bytes.byteLength);
    for (let i = 0; i < bytes.length; i += 1) {
      hash ^= bytes[i];
      hash = Math.imul(hash, 16777619);
    }
    return `kasm-${(hash >>> 0).toString(16).padStart(8, "0")}`;
  }

  function boomTextSourcePart(text, label) {
    const source = String(text || "");
    return {
      kind: "text",
      label,
      bytes: source.length,
      hash: kasmHashString(`import-text-v1|${label}|${source}`),
    };
  }

  function boomBufferSourcePart(buffer, label) {
    const byteLength = buffer?.byteLength || 0;
    return {
      kind: "bytes",
      label,
      bytes: byteLength,
      hash: boomHashArrayBuffer(buffer, `import-bytes-v1|${label}`),
    };
  }

  function boomImportSourceMeta(parser, sourceName, parts = []) {
    return {
      parser,
      sourceName: String(sourceName || ""),
      sourceHashHint: kasmHashString(`import-source-hint-v1|${parser}|${sourceName || ""}|${stableBoomStringify(parts)}`),
      sourceParts: parts,
    };
  }

  function boomCachedFloatArrayHash(values, label) {
    const started = boomNowMs();
    const length = values?.length || 0;
    const cacheKey = `${label}:${length}`;
    const cached = values?.__boomKasmHash;
    if (cached?.key === cacheKey) {
      return { hash: cached.hash, status: "HIT", elapsedMs: boomNowMs() - started };
    }
    const hash = boomHashFloatArray(values, label);
    try {
      Object.defineProperty(values, "__boomKasmHash", {
        value: { key: cacheKey, hash },
        configurable: true,
        enumerable: false,
      });
    } catch (_) {
      if (values) values.__boomKasmHash = { key: cacheKey, hash };
    }
    return { hash, status: "MISS", elapsedMs: boomNowMs() - started };
  }

  function buildBoomImportNormalizeView(sourcePos, sourceNrm, importSource = {}) {
    const fingerprintStarted = boomNowMs();
    const carriedHash = importSource?.sourceHashHint || "";
    const posHash = carriedHash ? { hash: carriedHash, status: "HIT" } : boomCachedFloatArrayHash(sourcePos, "import-pos");
    const nrmHash = carriedHash ? { hash: carriedHash, status: "HIT" } : boomCachedFloatArrayHash(sourceNrm, "import-nrm");
    const sourceHash = carriedHash || kasmHashString(`import-source|${sourcePos.length}|${sourceNrm.length}|${posHash.hash}|${nrmHash.hash}`);
    emitBoomAudit(
      "import_source_fingerprint",
      posHash.status === "HIT" && nrmHash.status === "HIT" ? "HIT" : "MISS",
      sourceHash,
      boomNowMs() - fingerprintStarted,
      sourcePos.length / 3,
      "vertices",
      {
        posHash: posHash.hash,
        normalHash: nrmHash.hash,
        fingerprintMode: carriedHash ? "importer-carried" : "float-buffer-scan",
        boundsMode: validBoomBoundsHint(importSource?.boundsHint) ? "importer-carried" : "position-rescan",
        parser: importSource?.parser || "",
      }
    );
    return boomCachedCompute(
      "import_normalize_view",
      {
        sourceHash,
        sourceFloats: sourcePos.length,
        normalFloats: sourceNrm.length,
        scaleTo: 6,
        parser: importSource?.parser || "",
      },
      sourcePos.length / 3,
      "vertices",
      () => {
        const carriedBounds = validBoomBoundsHint(importSource?.boundsHint) ? importSource.boundsHint : null;
        let minX, minY, minZ, maxX, maxY, maxZ, cx, cy, cz, spanX, spanY, spanZ, scale;
        if (carriedBounds) {
          [minX, minY, minZ] = carriedBounds.min;
          [maxX, maxY, maxZ] = carriedBounds.max;
          [cx, cy, cz] = carriedBounds.center || [
            (minX + maxX) * 0.5,
            (minY + maxY) * 0.5,
            (minZ + maxZ) * 0.5,
          ];
          [spanX, spanY, spanZ] = carriedBounds.span || [maxX - minX, maxY - minY, maxZ - minZ];
          scale = carriedBounds.scale || (6 / (Math.max(spanX, spanY, spanZ) || 1));
        } else {
          minX = Infinity; minY = Infinity; minZ = Infinity;
          maxX = -Infinity; maxY = -Infinity; maxZ = -Infinity;
          for (let i = 0; i < sourcePos.length; i += 3) {
            const x = sourcePos[i], y = sourcePos[i + 1], z = sourcePos[i + 2];
            if (x < minX) minX = x; if (x > maxX) maxX = x;
            if (y < minY) minY = y; if (y > maxY) maxY = y;
            if (z < minZ) minZ = z; if (z > maxZ) maxZ = z;
          }
          cx = (minX + maxX) * 0.5;
          cy = (minY + maxY) * 0.5;
          cz = (minZ + maxZ) * 0.5;
          spanX = maxX - minX;
          spanY = maxY - minY;
          spanZ = maxZ - minZ;
          scale = 6 / (Math.max(spanX, spanY, spanZ) || 1);
        }
        return {
          sourceHash,
          normalHash: nrmHash.hash,
          parser: importSource?.parser || "",
          sourceName: importSource?.sourceName || "",
          count: sourcePos.length / 3,
          faceCount: sourcePos.length / 9,
          center: [cx, cy, cz],
          span: [spanX, spanY, spanZ],
          min: [minX, minY, minZ],
          max: [maxX, maxY, maxZ],
          scale,
          boundsMode: carriedBounds ? "importer-carried" : "position-rescan",
          sourceBytes: sourcePos.byteLength + sourceNrm.byteLength,
        };
      }
    ).value;
  }

  function materializeBoomNormalizedPositions(sourcePos, view) {
    return boomCachedCompute(
      "import_normalized_positions",
      {
        sourceHash: view.sourceHash,
        center: view.center,
        scale: Number(view.scale || 1).toFixed(8),
        sourceFloats: sourcePos.length,
      },
      sourcePos.length / 3,
      "vertices",
      () => {
        const normalized = new Float32Array(sourcePos.length);
        const [cx, cy, cz] = view.center;
        const scale = view.scale || 1;
        for (let i = 0; i < sourcePos.length; i += 3) {
          normalized[i] = (sourcePos[i] - cx) * scale;
          normalized[i + 1] = (sourcePos[i + 1] - cy) * scale;
          normalized[i + 2] = (sourcePos[i + 2] - cz) * scale;
        }
        return normalized;
      }
    ).value;
  }

  function normalizeMeshData(posArray, nrmArray, importSource = {}) {
    if (!posArray.length) return null;
    const sourcePos = boomFloat32Array(posArray);
    const sourceNrm = boomFloat32Array(nrmArray);
    const view = buildBoomImportNormalizeView(sourcePos, sourceNrm, importSource);
    const normalized = materializeBoomNormalizedPositions(sourcePos, view);
    return {
      pos: normalized,
      nrm: sourceNrm,
      source: {
        pos: sourcePos,
        nrm: sourceNrm,
        sourceHash: view.sourceHash,
        normalHash: view.normalHash,
        parser: view.parser || importSource?.parser || "",
        sourceName: view.sourceName || importSource?.sourceName || "",
        sourceParts: importSource?.sourceParts || [],
      },
      normalizeView: view,
      count: view.count,
      faceCount: view.faceCount,
      bounds: {
        center: view.center,
        span: view.span,
        scale: view.scale,
      },
    };
  }

  function parseObjMesh(text, importSource = {}) {
    const lines = String(text || "").split(/\r?\n/);
    const verts = [];
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    for (const lineRaw of lines) {
      const line = lineRaw.trim();
      if (!line || line.startsWith("#")) continue;
      if (line.startsWith("v ")) {
        const parts = line.split(/\s+/);
        if (parts.length >= 4) {
          verts.push([Number(parts[1]), Number(parts[2]), Number(parts[3])]);
        }
        continue;
      }
      if (line.startsWith("f ")) {
        const refs = line.split(/\s+/).slice(1)
          .map((token) => token.split("/")[0])
          .map((token) => Number(token))
          .filter((index) => Number.isFinite(index) && index !== 0)
          .map((index) => (index > 0 ? index - 1 : verts.length + index))
          .filter((index) => index >= 0 && index < verts.length);
        if (refs.length < 3) continue;
        const a = verts[refs[0]];
        for (let i = 1; i < refs.length - 1; i += 1) {
          const b = verts[refs[i]];
          const c = verts[refs[i + 1]];
          pushTriangle(pos, nrm, a, b, c, null, bounds);
        }
      }
    }
    return normalizeMeshData(pos, nrm, { parser: "obj", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  function parseAsciiStl(text, importSource = {}) {
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    const vertexRegex = /vertex\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)/g;
    const normalRegex = /facet normal\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)\s+([+-]?\d*\.?\d+(?:[eE][+-]?\d+)?)/g;
    const normals = [];
    let match;
    while ((match = normalRegex.exec(text))) {
      normals.push([Number(match[1]), Number(match[2]), Number(match[3])]);
    }
    const vertices = [];
    while ((match = vertexRegex.exec(text))) {
      vertices.push([Number(match[1]), Number(match[2]), Number(match[3])]);
    }
    const triCount = Math.floor(vertices.length / 3);
    for (let i = 0; i < triCount; i += 1) {
      const a = vertices[i * 3];
      const b = vertices[i * 3 + 1];
      const c = vertices[i * 3 + 2];
      const normal = normals[i];
      pushTriangle(pos, nrm, a, b, c, normal, bounds);
    }
    return normalizeMeshData(pos, nrm, { parser: "stl-ascii", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  function parseBinaryStl(buffer, importSource = {}) {
    const view = new DataView(buffer);
    if (view.byteLength < 84) return null;
    const faceCount = view.getUint32(80, true);
    const expected = 84 + faceCount * 50;
    if (expected > view.byteLength) return null;
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    let offset = 84;
    for (let i = 0; i < faceCount; i += 1) {
      const normal = [
        view.getFloat32(offset, true),
        view.getFloat32(offset + 4, true),
        view.getFloat32(offset + 8, true),
      ];
      offset += 12;
      const a = [view.getFloat32(offset, true), view.getFloat32(offset + 4, true), view.getFloat32(offset + 8, true)];
      offset += 12;
      const b = [view.getFloat32(offset, true), view.getFloat32(offset + 4, true), view.getFloat32(offset + 8, true)];
      offset += 12;
      const c = [view.getFloat32(offset, true), view.getFloat32(offset + 4, true), view.getFloat32(offset + 8, true)];
      offset += 12;
      pushTriangle(pos, nrm, a, b, c, normal, bounds);
      offset += 2;
    }
    return normalizeMeshData(pos, nrm, { parser: "stl-binary", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  function parseAsciiPly(text, importSource = {}) {
    const lines = String(text || "").split(/\r?\n/);
    if (!/^ply\s*$/i.test(lines[0] || "")) return null;
    let vertexCount = 0;
    let faceCount = 0;
    let headerEnd = -1;
    for (let i = 1; i < lines.length; i += 1) {
      const line = lines[i].trim();
      if (line.startsWith("element vertex")) vertexCount = Number(line.split(/\s+/)[2] || 0);
      if (line.startsWith("element face")) faceCount = Number(line.split(/\s+/)[2] || 0);
      if (line === "end_header") {
        headerEnd = i;
        break;
      }
    }
    if (headerEnd < 0 || vertexCount <= 0) return null;
    const verts = [];
    for (let i = 0; i < vertexCount; i += 1) {
      const parts = String(lines[headerEnd + 1 + i] || "").trim().split(/\s+/);
      if (parts.length < 3) continue;
      verts.push([Number(parts[0]), Number(parts[1]), Number(parts[2])]);
    }
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    for (let i = 0; i < faceCount; i += 1) {
      const parts = String(lines[headerEnd + 1 + vertexCount + i] || "").trim().split(/\s+/).map(Number);
      const count = parts[0];
      if (!Number.isFinite(count) || count < 3) continue;
      const refs = parts.slice(1, 1 + count).filter((index) => index >= 0 && index < verts.length);
      if (refs.length < 3) continue;
      const a = verts[refs[0]];
      for (let j = 1; j < refs.length - 1; j += 1) {
        pushTriangle(pos, nrm, a, verts[refs[j]], verts[refs[j + 1]], null, bounds);
      }
    }
    return normalizeMeshData(pos, nrm, { parser: "ply-ascii", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  function parseOffMesh(text, importSource = {}) {
    const lines = String(text || "")
      .split(/\r?\n/)
      .map((line) => line.replace(/#.*/, "").trim())
      .filter(Boolean);
    if (!lines.length || !/^OFF$/i.test(lines[0])) return null;
    const counts = lines[1]?.split(/\s+/).map(Number) || [];
    const vertexCount = counts[0] || 0;
    const faceCount = counts[1] || 0;
    if (vertexCount <= 0 || faceCount <= 0) return null;
    const verts = [];
    for (let i = 0; i < vertexCount; i += 1) {
      const parts = (lines[2 + i] || "").split(/\s+/).map(Number);
      if (parts.length < 3) continue;
      verts.push([parts[0], parts[1], parts[2]]);
    }
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    for (let i = 0; i < faceCount; i += 1) {
      const parts = (lines[2 + vertexCount + i] || "").split(/\s+/).map(Number);
      const count = parts[0];
      if (!Number.isFinite(count) || count < 3) continue;
      const refs = parts.slice(1, 1 + count).filter((index) => index >= 0 && index < verts.length);
      if (refs.length < 3) continue;
      const a = verts[refs[0]];
      for (let j = 1; j < refs.length - 1; j += 1) {
        pushTriangle(pos, nrm, a, verts[refs[j]], verts[refs[j + 1]], null, bounds);
      }
    }
    return normalizeMeshData(pos, nrm, { parser: "off", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  function decodeBase64Uri(uri) {
    const comma = uri.indexOf(",");
    if (comma < 0) return null;
    const payload = uri.slice(comma + 1);
    const binary = atob(payload);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    return bytes.buffer;
  }

  function gltfComponentArray(buffer, componentType) {
    switch (componentType) {
      case 5120: return new Int8Array(buffer);
      case 5121: return new Uint8Array(buffer);
      case 5122: return new Int16Array(buffer);
      case 5123: return new Uint16Array(buffer);
      case 5125: return new Uint32Array(buffer);
      case 5126: return new Float32Array(buffer);
      default: return null;
    }
  }

  function gltfNumComponents(type) {
    if (type === "SCALAR") return 1;
    if (type === "VEC2") return 2;
    if (type === "VEC3") return 3;
    if (type === "VEC4") return 4;
    if (type === "MAT2") return 4;
    if (type === "MAT3") return 9;
    if (type === "MAT4") return 16;
    return 0;
  }

  function readGltfAccessor(json, accessorIndex, buffers) {
    const accessor = json.accessors?.[accessorIndex];
    const bufferView = json.bufferViews?.[accessor?.bufferView];
    if (!accessor || !bufferView) return null;
    const source = buffers[bufferView.buffer];
    if (!source) return null;
    const componentCount = gltfNumComponents(accessor.type);
    if (!componentCount || !accessor.count) return null;
    const bytesPerComponent = ({
      5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4,
    })[accessor.componentType];
    if (!bytesPerComponent) return null;
    const viewByteOffset = bufferView.byteOffset || 0;
    const accessorByteOffset = accessor.byteOffset || 0;
    const stride = bufferView.byteStride || (componentCount * bytesPerComponent);
    const readData = new Float32Array(accessor.count * componentCount);
    const dataView = new DataView(source, viewByteOffset + accessorByteOffset, Math.max(0, source.byteLength - viewByteOffset - accessorByteOffset));
    const normalized = !!accessor.normalized;
    const readScalar = (offset) => {
      switch (accessor.componentType) {
        case 5120: {
          const value = dataView.getInt8(offset);
          return normalized ? Math.max(value / 127, -1) : value;
        }
        case 5121: {
          const value = dataView.getUint8(offset);
          return normalized ? value / 255 : value;
        }
        case 5122: {
          const value = dataView.getInt16(offset, true);
          return normalized ? Math.max(value / 32767, -1) : value;
        }
        case 5123: {
          const value = dataView.getUint16(offset, true);
          return normalized ? value / 65535 : value;
        }
        case 5125:
          return dataView.getUint32(offset, true);
        case 5126:
          return dataView.getFloat32(offset, true);
        default:
          return 0;
      }
    };
    for (let i = 0; i < accessor.count; i += 1) {
      const base = i * stride;
      for (let j = 0; j < componentCount; j += 1) {
        readData[i * componentCount + j] = readScalar(base + j * bytesPerComponent);
      }
    }
    return {
      data: readData,
      componentCount,
      count: accessor.count,
    };
  }

  function applyMatrixToPoint(mat, point) {
    return [
      mat[0] * point[0] + mat[4] * point[1] + mat[8]  * point[2] + mat[12],
      mat[1] * point[0] + mat[5] * point[1] + mat[9]  * point[2] + mat[13],
      mat[2] * point[0] + mat[6] * point[1] + mat[10] * point[2] + mat[14],
    ];
  }

  function applyMatrixToDirection(mat, dir) {
    const x = mat[0] * dir[0] + mat[4] * dir[1] + mat[8]  * dir[2];
    const y = mat[1] * dir[0] + mat[5] * dir[1] + mat[9]  * dir[2];
    const z = mat[2] * dir[0] + mat[6] * dir[1] + mat[10] * dir[2];
    const len = Math.hypot(x, y, z) || 1;
    return [x / len, y / len, z / len];
  }

  function gltfNodeMatrix(node) {
    if (Array.isArray(node?.matrix) && node.matrix.length === 16) {
      return new Float32Array(node.matrix);
    }
    const t = Array.isArray(node?.translation) ? node.translation : [0, 0, 0];
    const s = Array.isArray(node?.scale) ? node.scale : [1, 1, 1];
    const q = Array.isArray(node?.rotation) ? node.rotation : [0, 0, 0, 1];
    const x = q[0], y = q[1], z = q[2], w = q[3];
    const x2 = x + x, y2 = y + y, z2 = z + z;
    const xx = x * x2, xy = x * y2, xz = x * z2;
    const yy = y * y2, yz = y * z2, zz = z * z2;
    const wx = w * x2, wy = w * y2, wz = w * z2;
    const m = new Float32Array(16);
    m[0] = (1 - (yy + zz)) * s[0];
    m[1] = (xy + wz) * s[0];
    m[2] = (xz - wy) * s[0];
    m[3] = 0;
    m[4] = (xy - wz) * s[1];
    m[5] = (1 - (xx + zz)) * s[1];
    m[6] = (yz + wx) * s[1];
    m[7] = 0;
    m[8] = (xz + wy) * s[2];
    m[9] = (yz - wx) * s[2];
    m[10] = (1 - (xx + yy)) * s[2];
    m[11] = 0;
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m[15] = 1;
    return m;
  }

  function collectGltfPrimitive(pos, nrm, json, primitive, buffers, worldMatrix, bounds = null) {
    const positions = readGltfAccessor(json, primitive.attributes?.POSITION, buffers);
    if (!positions || positions.componentCount < 3) return;
    const normals = primitive.attributes?.NORMAL != null ? readGltfAccessor(json, primitive.attributes.NORMAL, buffers) : null;
    let indices = null;
    if (primitive.indices != null) {
      const accessor = readGltfAccessor(json, primitive.indices, buffers);
      if (accessor) indices = Array.from(accessor.data.slice(0, accessor.count));
    }
    const mode = primitive.mode == null ? 4 : primitive.mode;
    const getPosition = (index) => {
      const i = index * positions.componentCount;
      return [
        positions.data[i] || 0,
        positions.data[i + 1] || 0,
        positions.data[i + 2] || 0,
      ];
    };
    const getNormal = (index) => {
      if (!normals || normals.componentCount < 3) return null;
      const i = index * normals.componentCount;
      return [
        normals.data[i] || 0,
        normals.data[i + 1] || 0,
        normals.data[i + 2] || 0,
      ];
    };
    const emitTriangle = (ia, ib, ic) => {
      const a = applyMatrixToPoint(worldMatrix, getPosition(ia));
      const b = applyMatrixToPoint(worldMatrix, getPosition(ib));
      const c = applyMatrixToPoint(worldMatrix, getPosition(ic));
      const na = getNormal(ia);
      const nb = getNormal(ib);
      const nc = getNormal(ic);
      if (na && nb && nc) {
        const an = applyMatrixToDirection(worldMatrix, na);
        const bn = applyMatrixToDirection(worldMatrix, nb);
        const cn = applyMatrixToDirection(worldMatrix, nc);
        pos.push(...a, ...b, ...c);
        nrm.push(...an, ...bn, ...cn);
        trackBoomBoundsPoint(bounds, a);
        trackBoomBoundsPoint(bounds, b);
        trackBoomBoundsPoint(bounds, c);
      } else {
        pushTriangle(pos, nrm, a, b, c, null, bounds);
      }
    };
    const refs = indices || Array.from({ length: positions.count }, (_, i) => i);
    if (mode === 4) {
      for (let i = 0; i + 2 < refs.length; i += 3) emitTriangle(refs[i], refs[i + 1], refs[i + 2]);
    } else if (mode === 5) {
      for (let i = 0; i + 2 < refs.length; i += 1) {
        if (i % 2 === 0) emitTriangle(refs[i], refs[i + 1], refs[i + 2]);
        else emitTriangle(refs[i + 1], refs[i], refs[i + 2]);
      }
    } else if (mode === 6) {
      for (let i = 1; i + 1 < refs.length; i += 1) emitTriangle(refs[0], refs[i], refs[i + 1]);
    }
  }

  async function parseGltfLike(json, buffers, importSource = {}) {
    const pos = [];
    const nrm = [];
    const bounds = createBoomBoundsTracker();
    const scenes = json.scenes || [];
    const nodes = json.nodes || [];
    const sceneIndex = json.scene || 0;
    const rootScene = scenes[sceneIndex] || scenes[0];
    if (!rootScene) return null;
    const visitNode = (nodeIndex, parentMatrix) => {
      const node = nodes[nodeIndex];
      if (!node) return;
      const localMatrix = gltfNodeMatrix(node);
      const worldMatrix = parentMatrix ? M4.multiply(parentMatrix, localMatrix) : localMatrix;
      const mesh = json.meshes?.[node.mesh];
      if (mesh?.primitives) {
        for (const primitive of mesh.primitives) {
          collectGltfPrimitive(pos, nrm, json, primitive, buffers, worldMatrix, bounds);
        }
      }
      for (const childIndex of node.children || []) visitNode(childIndex, worldMatrix);
    };
    for (const nodeIndex of rootScene.nodes || []) visitNode(nodeIndex, null);
    return normalizeMeshData(pos, nrm, { parser: "gltf", ...importSource, boundsHint: boomBoundsHintFromTracker(bounds) });
  }

  async function parseGltfMesh(file, companionFiles = []) {
    const text = await file.text();
    const json = JSON.parse(text);
    const sourceParts = [boomTextSourcePart(text, String(file?.name || "scene.gltf"))];
    const companionMap = new Map(
      companionFiles
        .filter((candidate) => candidate && candidate !== file)
        .map((candidate) => [String(candidate.name || "").toLowerCase(), candidate])
    );
    const buffers = [];
    for (const bufferDef of json.buffers || []) {
      if (typeof bufferDef.uri === "string" && bufferDef.uri.startsWith("data:")) {
        const decoded = decodeBase64Uri(bufferDef.uri);
        buffers.push(decoded);
        sourceParts.push(boomBufferSourcePart(decoded, `data:${buffers.length - 1}`));
      } else if (typeof bufferDef.uri === "string" && !bufferDef.uri.startsWith("data:")) {
        const rawName = decodeURIComponent(bufferDef.uri.split("/").pop() || "").toLowerCase();
        const sibling = companionMap.get(rawName);
        const siblingBuffer = sibling ? await sibling.arrayBuffer() : new ArrayBuffer(0);
        buffers.push(siblingBuffer);
        sourceParts.push(boomBufferSourcePart(siblingBuffer, rawName || `buffer:${buffers.length - 1}`));
      } else {
        buffers.push(new ArrayBuffer(0));
        sourceParts.push(boomBufferSourcePart(new ArrayBuffer(0), `empty:${buffers.length - 1}`));
      }
    }
    return parseGltfLike(json, buffers, boomImportSourceMeta("gltf", file?.name || "", sourceParts));
  }

  async function parseGlbMesh(file) {
    const buffer = await file.arrayBuffer();
    const importSource = boomImportSourceMeta("glb", file?.name || "", [
      boomBufferSourcePart(buffer, String(file?.name || "scene.glb")),
    ]);
    const view = new DataView(buffer);
    if (view.byteLength < 20 || view.getUint32(0, true) !== 0x46546c67) return null;
    const jsonChunks = [];
    const binChunks = [];
    let offset = 12;
    while (offset + 8 <= view.byteLength) {
      const length = view.getUint32(offset, true);
      const type = view.getUint32(offset + 4, true);
      const dataStart = offset + 8;
      const dataEnd = dataStart + length;
      if (dataEnd > view.byteLength) break;
      const chunk = buffer.slice(dataStart, dataEnd);
      if (type === 0x4E4F534A) jsonChunks.push(chunk);
      if (type === 0x004E4942) binChunks.push(chunk);
      offset = dataEnd;
    }
    if (!jsonChunks.length) return null;
    const jsonText = new TextDecoder().decode(jsonChunks[0]).replace(/\0+$/, "");
    const json = JSON.parse(jsonText);
    const buffers = [];
    let binIndex = 0;
    for (const bufferDef of json.buffers || []) {
      if (typeof bufferDef.uri === "string" && bufferDef.uri.startsWith("data:")) {
        buffers.push(decodeBase64Uri(bufferDef.uri));
      } else if (typeof bufferDef.uri === "string") {
        buffers.push(new ArrayBuffer(0));
      } else {
        buffers.push(binChunks[binIndex++] || new ArrayBuffer(0));
      }
    }
    return parseGltfLike(json, buffers, importSource);
  }

  // ---------- renderer state ----------
  let gl = null;
  let meshProg = null, lineProg = null, sdfProg = null;
  let cubeVAO = null, gridVAO = null;
  let cubeBuffers = []; // collected buffers for release
  let gridBuffers = [];
  let cubeCount = 0, gridCount = 0;
  let uMeshModel, uMeshProj, uMeshView, uMeshColor, uMeshClipOffset;
  let uLineProj, uLineView, uLineFadeNear, uLineFadeFar, uLineClipOffset;
  let uSdfResolution, uSdfCameraPos, uSdfCameraFwd, uSdfCameraRight, uSdfCameraUp, uSdfTanHalfFovY, uSdfViewProj;

  // Camera state survives suspend/resume — it's pure JS, no GPU resources.
  let camera = {
    azimuth: -Math.PI / 4,   // around Z
    elevation: Math.PI / 5,  // above XY plane
    distance: 22,
    target: [0, 0, 0],
  };
  let lastFps = 0, fpsAccum = 0, fpsFrames = 0, fpsTimer = 0;
  let raf = 0;
  let boomRenderDirty = true;
  let boomRenderReason = "initial";
  let boomRenderContinuousUntil = 0;
  let boomRenderStats = {
    requests: 0,
    frames: 0,
    dirtyFrames: 0,
    continuousFrames: 0,
    idleSkips: 0,
    lastReason: "initial",
    lastFrameAtMs: 0,
  };
  let boomUiRenderPending = false;
  let boomUiRenderMask = 0;
  let boomUiRenderReason = "";
  let boomUiRenderSeq = 0;
  let boomSidebarHtmlHash = "";
  let boomViewportHudHtmlHash = "";
  const BOOM_UI_RENDER_SIDEBAR = 1;
  const BOOM_UI_RENDER_HUD = 2;
  let boomUiRenderStats = {
    requests: 0,
    flushes: 0,
    coalesced: 0,
    sidebarFlushes: 0,
    sidebarSkips: 0,
    hudFlushes: 0,
    hudSkips: 0,
    contractSyncs: 0,
    lastReason: "",
  };

  // ---------- lifecycle state machine ----------
  // idle      : overlay never opened OR user closed it. Zero GPU resources.
  // active    : overlay open + window focused + page visible. Full GPU + RAF.
  // suspended : overlay open but window blurred / page hidden. GPU released, RAF stopped.
  // shutdown  : transient state during teardown.
  let gpuState = "idle";
  let inputAttached = false;
  let dropAttached = false;
  let gpuStatusEl = null;
  let runtimeStatus = null;
  let boomSidebarRoot = null;
  let boomSidebarBound = false;
  let boomViewportHud = null;
  let boomViewportHudBound = false;
  let boomSelectionOverlay = null;
  let boomDropOverlay = null;
  let boomDragDepth = 0;
  let lastProj = null;
  let lastView = null;
  let lastClipOffset = [0, 0];
  let sceneMesh = null;
  let slicerPreview = null;
  let boomModifierSeq = 0;
  let boomKasmGraph = null;
  let boomKasmQueries = null;
  let boomSpatialTools = null;
  let boomUiContract = null;
  let boomAnimationState = null;
  let boomPickHandle = null;
  let boomPickHandleStats = {
    hits: 0,
    misses: 0,
    invalidations: 0,
    faceTestsAvoided: 0,
    triangleTests: 0,
    candidateTests: 0,
    bytes: 0,
    lastBuildMs: 0,
    lastKey: "",
  };
  const BOOM_COMPUTE_CACHE_MAX_ENTRIES = 256;
  const BOOM_COMPUTE_CACHE_MAX_BYTES = 96 * 1024 * 1024;
  const BOOM_GPU_RESOURCE_CACHE_MAX_ENTRIES = 96;
  const BOOM_GPU_RESOURCE_CACHE_MAX_BYTES = 128 * 1024 * 1024;
  const BOOM_AUDIT_LOG_LIMIT = 240;
  let boomComputeCache = new Map();
  let boomComputeCacheBytes = 0;
  let boomCacheStats = { hits: 0, misses: 0, evictions: 0, evictedBytes: 0, oversizedSkips: 0 };
  let boomGpuResourceCache = new Map();
  let boomGpuResourceBytes = 0;
  let boomGpuStats = { hits: 0, misses: 0, evictions: 0, evictedBytes: 0, protectedSkips: 0, oversizedSkips: 0 };
  let boomAuditLog = [];
  const BOOM_KASM_RUN_HISTORY_LIMIT = 96;
  const BOOM_KASM_HASH_INDEX_LIMIT = 512;
  const BOOM_KASM_METRIC_HISTORY_LIMIT = 160;
  const BOOM_KASM_PROGRAM_HISTORY_LIMIT = 128;
  const BOOM_KASM_MATRIX_HISTORY_LIMIT = 96;
  const BOOM_KASM_SKILL_HISTORY_LIMIT = 96;
  const BOOM_KASM_RENDER_HISTORY_LIMIT = 96;
  const BOOM_KASM_ASSET_HISTORY_LIMIT = 192;
  const BOOM_KASM_COMPUTE_HISTORY_LIMIT = 96;
  const BOOM_KASM_TEMPLATE_HISTORY_LIMIT = 64;
  const BOOM_KASM_MCP_HISTORY_LIMIT = 64;
  const BOOM_KASM_ASSET_PAGE_BYTES = 256 * 1024;
  const BOOM_KASM_TEMPLATE_CATALOG = [
    "template.world_patch.basic",
    "template.entity.spawn",
    "template.material.pbr",
    "template.material.procedural",
    "template.mesh.meshletize",
    "template.mesh.simplify",
    "template.texture.compress",
    "template.texture.virtualize",
    "template.compute.shader",
    "template.compute.gpu_cull_instances",
    "template.compute.lod_select",
    "template.compute.metric_eval",
    "template.scene.optimize_vram",
    "template.scene.optimize_drawcalls",
    "template.scene.generate_layout",
    "template.scene.place_lights",
    "template.scene.generate_collision",
    "template.scene.generate_navmesh",
    "template.metric.vram_cost",
    "template.metric.draw_call_cost",
    "template.metric.scene_complexity",
    "template.metric.asset_duplication",
    "template.metric.lod_error",
    "template.metric.asset_ram_cost",
    "template.metric.asset_vram_cost",
    "template.metric.asset_evictable_pages",
    "template.metric.cluster_vram_cost",
    "template.metric.cluster_lod_error",
    "template.metric.cluster_draw_cost",
    "template.metric.cluster_stream_cost",
    "template.metric.navigation_quality",
    "template.metric.composition_score",
    "template.skill.import_asset_pipeline",
    "template.skill.generate_playable_scene",
    "template.skill.optimize_scene",
    "template.skill.create_interactive_object",
  ];
  const BOOM_KASM_GRAPH_VIEWS = [
    { id: "world", label: "World", icon: "world" },
    { id: "assets", label: "Assets", icon: "material" },
    { id: "skills", label: "Skills", icon: "wrench" },
    { id: "programs", label: "Programs", icon: "scene" },
    { id: "runs", label: "Runs", icon: "render" },
  ];
  const BOOM_KASM_MCP_TOOL_CATALOG = [
    { name: "kasm.create_program", slash: "/create_program", outputKind: "program_spec" },
    { name: "kasm.run_program", slash: "/program run", outputKind: "program_run" },
    { name: "kasm.create_metric", slash: "/create_metric", outputKind: "metric_spec" },
    { name: "kasm.run_metric", slash: "/metric run", outputKind: "metric_record" },
    { name: "kasm.run_matrix", slash: "/matrix run", outputKind: "matrix_run" },
    { name: "kasm.create_skill", slash: "/skill create", outputKind: "skill_spec" },
    { name: "kasm.run_skill", slash: "/skill run", outputKind: "skill_run" },
    { name: "kasm.promote_skill", slash: "/skill promote", outputKind: "skill_spec" },
    { name: "kasm.render_frame", slash: "/render frame", outputKind: "render_ir" },
    { name: "kasm.compute_dispatch", slash: "/program run", outputKind: "compute_dispatch" },
    { name: "kasm.asset_scan", slash: "/asset scan", outputKind: "asset_pack" },
    { name: "kasm.asset_residency", slash: "/asset residency", outputKind: "asset_residency_plan" },
    { name: "kasm.asset_evict_cold", slash: "/asset evict_cold", outputKind: "asset_residency_plan" },
    { name: "kasm.asset_pin_hot", slash: "/asset pin_hot", outputKind: "asset_residency_plan" },
    { name: "kasm.cache_stats", slash: "/cache stats", outputKind: "metric_summary" },
    { name: "kasm.status", slash: "/status current_run", outputKind: "run_status" },
    { name: "kasm.prove", slash: "/prove", outputKind: "proof_record" },
    { name: "kasm.explain", slash: "/explain", outputKind: "hash_explanation" },
    { name: "kasm.rollback", slash: "/world rollback", outputKind: "world_patch" },
  ];
  const BOOM_KASM_MCP_RESOURCE_URIS = [
    "kasm://graph",
    "kasm://templates",
    "kasm://programs",
    "kasm://metrics",
    "kasm://skills",
    "kasm://runs",
    "kasm://proofs",
    "kasm://assets",
    "kasm://render",
    "kasm://compute",
    "kasm://status",
  ];
  const BOOM_KASM_MCP_PROMPT_CATALOG = [
    "prompt_to_kasm_program",
    "matrix_creative_search",
    "auto_optimizer",
    "hash_time_machine",
    "asset_brain",
  ];
  let boomKasmRunHistory = [];
  let boomKasmProofHistory = [];
  let boomKasmPatchHistory = [];
  let boomKasmRollbackHistory = [];
  let boomKasmMetricSpecHistory = [];
  let boomKasmMetricHistory = [];
  let boomKasmMetricRegistry = new Map();
  let boomKasmProgramHistory = [];
  let boomKasmProgramRunHistory = [];
  let boomKasmProgramRegistry = new Map();
  let boomKasmMatrixHistory = [];
  let boomKasmSkillHistory = [];
  let boomKasmSkillRunHistory = [];
  let boomKasmSkillRegistry = new Map();
  let boomKasmRenderHistory = [];
  let boomKasmAssetPageHistory = [];
  let boomKasmAssetResidencyHistory = [];
  let boomKasmGeoClusterHistory = [];
  let boomKasmComputeHistory = [];
  let boomKasmComputeRegistry = new Map();
  let boomKasmTemplateHistory = [];
  let boomKasmMcpFacadeHistory = [];
  let boomKasmMcpFacade = null;
  let boomKasmHashIndex = new Map();
  let boomKasmHashIndexOrder = [];
  let boomKasmSpineStats = { commandSpecs: 0, bytecodePrograms: 0, sandboxMatrices: 0, metricSpecs: 0, metricRecords: 0, programSpecs: 0, programRuns: 0, matrixRuns: 0, skillSpecs: 0, skillRuns: 0, renderIRs: 0, assetPages: 0, assetResidencyPlans: 0, geoClusters: 0, computePrograms: 0, computeRuns: 0, templates: 0, mcpFacades: 0, mcpToolCalls: 0, mcpResourceReads: 0, mcpPromptReads: 0, worldPatches: 0, rollbackPatches: 0, runRecords: 0, proofRecords: 0 };

  const BOOM_PROPERTY_TABS = [
    { id: "slicer", title: "Slicer", icon: "printer" },
    { id: "object", title: "Object", icon: "object" },
    { id: "modifiers", title: "Modifiers", icon: "wrench" },
    { id: "material", title: "Material", icon: "material" },
    { id: "scene", title: "Scene", icon: "scene" },
  ];

  const BOOM_EDIT_MODES = [
    { id: "object", title: "Object" },
    { id: "vertex", title: "Vertex" },
    { id: "edge", title: "Edge" },
    { id: "face", title: "Face" },
  ];

  const BOOM_MODIFIER_PRESETS = [
    { type: "mirror", title: "Mirror", axis: "X", copy: "symmetry" },
    { type: "array", title: "Array", axis: "X", count: 3, offset: 2.25, copy: "repeat" },
    { type: "inflate", title: "Inflate", amount: 1.08, copy: "volume" },
    { type: "bevel", title: "Bevel", width: 0.14, copy: "chamfer" },
    { type: "subdivide", title: "Subdivide", levels: 1, copy: "refine" },
    { type: "solidify", title: "Solidify", thickness: 0.2, copy: "thickness" },
  ];

  const boomScene = {
    activeId: "grid",
    workspaceMode: "design",
    propertyTab: "object",
    editMode: "object",
    componentSelection: null,
    regionSelection: null,
    filter: "",
    collectionExpanded: true,
    kasmGraphView: "world",
    selectedKasmHash: "",
    slicer: {
      workflow: "prepare",
      mode: "recommended",
      level: "advanced",
      printerProfile: "CoreXY 0.4 nozzle",
      materialProfile: "PLA 1.75",
      qualityPreset: "0.20 mm Balanced",
      layerHeight: 0.20,
      wallLoops: 3,
      infillDensity: 18,
      infillPattern: "Gyroid",
      supportMode: "Organic",
      adhesion: "Brim",
      seam: "Aligned",
      adaptiveLayers: true,
      speedPreset: "Balanced",
      printSpeed: 160,
      nozzleTemp: 210,
      bedTemp: 60,
      discoveryState: "idle",
      profiles: [],
      devices: [],
      discoveryWarnings: [],
      discoveryBackend: "",
      },
    items: [
      {
        id: "camera",
        name: "Camera",
        type: "camera",
        visible: true,
        selectable: true,
        renderable: true,
        transform: {
          location: [11.33, -11.33, 8.60],
          rotation: [63.4, 0, 45],
          scale: [1, 1, 1],
          mode: "XYZ Euler",
        },
      },
      {
        id: "grid",
        name: "Grid",
        type: "grid",
        visible: true,
        selectable: true,
        renderable: true,
        transform: {
          location: [0, 0, 0],
          rotation: [0, 0, 0],
          scale: [1, 1, 1],
          mode: "XYZ Euler",
        },
      },
      {
        id: "light",
        name: "Light",
        type: "light",
        visible: true,
        selectable: true,
        renderable: true,
        transform: {
          location: [6.2, -4.1, 8.8],
          rotation: [52, 0, 36],
          scale: [1, 1, 1],
          mode: "XYZ Euler",
        },
      },
    ],
  };

  // Tauri IPC bridge to the native BangerEngine (P1a). If Tauri isn't loaded
  // (e.g. plain-browser dev), backendInvoke returns null and the WebGL
  // viewport still works — but the native engine just won't be claimed.
  let backendBusy = null; // most-recent in-flight backend call, for sequencing
  function backendInvoke(cmd, args = undefined) {
    const runtimeInvoke = window.ForgeShellRuntime?.tauri?.invoke || window.ForgeTauriBridge?.invoke || null;
    if (!runtimeInvoke) return Promise.resolve(null);
    return runtimeInvoke(cmd, args && typeof args === "object" ? args : {}, { section: "banger" }).catch((err) => {
      console.warn(`[banger] backend ${cmd} failed:`, err);
      return null;
    });
  }
  function applyBackendStatus(status) {
    if (!status || !gpuStatusEl) return;
    if (status.state === "active") {
      const tag = status.backend ? `${status.backend}` : "active";
      const name = status.adapter_name ? ` · ${status.adapter_name}` : "";
      setGpuStatus(`GPU ${tag}${name}`, "active");
    } else if (status.state === "stopped") {
      setGpuStatus("GPU paused", "paused");
    } else {
      setGpuStatus("GPU idle", "paused");
    }
  }

  function applyRuntimeStatus(status) {
    if (!status) return;
    runtimeStatus = status;
    if (typeof window !== "undefined") {
      window.__forgeBoomRuntimeStatus = status;
    }
    const warmed = status.backendReady && status.atlasAttached;
    const programs = Number(status.installedPrograms || 0);
    const cacheEntries = Number(status.runCacheEntries || 0) + Number(status.inspectCacheEntries || 0);
    if (warmed) {
      setGpuStatus(`KASM warm · atlas ready · ${programs} progs`, "active");
    } else {
      setGpuStatus("KASM cold", "paused");
    }
    try {
      els.view.dataset.runtimeReady = warmed ? "true" : "false";
      els.view.dataset.runtimePrograms = String(programs);
      els.view.dataset.runtimeCaches = String(cacheEntries);
    } catch (_) {}
    console.info("[banger] runtime status", status);
  }

  function radToDeg(rad) {
    return rad * 180 / Math.PI;
  }

  function formatScalar(value, digits = 3) {
    return Number(value || 0).toFixed(digits);
  }

  function formatBoomBytes(value = 0) {
    const bytes = Math.max(0, Number(value) || 0);
    const units = ["B", "KB", "MB", "GB"];
    let next = bytes;
    let unit = 0;
    while (next >= 1024 && unit < units.length - 1) {
      next /= 1024;
      unit += 1;
    }
    const digits = unit <= 1 ? 0 : 1;
    return `${next.toFixed(digits)} ${units[unit]}`;
  }

  function formatAngle(value) {
    return Number(value || 0).toFixed(1);
  }

  function escapeBoomHtml(value) {
    return String(value ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function stableBoomValue(value) {
    if (Array.isArray(value)) return value.map(stableBoomValue);
    if (value && typeof value === "object") {
      const out = {};
      for (const key of Object.keys(value).sort()) out[key] = stableBoomValue(value[key]);
      return out;
    }
    return value;
  }

  function stableBoomStringify(value) {
    return JSON.stringify(stableBoomValue(value));
  }

  function boomNowMs() {
    return typeof performance !== "undefined" && performance.now ? performance.now() : Date.now();
  }

  function boomFrameSafe(ms) {
    return Number(ms || 0) <= 16.667;
  }

  function boomPercentile(values, p) {
    if (!values.length) return 0;
    const sorted = values.slice().sort((a, b) => a - b);
    const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
    return Number(sorted[index].toFixed(3));
  }

  function boomRenderSchedulerSnapshot() {
    return {
      queued: !!raf,
      dirty: boomRenderDirty,
      continuous: boomRenderContinuousActive(),
      continuousUntilMs: Number(Math.max(0, boomRenderContinuousUntil - boomNowMs()).toFixed(3)),
      ...boomRenderStats,
    };
  }

  function boomUiRenderSnapshot() {
    return {
      pending: boomUiRenderPending,
      mask: boomUiRenderMask,
      seq: boomUiRenderSeq,
      ...boomUiRenderStats,
    };
  }

  function boomPickHandleSnapshot() {
    return {
      active: !!boomPickHandle,
      faces: boomPickHandle?.meshFaces?.count || 0,
      componentFaces: boomPickHandle?.componentFaces?.count || 0,
      vertices: boomPickHandle?.vertices?.length || 0,
      edges: boomPickHandle?.edges?.length || 0,
      ...boomPickHandleStats,
    };
  }

  function boomCacheStatusSummary() {
    const latency = [];
    const byStage = {};
    for (const event of boomAuditLog) {
      latency.push(Number(event.elapsedMs || 0));
      if (!byStage[event.stage]) byStage[event.stage] = { events: 0, hits: 0, misses: 0, p50: 0, p95: 0, p99: 0, latency: [] };
      const bucket = byStage[event.stage];
      bucket.events += 1;
      bucket.latency.push(Number(event.elapsedMs || 0));
      if (event.status === "HIT") bucket.hits += 1;
      if (event.status === "MISS") bucket.misses += 1;
    }
    for (const bucket of Object.values(byStage)) {
      bucket.p50 = boomPercentile(bucket.latency, 50);
      bucket.p95 = boomPercentile(bucket.latency, 95);
      bucket.p99 = boomPercentile(bucket.latency, 99);
      delete bucket.latency;
    }
    const total = boomCacheStats.hits + boomCacheStats.misses;
    const cacheFillPct = Number(((boomComputeCacheBytes / BOOM_COMPUTE_CACHE_MAX_BYTES) * 100).toFixed(2));
    const memory = boomBrowserMemorySnapshot();
    return {
      kind: "boom-compute-audit-summary",
      version: 1,
      cacheEntries: boomComputeCache.size,
      cacheBytes: boomComputeCacheBytes,
      cacheMaxBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
      cacheFillPct,
      cachePressure: cacheFillPct >= 90 || boomCacheStats.evictions > 0 ? "pressure" : cacheFillPct >= 70 ? "watch" : "ok",
      gpuResourceEntries: boomGpuResourceCache.size,
      gpuResourceBytes: boomGpuResourceBytes,
      gpuResourceMaxBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
      gpuResourceFillPct: Number(((boomGpuResourceBytes / BOOM_GPU_RESOURCE_CACHE_MAX_BYTES) * 100).toFixed(2)),
      gpuResourceHits: boomGpuStats.hits,
      gpuResourceMisses: boomGpuStats.misses,
      gpuResourceEvictions: boomGpuStats.evictions,
      gpuResourceEvictedBytes: boomGpuStats.evictedBytes,
      gpuResourceProtectedSkips: boomGpuStats.protectedSkips,
      gpuResourceOversizedSkips: boomGpuStats.oversizedSkips,
      renderScheduler: boomRenderSchedulerSnapshot(),
      uiRender: boomUiRenderSnapshot(),
      pickHandle: boomPickHandleSnapshot(),
      memory,
      events: boomAuditLog.length,
      hits: boomCacheStats.hits,
      misses: boomCacheStats.misses,
      hitRate: total ? Number((boomCacheStats.hits / total).toFixed(4)) : 0,
      missRate: total ? Number((boomCacheStats.misses / total).toFixed(4)) : 0,
      evictions: boomCacheStats.evictions,
      evictedBytes: boomCacheStats.evictedBytes,
      oversizedSkips: boomCacheStats.oversizedSkips,
      avoided: boomCacheStats.hits,
      p50: boomPercentile(latency, 50),
      p95: boomPercentile(latency, 95),
      p99: boomPercentile(latency, 99),
      byStage,
      last: boomAuditLog[boomAuditLog.length - 1] || null,
    };
  }

  function boomBrowserMemorySnapshot() {
    const memory = typeof performance !== "undefined" ? performance.memory : null;
    if (!memory) return null;
    const limit = Number(memory.jsHeapSizeLimit || 0);
    const used = Number(memory.usedJSHeapSize || 0);
    return {
      usedJSHeapBytes: used,
      totalJSHeapBytes: Number(memory.totalJSHeapSize || 0),
      jsHeapLimitBytes: limit,
      heapFillPct: limit ? Number(((used / limit) * 100).toFixed(2)) : 0,
    };
  }

  function exposeBoomAuditState() {
    if (typeof window === "undefined") return;
    const summary = boomCacheStatusSummary();
    window.__forgeBoomAuditLog = boomAuditLog;
    window.__forgeBoomAuditSummary = summary;
    window.__forgeBoomClearAudit = () => {
      boomAuditLog = [];
      exposeBoomAuditState();
    };
    window.__forgeBoomClearComputeCache = () => {
      boomComputeCache.clear();
      boomComputeCacheBytes = 0;
      boomCacheStats = { hits: 0, misses: 0, evictions: 0, evictedBytes: 0, oversizedSkips: 0 };
      boomAuditLog = [];
      exposeBoomAuditState();
    };
    window.__forgeBoomClearGpuResourceCache = () => {
      clearBoomGpuResourceCache();
      exposeBoomAuditState();
    };
    window.__forgeBoomRenderStats = summary.renderScheduler;
    window.__forgeBoomUiRenderStats = summary.uiRender;
    window.__forgeBoomPickHandleStats = summary.pickHandle;
    window.__forgeBoomKasmRuns = boomKasmRunHistory;
    window.__forgeBoomKasmProofs = boomKasmProofHistory;
    window.__forgeBoomKasmPatches = boomKasmPatchHistory;
    window.__forgeBoomKasmRollbacks = boomKasmRollbackHistory;
    window.__forgeBoomKasmMetricSpecs = boomKasmMetricSpecHistory;
    window.__forgeBoomKasmMetrics = boomKasmMetricHistory;
    window.__forgeBoomKasmPrograms = boomKasmProgramHistory;
    window.__forgeBoomKasmProgramRuns = boomKasmProgramRunHistory;
    window.__forgeBoomKasmMatrices = boomKasmMatrixHistory;
    window.__forgeBoomKasmSkills = boomKasmSkillHistory;
    window.__forgeBoomKasmSkillRuns = boomKasmSkillRunHistory;
    window.__forgeBoomKasmRenderIR = boomKasmRenderHistory;
    window.__forgeBoomKasmAssetPages = boomKasmAssetPageHistory;
    window.__forgeBoomKasmAssetResidency = boomKasmAssetResidencyHistory;
    window.__forgeBoomKasmGeoClusters = boomKasmGeoClusterHistory;
    window.__forgeBoomKasmComputePrograms = boomKasmComputeHistory;
    window.__forgeBoomKasmTemplates = ensureBoomKasmTemplateCatalog();
    window.__forgeBoomKasmGraphProjection = buildBoomKasmGraphProjection();
    window.__forgeBoomKasmMcpFacades = boomKasmMcpFacadeHistory;
    window.__forgeBoomKasmMcpFacade = getBoomKasmMcpFacade();
    window.__forgeBoomKasmMcp = {
      facade: window.__forgeBoomKasmMcpFacade,
      callTool: runBoomKasmMcpTool,
      readResource: readBoomKasmMcpResource,
      getPrompt: getBoomKasmMcpPrompt,
    };
    window.__forgeBoomKasmSpineStats = boomKasmSpineStats;
    window.__forgeBoomResolveKasmHash = resolveBoomKasmHash;
    window.__forgeBoomExplainHash = explainBoomKasmHash;
    window.__forgeBoomRunKasmCommand = runBoomSlashCommand;
    window.__forgeBoomRequestRender = requestBoomRender;
    window.__forgeBoomRequestUiRender = requestBoomUiRender;
  }

  function boomAnimationIsActive() {
    return !!(boomAnimationState?.clip && boomAnimationState.playing);
  }

  function boomRenderContinuousActive(now = boomNowMs()) {
    return boomAnimationIsActive() || now < boomRenderContinuousUntil;
  }

  function requestBoomRender(reason = "dirty", continuousMs = 0) {
    boomRenderDirty = true;
    boomRenderReason = reason || "dirty";
    boomRenderStats.requests += 1;
    boomRenderStats.lastReason = boomRenderReason;
    const keepWarmMs = Number(continuousMs || 0);
    if (keepWarmMs > 0) {
      boomRenderContinuousUntil = Math.max(boomRenderContinuousUntil, boomNowMs() + keepWarmMs);
    }
    if (gpuState === "active" && gl && !raf) {
      raf = requestAnimationFrame(render);
    }
  }

  function queueBoomMicrotask(fn) {
    if (typeof queueMicrotask === "function") {
      queueMicrotask(fn);
    } else {
      Promise.resolve().then(fn);
    }
  }

  function requestBoomUiRender(mask = BOOM_UI_RENDER_SIDEBAR | BOOM_UI_RENDER_HUD, reason = "ui") {
    boomUiRenderMask |= mask;
    boomUiRenderReason = reason || boomUiRenderReason || "ui";
    boomUiRenderStats.requests += 1;
    boomUiRenderStats.lastReason = boomUiRenderReason;
    if (boomUiRenderPending) {
      boomUiRenderStats.coalesced += 1;
      return;
    }
    boomUiRenderPending = true;
    queueBoomMicrotask(flushBoomUiRenderQueue);
  }

  function renderBoomSidebar(reason = "sidebar") {
    requestBoomUiRender(BOOM_UI_RENDER_SIDEBAR, reason);
  }

  function renderBoomViewportHud(reason = "hud") {
    requestBoomUiRender(BOOM_UI_RENDER_HUD, reason);
  }

  function flushBoomUiRenderQueue() {
    if (!boomUiRenderPending) return;
    const mask = boomUiRenderMask;
    const reason = boomUiRenderReason || "ui";
    boomUiRenderPending = false;
    boomUiRenderMask = 0;
    boomUiRenderReason = "";
    boomUiRenderStats.flushes += 1;
    boomUiRenderStats.lastReason = reason;
    boomUiRenderSeq += 1;

    let changed = false;
    if (mask & BOOM_UI_RENDER_SIDEBAR) changed = flushBoomSidebar() || changed;
    if (mask & BOOM_UI_RENDER_HUD) changed = flushBoomViewportHud() || changed;
    if (changed) {
      syncBoomInteractionContract();
      boomUiRenderStats.contractSyncs += 1;
    }
  }

  function clearBoomPickHandle(reason = "invalidate") {
    if (boomPickHandle) {
      boomPickHandleStats.invalidations += 1;
      boomPickHandleStats.lastKey = reason;
    }
    boomPickHandle = null;
    boomPickHandleStats.bytes = 0;
  }

  function emitBoomAudit(stage, status, key, elapsedMs, workUnits = 0, unit = "", extra = {}) {
    const event = {
      kind: "boom-compute-audit",
      version: 1,
      stage,
      status,
      key,
      elapsedMs: Number(Number(elapsedMs || 0).toFixed(3)),
      workUnits: Number(workUnits || 0),
      unit,
      avoided: status === "HIT",
      frameSafe: boomFrameSafe(elapsedMs),
      cacheEntries: boomComputeCache.size,
      cacheBytes: boomComputeCacheBytes,
      cacheMaxBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
      gpuResourceEntries: boomGpuResourceCache.size,
      gpuResourceBytes: boomGpuResourceBytes,
      gpuResourceMaxBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
      memory: boomBrowserMemorySnapshot(),
      ...extra,
    };
    boomAuditLog.push(event);
    while (boomAuditLog.length > BOOM_AUDIT_LOG_LIMIT) boomAuditLog.shift();
    exposeBoomAuditState();
    if (typeof console !== "undefined" && console.debug) {
      console.debug("[banger-audit]", event);
    }
    return event;
  }

  function boomApproxBytes(value, seen = new Set()) {
    if (value == null) return 0;
    if (typeof value === "boolean") return 4;
    if (typeof value === "number") return 8;
    if (typeof value === "string") return value.length * 2;
    if (typeof value !== "object") return 16;
    if (seen.has(value)) return 0;
    seen.add(value);
    if (ArrayBuffer.isView(value)) return value.byteLength || 0;
    if (value instanceof ArrayBuffer) return value.byteLength || 0;
    if (Array.isArray(value)) {
      let bytes = 24 + value.length * 8;
      for (const entry of value) bytes += boomApproxBytes(entry, seen);
      return bytes;
    }
    let bytes = 32;
    for (const [key, entry] of Object.entries(value)) {
      bytes += key.length * 2 + boomApproxBytes(entry, seen);
    }
    return bytes;
  }

  function evictBoomComputeUntilFit(requiredBytes = 0) {
    let evicted = 0;
    let evictedBytes = 0;
    while (
      boomComputeCache.size > BOOM_COMPUTE_CACHE_MAX_ENTRIES ||
      (boomComputeCacheBytes + requiredBytes > BOOM_COMPUTE_CACHE_MAX_BYTES && boomComputeCache.size)
    ) {
      const firstKey = boomComputeCache.keys().next().value;
      if (!firstKey) break;
      const entry = boomComputeCache.get(firstKey);
      boomComputeCache.delete(firstKey);
      const bytes = entry?.bytes || 0;
      boomComputeCacheBytes = Math.max(0, boomComputeCacheBytes - bytes);
      evicted += 1;
      evictedBytes += bytes;
    }
    if (evicted) {
      boomCacheStats.evictions += evicted;
      boomCacheStats.evictedBytes += evictedBytes;
    }
    return { evicted, evictedBytes };
  }

  function rememberBoomCompute(key, value, stage = "") {
    const bytes = boomApproxBytes(value);
    if (bytes > BOOM_COMPUTE_CACHE_MAX_BYTES) {
      boomCacheStats.oversizedSkips += 1;
      return { stored: false, bytes, evicted: 0, evictedBytes: 0, reason: "oversized" };
    }
    if (boomComputeCache.has(key)) {
      const previous = boomComputeCache.get(key);
      boomComputeCacheBytes = Math.max(0, boomComputeCacheBytes - (previous?.bytes || 0));
      boomComputeCache.delete(key);
    }
    const eviction = evictBoomComputeUntilFit(bytes);
    boomComputeCache.set(key, { value, bytes, stage, touchedAt: boomNowMs() });
    boomComputeCacheBytes += bytes;
    return { stored: true, bytes, ...eviction };
  }

  function boomCachedCompute(stage, keyParts, workUnits, unit, compute) {
    const key = boomComputeCacheKey(stage, keyParts);
    const started = boomNowMs();
    if (boomComputeCache.has(key)) {
      const cached = boomComputeCache.get(key);
      boomComputeCache.delete(key);
      boomComputeCache.set(key, cached);
      cached.touchedAt = boomNowMs();
      boomCacheStats.hits += 1;
      emitBoomAudit(stage, "HIT", key, boomNowMs() - started, workUnits, unit, { bytes: cached.bytes || 0 });
      return { key, status: "HIT", value: cached.value };
    }
    boomCacheStats.misses += 1;
    const value = compute();
    const store = rememberBoomCompute(key, value, stage);
    emitBoomAudit(stage, "MISS", key, boomNowMs() - started, workUnits, unit, {
      bytes: store.bytes,
      stored: store.stored,
      evicted: store.evicted,
      evictedBytes: store.evictedBytes,
      cacheSkipReason: store.reason || "",
    });
    return { key, status: "MISS", value };
  }

  function boomComputeCacheKey(stage, keyParts) {
    return kasmHashString(`boom-cache-v2|${stage}|${stableBoomStringify(keyParts)}`);
  }

  function boomGpuResourceKey(kind, key) {
    return kasmHashString(`boom-gpu-resource-v1|${kind}|${key || "none"}`);
  }

  function activeBoomGpuResourceKeys() {
    return new Set([
      slicerPreview?.gpuCacheKey || "",
      sceneMesh?.display?.gpuCacheKey || "",
    ].filter(Boolean));
  }

  function deleteBoomGpuResource(resource) {
    if (!gl || !resource) return;
    try {
      for (const buffer of resource.buffers || []) gl.deleteBuffer(buffer);
      if (resource.vao) gl.deleteVertexArray(resource.vao);
    } catch (err) {
      console.warn("[banger] deleteBoomGpuResource error:", err);
    }
  }

  function evictBoomGpuResources(requiredBytes = 0) {
    let evicted = 0;
    let evictedBytes = 0;
    let rotations = 0;
    const protectedKeys = activeBoomGpuResourceKeys();
    while (
      boomGpuResourceCache.size > BOOM_GPU_RESOURCE_CACHE_MAX_ENTRIES ||
      (boomGpuResourceBytes + requiredBytes > BOOM_GPU_RESOURCE_CACHE_MAX_BYTES && boomGpuResourceCache.size)
    ) {
      const firstKey = boomGpuResourceCache.keys().next().value;
      if (!firstKey || rotations > boomGpuResourceCache.size + 4) break;
      const entry = boomGpuResourceCache.get(firstKey);
      if (protectedKeys.has(firstKey)) {
        boomGpuResourceCache.delete(firstKey);
        boomGpuResourceCache.set(firstKey, entry);
        boomGpuStats.protectedSkips += 1;
        rotations += 1;
        continue;
      }
      boomGpuResourceCache.delete(firstKey);
      deleteBoomGpuResource(entry?.resource);
      const bytes = entry?.bytes || 0;
      boomGpuResourceBytes = Math.max(0, boomGpuResourceBytes - bytes);
      evicted += 1;
      evictedBytes += bytes;
    }
    if (evicted) {
      boomGpuStats.evictions += evicted;
      boomGpuStats.evictedBytes += evictedBytes;
    }
    return { evicted, evictedBytes };
  }

  function rememberBoomGpuResource(key, resource, bytes, kind) {
    if (!key || !resource) return { stored: false, bytes: bytes || 0, evicted: 0, evictedBytes: 0, reason: "empty" };
    if (bytes > BOOM_GPU_RESOURCE_CACHE_MAX_BYTES) {
      boomGpuStats.oversizedSkips += 1;
      return { stored: false, bytes, evicted: 0, evictedBytes: 0, reason: "oversized" };
    }
    if (boomGpuResourceCache.has(key)) {
      const previous = boomGpuResourceCache.get(key);
      boomGpuResourceBytes = Math.max(0, boomGpuResourceBytes - (previous?.bytes || 0));
      boomGpuResourceCache.delete(key);
      if (previous?.resource !== resource) deleteBoomGpuResource(previous?.resource);
    }
    const eviction = evictBoomGpuResources(bytes);
    boomGpuResourceCache.set(key, { resource, bytes, kind, touchedAt: boomNowMs() });
    boomGpuResourceBytes += bytes;
    return { stored: true, bytes, ...eviction };
  }

  function lookupBoomGpuResource(stage, key, workUnits, unit) {
    if (!key || !boomGpuResourceCache.has(key)) {
      boomGpuStats.misses += 1;
      return null;
    }
    const started = boomNowMs();
    const entry = boomGpuResourceCache.get(key);
    boomGpuResourceCache.delete(key);
    boomGpuResourceCache.set(key, entry);
    entry.touchedAt = boomNowMs();
    boomGpuStats.hits += 1;
    emitBoomAudit(stage, "HIT", key, boomNowMs() - started, workUnits, unit, {
      bytes: entry.bytes || 0,
      gpuResourceHit: true,
    });
    return entry.resource;
  }

  function clearBoomGpuResourceCache() {
    for (const entry of boomGpuResourceCache.values()) {
      deleteBoomGpuResource(entry.resource);
    }
    boomGpuResourceCache.clear();
    boomGpuResourceBytes = 0;
    boomGpuStats = { hits: 0, misses: 0, evictions: 0, evictedBytes: 0, protectedSkips: 0, oversizedSkips: 0 };
  }

  function boomHashText(hash, text) {
    const source = String(text || "");
    for (let i = 0; i < source.length; i += 1) {
      hash ^= source.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    return hash >>> 0;
  }

  function boomHashInt(hash, value) {
    let n = Number(value || 0) | 0;
    for (let i = 0; i < 4; i += 1) {
      hash ^= (n >>> (i * 8)) & 0xff;
      hash = Math.imul(hash, 16777619);
    }
    return hash >>> 0;
  }

  function boomHashFloatArray(values, label) {
    let hash = boomHashText(2166136261, label);
    hash = boomHashInt(hash, values?.length || 0);
    for (let i = 0; i < (values?.length || 0); i += 1) {
      hash = boomHashInt(hash, Math.round(Number(values[i] || 0) * 100000));
    }
    return `kasm-${(hash >>> 0).toString(16).padStart(8, "0")}`;
  }

  function boomGeometryHash(geometry) {
    if (!geometry?.pos?.length) return "kasm-empty";
    if (geometry.kasmHash) return geometry.kasmHash;
    const posHash = boomHashFloatArray(geometry.pos, "pos");
    const nrmHash = boomHashFloatArray(geometry.nrm || [], "nrm");
    const hash = kasmHashString(`geometry|${geometry.count || geometry.pos.length / 3}|${geometry.faceCount || geometry.pos.length / 9}|${posHash}|${nrmHash}`);
    try {
      Object.defineProperty(geometry, "kasmHash", {
        value: hash,
        configurable: true,
        enumerable: false,
      });
    } catch (_) {
      geometry.kasmHash = hash;
    }
    return hash;
  }

  function boomModifierCachePayload(modifier) {
    return {
      type: modifier?.type || "",
      enabled: modifier?.enabled !== false,
      params: serializeModifierParams(modifier),
    };
  }

  function boomModifierStackHash(modifiers) {
    return kasmHashString(`modifier-stack|${stableBoomStringify((modifiers || []).map(boomModifierCachePayload))}`);
  }

  function downloadBoomTextFile(fileName, text, type = "application/json") {
    const blob = new Blob([text], { type });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = fileName;
    document.body.appendChild(anchor);
    anchor.click();
    anchor.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1500);
  }

  function boomContractRoot() {
    return document.querySelector("#alphaSection") || document.body;
  }

  function boomElementVisible(el) {
    if (!(el instanceof HTMLElement)) return false;
    if (el.hidden) return false;
    if (el.getAttribute("aria-hidden") === "true") return false;
    const rect = el.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }

  function boomElementLabel(el) {
    const aria = el.getAttribute("aria-label");
    const title = el.getAttribute("title");
    const placeholder = "placeholder" in el ? el.placeholder : "";
    const value = "value" in el && typeof el.value === "string" ? el.value : "";
    const text = el.textContent?.replace(/\s+/g, " ").trim() || "";
    return aria || title || placeholder || text || value || el.id || el.name || el.className || el.tagName.toLowerCase();
  }

  function boomElementPath(el, root = boomContractRoot()) {
    const parts = [];
    let current = el;
    while (current && current !== root && current instanceof HTMLElement) {
      const id = current.id ? `#${current.id}` : "";
      const name = current.getAttribute("name") ? `[name="${current.getAttribute("name")}"]` : "";
      const cls = typeof current.className === "string"
        ? current.className.trim().split(/\s+/).filter(Boolean).slice(0, 2).map((entry) => `.${entry}`).join("")
        : "";
      const parent = current.parentElement;
      const index = parent ? Array.from(parent.children).filter((entry) => entry.tagName === current.tagName).indexOf(current) : 0;
      parts.unshift(`${current.tagName.toLowerCase()}${id}${name}${cls}:nth(${index})`);
      current = parent;
    }
    return parts.join(" > ");
  }

  function boomControlMcpTool(el) {
    const dataset = el.dataset || {};
    if (dataset.action === "set-edit-mode") return "boom.viewport.set_edit_mode";
    if (dataset.action === "select") return "boom.scene.select_item";
    if (dataset.action === "toggle-visible") return "boom.scene.toggle_visibility";
    if (dataset.action === "toggle-selectable") return "boom.scene.toggle_selectability";
    if (dataset.action === "toggle-renderable") return "boom.scene.toggle_renderability";
    if (dataset.action === "toggle-collection") return "boom.outliner.toggle_collection";
    if (dataset.action === "tab") return "boom.inspector.select_tab";
    if (dataset.action === "slicer-mode") return "boom.slicer.set_mode";
    if (dataset.action === "slicer-level") return "boom.slicer.set_level";
    if (dataset.action === "slicer-workflow") return "boom.slicer.set_workflow";
    if (dataset.action === "modifier-add") return "boom.modifier.add";
    if (dataset.action === "modifier-toggle") return "boom.modifier.toggle";
    if (dataset.action === "modifier-expand") return "boom.modifier.expand";
    if (dataset.action === "modifier-up" || dataset.action === "modifier-down") return "boom.modifier.reorder";
    if (dataset.action === "modifier-remove") return "boom.modifier.remove";
    if (dataset.modifierField) return "boom.modifier.set_param";
    if (dataset.slicerField) return "boom.slicer.set_param";
    if (dataset.field) return "boom.transform.set_param";
    if (el.id === "bangerBoomBtn") return "boom.overlay.toggle";
    if (el.id === "bangerExitBtn") return "boom.overlay.close";
    return "boom.ui.invoke_control";
  }

  function boomControlPayload(el) {
    const dataset = { ...(el.dataset || {}) };
    const payload = {
      tag: el.tagName.toLowerCase(),
      type: "type" in el ? el.type || null : null,
      path: boomElementPath(el),
      label: boomElementLabel(el),
      dataset,
    };
    if ("value" in el && typeof el.value === "string") payload.value = el.value;
    if ("checked" in el && typeof el.checked === "boolean") payload.checked = el.checked;
    if (el.getAttribute("role")) payload.role = el.getAttribute("role");
    if (el.id) payload.id = el.id;
    if (el.getAttribute("name")) payload.name = el.getAttribute("name");
    return payload;
  }

  function syncBoomInteractionContract() {
    const root = boomContractRoot();
    if (!root) return null;
    const controls = [];
    const nodes = root.querySelectorAll('button, input, select, textarea, [role="button"]');
    for (const node of nodes) {
      if (!(node instanceof HTMLElement)) continue;
      if (!boomElementVisible(node)) continue;
      const payload = boomControlPayload(node);
      const tool = boomControlMcpTool(node);
      const control = {
        hash: kasmHashString(`ui-control|${tool}|${stableBoomStringify(payload)}`),
        tool,
        payload,
      };
      node.dataset.kasmHash = control.hash;
      node.dataset.boomMcpTool = tool;
      controls.push(control);
    }
    boomUiContract = {
      kind: "boom-ui-contract",
      version: 1,
      hash: kasmHashString(`ui-contract|${controls.map((entry) => entry.hash).join("|")}`),
      controls,
    };
    if (typeof window !== "undefined") {
      window.__forgeBoomUiContract = boomUiContract;
      window.__forgeBoomCommandCatalog = controls;
      window.__forgeBoomResolveControlHash = (hash) => controls.find((entry) => entry.hash === hash) || null;
      window.__forgeBoomConsoleContext = buildBoomConsoleContext;
      window.__forgeBoomExecuteTool = executeBoomTool;
    }
    return boomUiContract;
  }

  function boomImportedMeshLabel(name) {
    const raw = String(name || "Imported mesh").trim();
    if (!raw) return "Imported mesh";
    const cleaned = raw.replace(/^.*[\\/]/, "");
    return cleaned || raw;
  }

  function isBoomMeshItem(item) {
    return item?.type === "mesh";
  }

  function ensureBoomItemModifiers(item) {
    if (!item) return [];
    if (!Array.isArray(item.modifiers)) item.modifiers = [];
    return item.modifiers;
  }

  function modifierAxisIndex(axis) {
    return axis === "Y" ? 1 : axis === "Z" ? 2 : 0;
  }

  function clearBoomComponentSelection() {
    boomScene.componentSelection = null;
  }

  function setBoomComponentSelection(selection) {
    boomScene.componentSelection = selection || null;
  }

  function boomModifierTitle(modifier) {
    if (!modifier) return "Modifier";
    if (modifier.type === "mirror") return `Mirror ${modifier.axis || "X"}`;
    if (modifier.type === "array") return `Array ${modifier.axis || "X"}`;
    if (modifier.type === "inflate") return "Inflate";
    if (modifier.type === "bevel") return "Bevel";
    if (modifier.type === "subdivide") return "Subdivide";
    if (modifier.type === "solidify") return "Solidify";
    return modifier.title || modifier.type || "Modifier";
  }

  function boomModifierMeta(modifier) {
    if (!modifier) return "";
    if (modifier.type === "mirror") return "Live symmetry around object origin";
    if (modifier.type === "array") return `${Math.max(1, Number(modifier.count || 1))} copies · ${Number(modifier.offset || 0).toFixed(2)} step`;
    if (modifier.type === "inflate") return `${Number((((Number(modifier.amount || 1) - 1) * 100))).toFixed(0)}% volume boost`;
    if (modifier.type === "bevel") return `${Number(modifier.width || 0).toFixed(2)} face chamfer preview`;
    if (modifier.type === "subdivide") return `${Math.max(1, Number(modifier.levels || 1))} refinement level`;
    if (modifier.type === "solidify") return `${Number(modifier.thickness || 0).toFixed(2)} shell thickness`;
    return "";
  }

  function createBoomModifier(preset) {
    const type = String(preset?.type || "").trim().toLowerCase();
    const modifier = {
      id: `modifier-${++boomModifierSeq}`,
      type,
      enabled: true,
      expanded: true,
    };
    if (type === "mirror") {
      modifier.axis = preset?.axis || "X";
    } else if (type === "array") {
      modifier.axis = preset?.axis || "X";
      modifier.count = Math.max(2, Number(preset?.count || 3));
      modifier.offset = Number(preset?.offset || 2.25);
    } else if (type === "inflate") {
      modifier.amount = Number(preset?.amount || 1.08);
    } else if (type === "bevel") {
      modifier.width = Number(preset?.width || 0.14);
    } else if (type === "subdivide") {
      modifier.levels = Math.max(1, Math.min(3, Math.round(Number(preset?.levels || 1))));
    } else if (type === "solidify") {
      modifier.thickness = Number(preset?.thickness || 0.2);
    }
    modifier.title = boomModifierTitle(modifier);
    return modifier;
  }

  function moveBoomModifier(item, modifierId, delta) {
    const modifiers = ensureBoomItemModifiers(item);
    const index = modifiers.findIndex((modifier) => modifier.id === modifierId);
    if (index < 0) return false;
    const nextIndex = index + delta;
    if (nextIndex < 0 || nextIndex >= modifiers.length) return false;
    const [entry] = modifiers.splice(index, 1);
    modifiers.splice(nextIndex, 0, entry);
    return true;
  }

  function activeBoomMeshItem() {
    const active = activeBoomItem();
    if (isBoomMeshItem(active)) return active;
    return findBoomItem("imported-mesh");
  }

  function activeBoomComponentSelection() {
    return boomScene.componentSelection || null;
  }

  function boomKasmVertexMap(graph = boomKasmGraph) {
    if (!graph) return new Map();
    if (graph._vertexMap) return graph._vertexMap;
    const map = new Map((graph.vertices || []).map((vertex) => [vertex.id, vertex]));
    try {
      Object.defineProperty(graph, "_vertexMap", { value: map, configurable: true, enumerable: false });
    } catch (_) {
      graph._vertexMap = map;
    }
    return map;
  }

  function boomComponentSummary(selection = activeBoomComponentSelection(), graph = boomKasmGraph) {
    if (!selection || !graph) return null;
    const vertexMap = boomKasmVertexMap(graph);
    if (selection.type === "vertex") {
      const vertex = graph.vertices.find((entry) => entry.id === selection.nodeId);
      const coord = graph.coordinates?.find((entry) => entry.id === vertex?.coordinate);
      if (!vertex) return null;
      return {
        title: `Vertex ${selection.index + 1}`,
        subtitle: `Instance ${selection.passIndex + 1}`,
        hash: vertex.hash,
        coordHash: coord?.hash || vertex.coordinateHash || "",
        cellHashes: [...(vertex.cellHashes || [])],
        details: [
          ["Position", vertex.position.map((value) => Number(value).toFixed(3)).join(", ")],
          ["Connected", `${selection.linkCount || 0} links`],
          ["Cell 1", coord?.cells?.[0]?.index?.join(", ") || "0, 0, 0"],
        ],
      };
    }
    if (selection.type === "edge") {
      const edge = graph.edges.find((entry) => entry.id === selection.nodeId);
      if (!edge) return null;
      const verts = (edge.vertices || []).map((id) => vertexMap.get(id)?.position || [0, 0, 0]);
      const length = verts.length === 2
        ? Math.hypot(
            verts[1][0] - verts[0][0],
            verts[1][1] - verts[0][1],
            verts[1][2] - verts[0][2]
          )
        : 0;
      return {
        title: `Edge ${selection.index + 1}`,
        subtitle: `Instance ${selection.passIndex + 1}`,
        hash: edge.hash,
        cellHashes: [...(edge.cellHashes || [])],
        details: [
          ["Length", length.toFixed(3)],
          ["Faces", `${edge.faces?.length || 0}`],
        ],
      };
    }
    if (selection.type === "face") {
      const face = graph.faces.find((entry) => entry.id === selection.nodeId);
      if (!face) return null;
      return {
        title: `Face ${selection.index + 1}`,
        subtitle: `Instance ${selection.passIndex + 1}`,
        hash: face.hash,
        cellHashes: [...(face.cellHashes || [])],
        details: [
          ["Vertices", `${face.vertices?.length || 0}`],
          ["Edges", `${face.edges?.length || 0}`],
        ],
      };
    }
    return null;
  }

  function boomRegionSummary(region = activeBoomRegionSelection()) {
    if (!region) return null;
    return {
      title: region.sourceType === "slicer-layer"
        ? `Layer ${Number(region.layerIndex || 0) + 1}`
        : region.sourceType === "volume-region"
          ? "Volume region"
          : "Spatial region",
      hash: region.hash,
      geonodeSeedHash: region.geonodeSeedHash,
      details: [
        ["Cells", `${region.cellHashes?.length || 0}`],
        ["Vertices", `${region.vertexIds?.length || 0}`],
        ["Faces", `${region.faceIds?.length || 0}`],
      ],
      bounds: region.bounds,
    };
  }

  function currentBoomAnimationSummary() {
    if (!boomAnimationState?.clip) return null;
    return {
      name: boomAnimationState.clip.name || boomAnimationState.sourceName || "BOOM animation",
      durationMs: Number(boomAnimationState.clip.durationMs || 0),
      trackCount: Array.isArray(boomAnimationState.clip.tracks) ? boomAnimationState.clip.tracks.length : 0,
      format: boomAnimationState.clip.format || "boom_animation_v1",
      playing: boomAnimationState.playing !== false,
      sourceName: boomAnimationState.sourceName || "",
    };
  }

  function boomAnimationAxisIndex(axis) {
    return axis === "y" ? 1 : axis === "z" ? 2 : 0;
  }

  function cloneBoomTransform(transform = {}) {
    return {
      location: [...(transform.location || [0, 0, 0])],
      rotation: [...(transform.rotation || [0, 0, 0])],
      scale: [...(transform.scale || [1, 1, 1])],
      mode: transform.mode || "XYZ Euler",
    };
  }

  function defaultBoomAnimationTracks(transform = activeBoomMeshItem()?.transform || { location: [0, 0, 0], rotation: [0, 0, 0], scale: [1, 1, 1] }) {
    const location = transform.location || [0, 0, 0];
    const rotation = transform.rotation || [0, 0, 0];
    return [
      {
        target: "imported-mesh",
        property: "rotation",
        axis: "z",
        keyframes: [
          { time: 0, value: Number(rotation[2] || 0) },
          { time: 1, value: Number((rotation[2] || 0) + 360) },
        ],
      },
      {
        target: "imported-mesh",
        property: "location",
        axis: "z",
        keyframes: [
          { time: 0, value: Number(location[2] || 0) },
          { time: 0.5, value: Number((location[2] || 0) + 0.75) },
          { time: 1, value: Number(location[2] || 0) },
        ],
      },
    ];
  }

  function buildBoomAnimationPayload() {
    const item = activeBoomMeshItem();
    if (!sceneMesh?.base?.pos?.length || !sceneMesh?.base?.nrm?.length || !item) return null;
    const activeAnimation = boomAnimationState?.clip || null;
    return {
      format: "boom_animation_v1",
      exportedAt: new Date().toISOString(),
      scene: {
        name: item.name || "BOOM mesh",
        mesh: {
          positions: Array.from(sceneMesh.base.pos || []),
          normals: Array.from(sceneMesh.base.nrm || []),
          vertexCount: Number(sceneMesh.base.count || sceneMesh.count || 0),
          faceCount: Number(sceneMesh.base.faceCount || sceneMesh.faceCount || 0),
        },
        transform: cloneBoomTransform(item.transform),
        modifiers: ensureBoomItemModifiers(item).map((modifier) => stableBoomValue(modifier)),
        animation: activeAnimation || {
          name: `${item.name || "BOOM"} bridge`,
          durationMs: 4000,
          loop: true,
          autoPlay: true,
          tracks: defaultBoomAnimationTracks(item.transform),
        },
      },
    };
  }

  function buildBoomAnimationJsSource(payload) {
    const json = JSON.stringify(payload, null, 2);
    return [
      "/* BOOM_ANIMATION_JSON_START",
      json,
      "BOOM_ANIMATION_JSON_END */",
      "",
      "const boomAnimation = JSON.parse(String.raw`" + json.replace(/`/g, "\\`").replace(/\$\{/g, "\\${") + "`);",
      "if (typeof window !== 'undefined') window.BoomAnimation = boomAnimation;",
      "if (typeof module !== 'undefined' && module.exports) module.exports = boomAnimation;",
      "",
      "// Import this file back into BOOM with drag-and-drop or the native file picker.",
      "",
    ].join("\n");
  }

  function exportBoomAnimationBridge(kind = "js") {
    const payload = buildBoomAnimationPayload();
    if (!payload) return null;
    const baseName = (payload.scene?.name || "boom-animation")
      .replace(/[^\w.-]+/g, "_")
      .replace(/^_+|_+$/g, "") || "boom-animation";
    if (kind === "json") {
      downloadBoomTextFile(`${baseName}.boom.json`, JSON.stringify(payload, null, 2), "application/json");
      return { kind: "json", fileName: `${baseName}.boom.json`, payload };
    }
    const js = buildBoomAnimationJsSource(payload);
    downloadBoomTextFile(`${baseName}.boom.js`, js, "text/javascript");
    return { kind: "js", fileName: `${baseName}.boom.js`, payload };
  }

  function normalizeBoomAnimationTrack(track) {
    if (!track || typeof track !== "object") return null;
    const property = ["location", "rotation", "scale"].includes(String(track.property || ""))
      ? String(track.property)
      : null;
    if (!property) return null;
    const axis = ["x", "y", "z"].includes(String(track.axis || "").toLowerCase())
      ? String(track.axis).toLowerCase()
      : "x";
    const keyframes = Array.isArray(track.keyframes)
      ? track.keyframes
          .map((frame) => ({
            time: Math.max(0, Math.min(1, Number(frame?.time ?? 0))),
            value: Number(frame?.value ?? 0),
          }))
          .filter((frame) => Number.isFinite(frame.time) && Number.isFinite(frame.value))
          .sort((a, b) => a.time - b.time)
      : [];
    if (!keyframes.length) return null;
    return {
      target: String(track.target || "imported-mesh"),
      property,
      axis,
      keyframes,
    };
  }

  function normalizeBoomAnimationPayload(raw, sourceName = "") {
    const scene = raw?.scene;
    const mesh = scene?.mesh;
    if (!scene || !mesh || !Array.isArray(mesh.positions) || !Array.isArray(mesh.normals)) return null;
    const pos = new Float32Array(mesh.positions.map((value) => Number(value) || 0));
    const nrm = new Float32Array(mesh.normals.map((value) => Number(value) || 0));
    if (!pos.length || !nrm.length || pos.length !== nrm.length) return null;
    const tracks = Array.isArray(scene?.animation?.tracks)
      ? scene.animation.tracks.map(normalizeBoomAnimationTrack).filter(Boolean)
      : [];
    return {
      meshData: {
        pos,
        nrm,
        count: Number(mesh.vertexCount || pos.length / 3),
        faceCount: Number(mesh.faceCount || (pos.length / 9)),
      },
      name: String(scene.name || sourceName || "Imported animation"),
      transform: cloneBoomTransform(scene.transform || {}),
      modifiers: Array.isArray(scene.modifiers) ? scene.modifiers.map((entry) => stableBoomValue(entry)) : [],
      animation: {
        format: String(raw.format || "boom_animation_v1"),
        name: String(scene?.animation?.name || scene.name || sourceName || "BOOM animation"),
        durationMs: Math.max(250, Number(scene?.animation?.durationMs || 4000)),
        loop: scene?.animation?.loop !== false,
        autoPlay: scene?.animation?.autoPlay !== false,
        tracks,
      },
    };
  }

  function extractBoomAnimationJsonFromJs(text) {
    const source = String(text || "");
    const markerStart = "BOOM_ANIMATION_JSON_START";
    const markerEnd = "BOOM_ANIMATION_JSON_END";
    const startIndex = source.indexOf(markerStart);
    const endIndex = source.indexOf(markerEnd);
    if (startIndex < 0 || endIndex < 0 || endIndex <= startIndex) return null;
    const json = source.slice(startIndex + markerStart.length, endIndex).replace(/^\s*\n?/, "").replace(/\n?\s*$/, "");
    return json.trim();
  }

  async function parseBoomAnimationFile(file) {
    const lower = String(file?.name || "").toLowerCase();
    if (!(lower.endsWith(".json") || lower.endsWith(".js"))) return null;
    const text = await file.text();
    let payloadText = text;
    if (lower.endsWith(".js")) {
      payloadText = extractBoomAnimationJsonFromJs(text) || "";
    }
    if (!payloadText) return null;
    let raw = null;
    try {
      raw = JSON.parse(payloadText);
    } catch (_) {
      return null;
    }
    return normalizeBoomAnimationPayload(raw, file.name);
  }

  function clearBoomAnimationState() {
    boomAnimationState = null;
  }

  function setBoomAnimationState(animation, sourceName = "") {
    if (!animation?.tracks?.length) {
      boomAnimationState = animation ? {
        clip: animation,
        sourceName,
        playing: false,
        loop: animation.loop !== false,
        durationMs: animation.durationMs || 4000,
        startedAtMs: 0,
      } : null;
      return boomAnimationState;
    }
    boomAnimationState = {
      clip: animation,
      sourceName,
      playing: animation.autoPlay !== false,
      loop: animation.loop !== false,
      durationMs: Math.max(250, Number(animation.durationMs || 4000)),
      startedAtMs: 0,
    };
    return boomAnimationState;
  }

  function sampleBoomAnimationTrack(track, normalizedTime) {
    const frames = track?.keyframes || [];
    if (!frames.length) return 0;
    if (normalizedTime <= frames[0].time) return frames[0].value;
    for (let i = 1; i < frames.length; i += 1) {
      const prev = frames[i - 1];
      const next = frames[i];
      if (normalizedTime <= next.time) {
        const span = Math.max(0.000001, next.time - prev.time);
        const mix = (normalizedTime - prev.time) / span;
        return prev.value + (next.value - prev.value) * mix;
      }
    }
    return frames[frames.length - 1].value;
  }

  function applyBoomAnimationFrame(ts) {
    if (!boomAnimationState?.clip || !boomAnimationState.playing) return;
    const item = findBoomItem("imported-mesh");
    if (!item?.transform) return;
    if (!boomAnimationState.startedAtMs) boomAnimationState.startedAtMs = ts;
    const durationMs = Math.max(250, Number(boomAnimationState.durationMs || 4000));
    let elapsed = ts - boomAnimationState.startedAtMs;
    if (boomAnimationState.loop) elapsed = elapsed % durationMs;
    else elapsed = Math.min(durationMs, elapsed);
    const normalizedTime = durationMs > 0 ? elapsed / durationMs : 0;
    for (const track of boomAnimationState.clip.tracks || []) {
      if ((track.target || "imported-mesh") !== "imported-mesh") continue;
      const property = track.property;
      const axisIndex = boomAnimationAxisIndex(track.axis);
      if (!Array.isArray(item.transform[property])) continue;
      item.transform[property][axisIndex] = Number(sampleBoomAnimationTrack(track, normalizedTime).toFixed(property === "rotation" ? 1 : 3));
    }
    if (!boomAnimationState.loop && elapsed >= durationMs) {
      boomAnimationState.playing = false;
      boomRenderContinuousUntil = 0;
      renderBoomSidebar();
    }
  }

  function buildBoomConsoleContext() {
    const activeItem = activeBoomItem();
    const componentSummary = boomComponentSummary();
    const regionSelection = activeBoomRegionSelection();
    const regionSummary = boomRegionSummary(regionSelection);
    return {
      active: isViewVisible(),
      workspaceMode: boomScene.workspaceMode,
      propertyTab: boomScene.propertyTab,
      editMode: boomScene.editMode,
      activeItem: activeItem ? {
        id: activeItem.id,
        name: activeItem.name,
        type: activeItem.type,
      } : null,
      selection: componentSummary ? {
        title: componentSummary.title,
        subtitle: componentSummary.subtitle,
        hash: componentSummary.hash,
        coordHash: componentSummary.coordHash || "",
        cellCount: componentSummary.cellHashes?.length || 0,
      } : null,
      region: regionSelection ? {
        title: regionSummary?.title || "Spatial region",
        hash: regionSelection.hash,
        geonodeSeedHash: regionSelection.geonodeSeedHash || "",
        cellCount: regionSelection.cellHashes?.length || 0,
        vertexCount: regionSelection.vertexIds?.length || 0,
        faceCount: regionSelection.faceIds?.length || 0,
      } : null,
      kasm: boomKasmGraph ? {
        objectHash: boomKasmGraph.object?.hash || "",
        cellCount: boomKasmGraph.cells?.length || 0,
        coordinateCount: boomKasmGraph.coordinates?.length || 0,
        vertexCount: boomKasmGraph.vertices?.length || 0,
        edgeCount: boomKasmGraph.edges?.length || 0,
        faceCount: boomKasmGraph.faces?.length || 0,
        modifierCount: boomKasmGraph.modifiers?.length || 0,
      } : null,
      commandCatalogSize: boomUiContract?.controls?.length || 0,
      uiContractHash: boomUiContract?.hash || "",
    };
  }

  function boomModifierPresetByType(type) {
    const normalized = String(type || "").trim().toLowerCase();
    return BOOM_MODIFIER_PRESETS.find((entry) => entry.type === normalized) || null;
  }

  function boomApplyModifierPayload(modifier, payload = {}) {
    if (!modifier || !payload || typeof payload !== "object") return modifier;
    if (payload.axis && typeof payload.axis === "string") {
      modifier.axis = payload.axis.toUpperCase().slice(0, 1) || modifier.axis;
    }
    if (payload.count != null) {
      modifier.count = Math.max(2, Math.min(6, Math.round(Number(payload.count) || modifier.count || 2)));
    }
    if (payload.offset != null) {
      modifier.offset = Number(Math.max(-99, Math.min(99, Number(payload.offset) || 0)).toFixed(2));
    }
    if (payload.amount != null) {
      modifier.amount = Number(Math.max(0.1, Math.min(8, Number(payload.amount) || 1)).toFixed(2));
    }
    if (payload.width != null) {
      modifier.width = Number(Math.max(0.02, Math.min(0.42, Number(payload.width) || 0.02)).toFixed(2));
    }
    if (payload.levels != null) {
      modifier.levels = Math.max(1, Math.min(3, Math.round(Number(payload.levels) || 1)));
    }
    if (payload.thickness != null) {
      modifier.thickness = Number(Math.max(0.02, Math.min(0.7, Number(payload.thickness) || 0.02)).toFixed(2));
    }
    modifier.title = boomModifierTitle(modifier);
    return modifier;
  }

  function boomToolResult(tool, ok, detail = {}) {
    return {
      ok,
      tool,
      detail,
      context: buildBoomConsoleContext(),
    };
  }

  function boomKasmObjectHash(label, value) {
    return kasmHashString(`${label}|${stableBoomStringify(value)}`);
  }

  function boomKasmCurrentSceneHash() {
    return boomKasmObjectHash("scene-snapshot-v1", {
      activeId: boomScene.activeId,
      workspaceMode: boomScene.workspaceMode,
      propertyTab: boomScene.propertyTab,
      editMode: boomScene.editMode,
      meshHash: sceneMesh ? boomGeometryHash(sceneMesh.display?.pos?.length ? sceneMesh.display : sceneMesh) : "none",
      slicerWorkflow: boomScene.slicer?.workflow || "",
      componentHash: boomScene.componentSelection?.nodeHash || "",
      regionHash: boomScene.regionSelection?.hash || "",
      items: (boomScene.items || []).map((item) => ({
        id: item.id,
        type: item.type,
        visible: item.visible !== false,
        renderable: item.renderable !== false,
        selectable: item.selectable !== false,
        transform: item.transform || null,
        modifiers: isBoomMeshItem(item) ? ensureBoomItemModifiers(item).map(boomModifierCachePayload) : [],
      })),
    });
  }

  function buildBoomKasmCommandSpec(command, payload = {}, options = {}) {
    const normalizedCommand = String(command || "boom.unknown").trim() || "boom.unknown";
    const stablePayload = stableBoomValue(payload || {});
    const sceneHash = boomKasmCurrentSceneHash();
    const payloadHash = boomKasmObjectHash("command-payload-v1", stablePayload);
    const permissions = stableBoomValue(options.permissions || {
      world: true,
      assets: normalizedCommand.startsWith("boom.animation.export"),
      filesystem: false,
      shell: false,
      network: false,
      renderer: normalizedCommand.startsWith("boom.viewport") || normalizedCommand.startsWith("boom.slicer"),
    });
    const budget = stableBoomValue(options.budget || {
      frameMs: 16.667,
      interactionMs: 50,
      ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
      vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
    });
    const permissionsHash = boomKasmObjectHash("permissions-v1", permissions);
    const budgetHash = boomKasmObjectHash("resource-budget-v1", budget);
    const commandSpec = {
      kind: "kasm-command-spec",
      version: 1,
      rawInput: String(options.rawInput || normalizedCommand),
      modelSource: options.modelSource || null,
      command: {
        type: normalizedCommand,
        payload: stablePayload,
      },
      permissions,
      budget,
      sceneHash,
      inputHashes: [sceneHash, payloadHash],
      payloadHash,
      permissionsHash,
      budgetHash,
    };
    commandSpec.id = boomKasmObjectHash("command-spec-v1", commandSpec);
    return commandSpec;
  }

  function buildBoomKasmBytecodeProgram(commandSpec) {
    const program = {
      kind: "kasm-bytecode-program",
      version: 1,
      commandSpecHash: commandSpec.id,
      opcode: commandSpec.command.type,
      inputSchemaHash: boomKasmObjectHash("input-schema-v1", Object.keys(commandSpec.command.payload || {}).sort()),
      outputSchemaHash: boomKasmObjectHash("output-schema-v1", ["ok", "detail", "context"]),
      deterministic: true,
    };
    program.id = boomKasmObjectHash("bytecode-program-v1", program);
    return program;
  }

  function buildBoomKasmSandboxMatrix(commandSpec) {
    const matrix = {
      kind: "kasm-sandbox-matrix",
      version: 1,
      commandSpecHash: commandSpec.id,
      permissionsHash: commandSpec.permissionsHash,
      budgetHash: commandSpec.budgetHash,
      lanes: {
        llmDirectFilesystem: false,
        llmDirectShell: false,
        llmDirectRenderer: false,
        mcpDirectExternalTool: false,
        kasmWorldPatchOnly: true,
      },
    };
    matrix.id = boomKasmObjectHash("sandbox-matrix-v1", matrix);
    return matrix;
  }

  function rememberBoomKasmRecord(list, record, limit = BOOM_KASM_RUN_HISTORY_LIMIT) {
    rememberBoomKasmHash(record, record?.kind || "kasm-record");
    list.push(record);
    while (list.length > limit) list.shift();
    return record;
  }

  function rememberBoomKasmMetricRecord(list, record) {
    rememberBoomKasmHash(record, record?.kind || "kasm-metric-record");
    list.push(record);
    while (list.length > BOOM_KASM_METRIC_HISTORY_LIMIT) list.shift();
    return record;
  }

  function rememberBoomKasmHash(record, role = "kasm-object") {
    const id = String(record?.id || record?.hash || "");
    if (!id) return record;
    if (!boomKasmHashIndex.has(id)) boomKasmHashIndexOrder.push(id);
    boomKasmHashIndex.set(id, {
      kind: "kasm-hash-index-entry",
      version: 1,
      role,
      id,
      record,
      rememberedAtMs: Number(boomNowMs().toFixed(3)),
    });
    while (boomKasmHashIndexOrder.length > BOOM_KASM_HASH_INDEX_LIMIT) {
      const evicted = boomKasmHashIndexOrder.shift();
      if (evicted && evicted !== id) boomKasmHashIndex.delete(evicted);
    }
    return record;
  }

  function compactBoomKasmRecord(record) {
    if (!record || typeof record !== "object") return null;
    return {
      id: record.id || record.hash || "",
      kind: record.kind || "kasm-object",
      name: record.name || record.programName || record.skillName || record.metricName || record.label || record.pageKind || record.action || "",
      commandHash: record.commandHash || record.commandSpecHash || "",
      programHash: record.programHash || "",
      computeProgramHash: record.computeProgramHash || "",
      bytecodeHash: record.bytecodeHash || "",
      shaderHash: record.shaderHash || "",
      proofHash: record.proofHash || "",
      status: record.status || record.residency || "",
      pageKind: record.pageKind || "",
      residency: record.residency || "",
      outputHashes: Array.isArray(record.outputHashes) ? record.outputHashes.slice(0, 8) : [],
      metricHashes: Array.isArray(record.metricHashes) ? record.metricHashes.slice(0, 8) : [],
    };
  }

  function ensureBoomKasmTemplateCatalog() {
    if (boomKasmTemplateHistory.length === BOOM_KASM_TEMPLATE_CATALOG.length) return boomKasmTemplateHistory;
    boomKasmTemplateHistory = BOOM_KASM_TEMPLATE_CATALOG.map((name) => {
      const target = name.split(".")[1] || "program";
      const template = {
        kind: "kasm-template-spec",
        version: 1,
        name,
        target,
        sourceHash: boomKasmObjectHash("template-source-v1", { name }),
        bytecodeHash: boomKasmObjectHash("template-bytecode-v1", { name, compilesTo: "kasm-bytecode" }),
        inputSchemaHash: boomKasmObjectHash("template-input-schema-v1", { name, target }),
        outputSchemaHash: boomKasmObjectHash("template-output-schema-v1", { name, output: `${target}_hash` }),
        permissionHash: boomKasmObjectHash("template-permissions-v1", {
          filesystem: false,
          shell: false,
          network: false,
          worldPatchOnly: target !== "metric",
        }),
      };
      template.id = boomKasmObjectHash("template-spec-v1", template);
      rememberBoomKasmHash(template, "template-spec");
      return template;
    });
    boomKasmSpineStats.templates = boomKasmTemplateHistory.length;
    return boomKasmTemplateHistory;
  }

  function buildBoomKasmGraphProjection() {
    const templates = ensureBoomKasmTemplateCatalog();
    const sceneHash = boomKasmCurrentSceneHash();
    const latestProofHash = boomKasmProofHistory[boomKasmProofHistory.length - 1]?.id || "";
    const sceneRecord = {
      kind: "kasm-scene-snapshot",
      version: 1,
      id: sceneHash,
      name: "SceneHash",
      status: "live",
      proofHash: latestProofHash,
      outputHashes: [sceneHash],
    };
    rememberBoomKasmHash(sceneRecord, "scene-hash");
    const projection = {
      kind: "kasm-graph-projection",
      version: 1,
      sceneHash,
      views: {
        world: [
          compactBoomKasmRecord(sceneRecord),
          ...boomKasmPatchHistory.slice(-12).map(compactBoomKasmRecord),
          ...boomKasmRollbackHistory.slice(-12).map(compactBoomKasmRecord),
        ].filter(Boolean),
        assets: [
          ...boomKasmGeoClusterHistory.slice(-16).map(compactBoomKasmRecord),
          ...boomKasmAssetResidencyHistory.slice(-16).map(compactBoomKasmRecord),
          ...boomKasmAssetPageHistory.slice(-24).map(compactBoomKasmRecord),
          ...boomKasmRenderHistory.slice(-8).map(compactBoomKasmRecord),
        ].filter(Boolean),
        skills: [
          ...boomKasmSkillHistory.slice(-16).map(compactBoomKasmRecord),
          ...templates.filter((template) => template.target === "skill").slice(0, 6).map(compactBoomKasmRecord),
        ].filter(Boolean),
        programs: [
          ...boomKasmProgramHistory.slice(-20).map(compactBoomKasmRecord),
          ...boomKasmComputeHistory.slice(-12).map(compactBoomKasmRecord),
          ...templates.filter((template) => !["metric", "skill"].includes(template.target)).slice(0, 10).map(compactBoomKasmRecord),
        ].filter(Boolean),
        runs: [
          ...boomKasmRunHistory.slice(-20).map(compactBoomKasmRecord),
          ...boomKasmProofHistory.slice(-8).map(compactBoomKasmRecord),
        ].filter(Boolean),
      },
      templateHashes: templates.map((template) => template.id),
      statsHash: boomKasmObjectHash("graph-projection-stats-v1", boomKasmSpineStats),
    };
    projection.id = boomKasmObjectHash("graph-projection-v1", projection);
    rememberBoomKasmHash(projection, "graph-projection");
    return projection;
  }

  function buildBoomKasmMcpFacade() {
    const templates = ensureBoomKasmTemplateCatalog();
    const tools = BOOM_KASM_MCP_TOOL_CATALOG.map((tool) => ({
      kind: "kasm-mcp-tool",
      name: tool.name,
      slash: tool.slash,
      outputKind: tool.outputKind,
      commandSpecTemplateHash: boomKasmObjectHash("mcp-tool-command-template-v1", tool),
      inputSchemaHash: boomKasmObjectHash("mcp-tool-input-schema-v1", { name: tool.name }),
      outputSchemaHash: boomKasmObjectHash("mcp-tool-output-schema-v1", { outputKind: tool.outputKind }),
    }));
    const resources = BOOM_KASM_MCP_RESOURCE_URIS.map((uri) => ({
      kind: "kasm-mcp-resource",
      uri,
      resourceHash: boomKasmObjectHash("mcp-resource-uri-v1", { uri }),
    }));
    const prompts = BOOM_KASM_MCP_PROMPT_CATALOG.map((name) => ({
      kind: "kasm-mcp-prompt",
      name,
      promptHash: boomKasmObjectHash("mcp-prompt-template-v1", { name }),
    }));
    const facade = {
      kind: "kasm-mcp-facade",
      version: 1,
      tools,
      resources,
      prompts,
      templateHashes: templates.map((template) => template.id),
      sandboxHash: boomKasmObjectHash("mcp-facade-sandbox-v1", {
        directExternalTools: false,
        directFilesystem: false,
        directShell: false,
        allEntriesCompileToCommandSpec: true,
      }),
    };
    facade.id = boomKasmObjectHash("mcp-facade-v1", facade);
    return facade;
  }

  function getBoomKasmMcpFacade() {
    const facade = buildBoomKasmMcpFacade();
    if (!boomKasmMcpFacade || boomKasmMcpFacade.id !== facade.id) {
      boomKasmMcpFacade = facade;
      boomKasmSpineStats.mcpFacades += 1;
      rememberBoomKasmRecord(boomKasmMcpFacadeHistory, facade, BOOM_KASM_MCP_HISTORY_LIMIT);
    }
    return boomKasmMcpFacade;
  }

  function boomMcpToolSlash(toolName, args = {}) {
    const name = String(toolName || "").trim();
    const payload = args && typeof args === "object" ? args : {};
    const text = (value, fallback = "") => String(value == null || value === "" ? fallback : value).trim();
    if (name === "kasm.create_program") return `/create_program ${text(payload.name, "default_program")} --template ${text(payload.template, payload.name || "default_program")}`;
    if (name === "kasm.run_program") return `/program run ${text(payload.program || payload.name || payload.hash, "default_program")} --input ${text(payload.input || payload.target, "last")}`;
    if (name === "kasm.create_metric") return `/create_metric ${text(payload.name, "scene_complexity")} --template ${text(payload.template, payload.name || "scene_complexity")}`;
    if (name === "kasm.run_metric") return `/metric run ${text(payload.metric || payload.name, "scene_complexity")} --target ${text(payload.target || payload.hash, "last")}`;
    if (name === "kasm.run_matrix") return `/matrix run ${text(payload.program || payload.name, "default_program")} --variants ${Math.max(1, Math.min(512, Number(payload.variants || 128)))} --metrics ${text(Array.isArray(payload.metrics) ? payload.metrics.join(",") : payload.metrics, "scene_complexity,draw_call_cost,ram_cache_fill_pct")}`;
    if (name === "kasm.create_skill") return `/skill create ${text(payload.name, "default_skill")} --metrics ${text(Array.isArray(payload.metrics) ? payload.metrics.join(",") : payload.metrics, "scene_complexity,draw_call_cost")}`;
    if (name === "kasm.run_skill") return `/skill run ${text(payload.skill || payload.name, "default_skill")} --target ${text(payload.target || payload.hash, "current")}`;
    if (name === "kasm.promote_skill") return `/skill promote --from ${text(payload.target || payload.hash, "current")} --name ${text(payload.name, "promoted_skill")}`;
    if (name === "kasm.render_frame") return `/render ${text(payload.action, "frame")} ${text(payload.mode, "lit")}`;
    if (name === "kasm.compute_dispatch") return `/program run ${text(payload.program || payload.name, "gpu_cull_instances")} --input ${text(payload.input || payload.target, "current")}`;
    if (name === "kasm.asset_scan") return `/asset ${text(payload.action, "scan")} --root ${text(payload.root, "project")}`;
    if (name === "kasm.asset_residency") return `/asset residency --target ${text(payload.target || payload.hash, "current")}`;
    if (name === "kasm.asset_evict_cold") return `/asset evict_cold --hot-pages ${Math.max(1, Math.min(64, Number(payload.hotPages || 4)))}`;
    if (name === "kasm.asset_pin_hot") return `/asset pin_hot --target ${text(payload.target || payload.hash, "current")} --hot-pages ${Math.max(1, Math.min(64, Number(payload.hotPages || 4)))}`;
    if (name === "kasm.cache_stats") return "/cache stats";
    if (name === "kasm.status") return "/status current_run";
    if (name === "kasm.prove") return `/prove ${text(payload.hash || payload.target, "last")}`;
    if (name === "kasm.explain") return `/explain ${text(payload.hash || payload.target, "last")}`;
    if (name === "kasm.rollback") return `/world rollback ${text(payload.hash || payload.target, "last")}`;
    return "";
  }

  function runBoomKasmMcpTool(toolName, args = {}) {
    const facade = getBoomKasmMcpFacade();
    const slash = boomMcpToolSlash(toolName, args);
    const payload = {
      facadeHash: facade.id,
      toolName: String(toolName || ""),
      args: stableBoomValue(args || {}),
      slash,
    };
    return executeBoomKasmCommandSpec("boom.kasm.mcp_tool", payload, () => {
      if (!slash) return boomToolResult("boom.kasm.mcp_tool", false, { error: "unknown_mcp_tool", facadeHash: facade.id });
      const inner = runBoomSlashCommand(slash, { modelSource: "mcp", facadeHash: facade.id, mcpToolName: toolName });
      boomKasmSpineStats.mcpToolCalls += 1;
      return boomToolResult("boom.kasm.mcp_tool", !!inner?.ok, {
        facadeHash: facade.id,
        toolName,
        slash,
        innerKasm: inner?.kasm || null,
        outputHashes: [inner?.kasm?.runHash, inner?.kasm?.proofHash, inner?.kasm?.outputHash].filter(Boolean),
      });
    }, {
      rawInput: `mcp tool ${toolName}`,
      modelSource: "mcp",
      applyWorldPatch: false,
      permissions: { world: false, assets: false, filesystem: false, shell: false, network: false, renderer: false },
    });
  }

  function buildBoomKasmMcpResource(uri = "kasm://graph") {
    const normalized = String(uri || "kasm://graph").trim();
    const graph = normalized === "kasm://graph" ? buildBoomKasmGraphProjection() : null;
    const payload = normalized === "kasm://templates"
      ? ensureBoomKasmTemplateCatalog().map(compactBoomKasmRecord)
      : normalized === "kasm://programs"
        ? boomKasmProgramHistory.slice(-32).map(compactBoomKasmRecord).filter(Boolean)
        : normalized === "kasm://metrics"
          ? [...boomKasmMetricSpecHistory.slice(-24), ...boomKasmMetricHistory.slice(-24)].map(compactBoomKasmRecord).filter(Boolean)
          : normalized === "kasm://skills"
            ? boomKasmSkillHistory.slice(-24).map(compactBoomKasmRecord).filter(Boolean)
            : normalized === "kasm://runs"
              ? boomKasmRunHistory.slice(-32).map(compactBoomKasmRecord).filter(Boolean)
              : normalized === "kasm://proofs"
                ? boomKasmProofHistory.slice(-32).map(compactBoomKasmRecord).filter(Boolean)
                : normalized === "kasm://assets"
                  ? [...boomKasmAssetResidencyHistory.slice(-20), ...boomKasmGeoClusterHistory.slice(-12), ...boomKasmAssetPageHistory.slice(-40)].map(compactBoomKasmRecord).filter(Boolean)
                  : normalized === "kasm://render"
                    ? boomKasmRenderHistory.slice(-16).map(compactBoomKasmRecord).filter(Boolean)
                    : normalized === "kasm://compute"
                      ? boomKasmComputeHistory.slice(-24).map(compactBoomKasmRecord).filter(Boolean)
                      : normalized === "kasm://status"
                        ? { stats: boomKasmSpineStats, latestRun: compactBoomKasmRecord(boomKasmRunHistory[boomKasmRunHistory.length - 1]) }
                        : graph;
    const resource = {
      kind: "kasm-mcp-resource-read",
      version: 1,
      uri: normalized,
      content: graph || payload || null,
    };
    resource.contentHash = boomKasmObjectHash("mcp-resource-content-v1", resource.content);
    resource.id = boomKasmObjectHash("mcp-resource-read-v1", resource);
    rememberBoomKasmHash(resource, "mcp-resource");
    return resource;
  }

  function readBoomKasmMcpResource(uri = "kasm://graph") {
    const facade = getBoomKasmMcpFacade();
    const normalized = String(uri || "kasm://graph").trim();
    return executeBoomKasmCommandSpec("boom.kasm.mcp_resource", { facadeHash: facade.id, uri: normalized }, () => {
      const resource = buildBoomKasmMcpResource(normalized);
      boomKasmSpineStats.mcpResourceReads += 1;
      return boomToolResult("boom.kasm.mcp_resource", true, {
        facadeHash: facade.id,
        resource,
        outputHashes: [resource.id, resource.contentHash],
      });
    }, {
      rawInput: `mcp resource ${normalized}`,
      modelSource: "mcp",
      applyWorldPatch: false,
      permissions: { world: false, assets: false, filesystem: false, shell: false, network: false, renderer: false },
    });
  }

  function buildBoomKasmMcpPrompt(name = "prompt_to_kasm_program", args = {}) {
    const normalized = String(name || "prompt_to_kasm_program").trim();
    const budget = args?.budget || "fps=60 ram=12gb vram=6gb";
    const promptPlans = {
      prompt_to_kasm_program: [
        `/create_program ${args?.program || "generate_world_patch"} --template ${args?.template || "template.scene.generate_layout"}`,
        "/create_metric vram_cost --template template.metric.vram_cost",
        "/matrix run generate_world_patch --variants 128 --metrics vram_cost,scene_complexity,composition_score",
        "/world patch preview last",
        "/skill promote --from last --name prompt_to_world_patch",
      ],
      matrix_creative_search: [
        `/matrix run ${args?.program || "generate_layout"} --variants ${args?.variants || 256} --metrics ${args?.metrics || "scene_complexity,composition_score,draw_call_cost"}`,
        "/matrix select --top 8 --by score",
      ],
      auto_optimizer: [
        `/skill run optimize_scene --target ${args?.target || "current"} --budget ${budget}`,
        "/metric run vram_cost --target last",
        "/prove last",
      ],
      hash_time_machine: [
        "/world diff last previous",
        `/world rollback ${args?.target || "last"}`,
        "/prove last",
      ],
      asset_brain: [
        `/asset scan --root ${args?.root || "project"}`,
        "/asset dedup",
        "/asset residency",
        "/prove last",
      ],
    };
    const prompt = {
      kind: "kasm-mcp-prompt-read",
      version: 1,
      name: normalized,
      args: stableBoomValue(args || {}),
      slashPlan: promptPlans[normalized] || promptPlans.prompt_to_kasm_program,
      compilesTo: "kasm-command-spec-sequence",
    };
    prompt.id = boomKasmObjectHash("mcp-prompt-read-v1", prompt);
    rememberBoomKasmHash(prompt, "mcp-prompt");
    return prompt;
  }

  function getBoomKasmMcpPrompt(name = "prompt_to_kasm_program", args = {}) {
    const facade = getBoomKasmMcpFacade();
    return executeBoomKasmCommandSpec("boom.kasm.mcp_prompt", {
      facadeHash: facade.id,
      name,
      args: stableBoomValue(args || {}),
    }, () => {
      const prompt = buildBoomKasmMcpPrompt(name, args);
      boomKasmSpineStats.mcpPromptReads += 1;
      return boomToolResult("boom.kasm.mcp_prompt", true, {
        facadeHash: facade.id,
        prompt,
        outputHashes: [prompt.id],
      });
    }, {
      rawInput: `mcp prompt ${name}`,
      modelSource: "mcp",
      applyWorldPatch: false,
      permissions: { world: false, assets: false, filesystem: false, shell: false, network: false, renderer: false },
    });
  }

  function resolveBoomKasmHash(hash) {
    const raw = String(hash || "").trim();
    const target = raw === "last" ? boomKasmRunHistory[boomKasmRunHistory.length - 1]?.id || "" : raw;
    if (!target) return null;
    let entry = boomKasmHashIndex.get(target) || null;
    if (!entry && target.length >= 8) {
      const resolvedKey = boomKasmHashIndexOrder.find((key) => key.startsWith(target));
      entry = resolvedKey ? boomKasmHashIndex.get(resolvedKey) || null : null;
    }
    return entry ? { hash: entry.id, role: entry.role, record: entry.record } : null;
  }

  function explainBoomKasmHash(hash) {
    const resolved = resolveBoomKasmHash(hash);
    if (!resolved) return null;
    const record = resolved.record || {};
    return {
      kind: "kasm-explain-hash",
      version: 1,
      hash: resolved.hash,
      role: resolved.role,
      objectKind: record.kind || "unknown",
      commandHash: record.commandHash || record.commandSpecHash || "",
      programHash: record.programHash || "",
      computeProgramHash: record.computeProgramHash || "",
      sandboxHash: record.sandboxHash || "",
      environmentHash: record.environmentHash || "",
      sourceHash: record.sourceHash || record.source_hash || "",
      bytecodeHash: record.bytecodeHash || record.bytecode_hash || "",
      shaderHash: record.shaderHash || "",
      programGraphHash: record.programGraphHash || "",
      skillHash: record.skillHash || "",
      metricSetHash: record.metricSetHash || "",
      variantCount: record.variantCount || record.variants?.length || 0,
      testSetHash: record.testSetHash || "",
      sceneHash: record.sceneHash || "",
      renderMode: record.renderMode || "",
      dispatch: record.dispatch || null,
      backend: record.backend || "",
      clusterPageHashes: record.clusterPageHashes || [],
      lodTreeHash: record.lodTreeHash || "",
      boundsTreeHash: record.boundsTreeHash || "",
      assetPackHash: record.assetPackHash || "",
      assetStoreHash: record.assetStoreHash || "",
      residencyHash: record.residencyHash || "",
      pageHashes: record.pageHashes || [],
      pageCount: record.pageCount || 0,
      assetPageHashes: record.assetPageHashes || record.pageHashes || [],
      residency: record.residency || "",
      action: record.action || "",
      proofHash: record.proofHash || "",
      metricSpecHash: record.metricSpecHash || "",
      targetHash: record.targetHash || "",
      value: record.value,
      unit: record.unit || "",
      outputHashes: record.outputHashes || [],
      inputHashes: record.inputHashes || [],
      metricHashes: record.metricHashes || [],
      rollbackPatchHash: record.rollbackPatchHash || "",
      status: record.status || "",
      summaryHash: boomKasmObjectHash("explain-hash-summary-v1", {
        hash: resolved.hash,
        role: resolved.role,
        objectKind: record.kind || "unknown",
      }),
    };
  }

  function normalizeBoomProgramName(name = "") {
    return String(name || "default_program").trim().replace(/^template\./, "").replace(/[^\w.-]+/g, "_") || "default_program";
  }

  function isBoomComputeProgramTemplate(template = "") {
    const normalized = normalizeBoomProgramName(template);
    return normalized.includes("compute")
      || normalized.includes("gpu_cull")
      || normalized.includes("lod_select")
      || normalized.includes("metric_eval");
  }

  function boomComputeTemplateOpcodes(template = "") {
    const normalized = normalizeBoomProgramName(template);
    if (normalized.includes("gpu_cull")) return ["load_entity_soa", "load_bounds", "frustum_test", "write_visible_instances"];
    if (normalized.includes("lod_select")) return ["load_geocluster_bounds", "estimate_screen_error", "write_lod_selection"];
    if (normalized.includes("metric_eval")) return ["load_target_hash", "evaluate_metric_program", "write_metric_buffer"];
    return ["load_input_buffers", "dispatch_template_kernel", "write_output_buffers"];
  }

  function boomKasmHashScore(value = "") {
    const text = String(value || "");
    let score = 2166136261;
    for (let index = 0; index < text.length; index += 1) {
      score ^= text.charCodeAt(index);
      score = Math.imul(score, 16777619) >>> 0;
    }
    return Number((score / 0xffffffff).toFixed(6));
  }

  function buildBoomProgramSpec(name, options = {}) {
    const programName = normalizeBoomProgramName(name);
    const template = normalizeBoomProgramName(options.template || programName);
    const computeTemplate = isBoomComputeProgramTemplate(template);
    const sourceHash = options.sourceHash || boomKasmObjectHash("program-source-v1", {
      name: programName,
      template,
      promptHash: boomKasmObjectHash("program-prompt-v1", options.rawInput || programName),
    });
    const bytecodeHash = options.bytecodeHash || boomKasmObjectHash("program-bytecode-v1", {
      sourceHash,
      template,
      opcodes: computeTemplate
        ? ["load_buffer_hashes", "compile_compute_ir", "dispatch_sandbox", "emit_output_hashes"]
        : ["load_input_hashes", "apply_template", "emit_output_hashes"],
    });
    const inputSchemaHash = options.inputSchemaHash || boomKasmObjectHash("program-input-schema-v1", {
      name: programName,
      fields: options.inputFields || (computeTemplate ? ["target_hash", "input_buffer_hashes", "dispatch_hash", "budget_hash"] : ["target_hash", "budget_hash"]),
    });
    const outputSchemaHash = options.outputSchemaHash || boomKasmObjectHash("program-output-schema-v1", {
      name: programName,
      fields: options.outputFields || (computeTemplate ? ["compute_program_hash", "output_buffer_hashes", "metric_hashes", "proof_hash"] : ["output_hash", "metric_hashes", "proof_hash"]),
    });
    const sandboxTemplateHash = options.sandboxTemplateHash || boomKasmObjectHash("program-sandbox-template-v1", {
      filesystem: false,
      shell: false,
      renderer: false,
      worldPatchOnly: true,
    });
    const permissions = stableBoomValue(options.permissions || {
      world: true,
      assets: computeTemplate,
      filesystem: false,
      shell: false,
      network: false,
      renderer: false,
      gpuCompute: computeTemplate,
    });
    const budget = stableBoomValue(options.budget || {
      frameMs: 16.667,
      cpuMs: 50,
      gpuMs: computeTemplate ? 4 : 0,
      ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
      vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
    });
    const spec = {
      kind: "kasm-program-spec",
      version: 1,
      name: programName,
      template,
      sourceHash,
      bytecodeHash,
      inputSchemaHash,
      outputSchemaHash,
      sandboxTemplateHash,
      permissions,
      permissionHash: boomKasmObjectHash("program-permission-set-v1", permissions),
      budget,
      budgetHash: boomKasmObjectHash("program-budget-v1", budget),
      deterministic: options.deterministic !== false,
    };
    spec.id = boomKasmObjectHash("program-spec-v1", spec);
    return spec;
  }

  function ensureBoomProgramSpec(name, options = {}) {
    const programName = normalizeBoomProgramName(name);
    const existing = boomKasmProgramRegistry.get(programName);
    if (existing) return existing;
    const spec = buildBoomProgramSpec(programName, options);
    boomKasmProgramRegistry.set(programName, spec);
    boomKasmSpineStats.programSpecs += 1;
    rememberBoomKasmRecord(boomKasmProgramHistory, spec, BOOM_KASM_PROGRAM_HISTORY_LIMIT);
    return spec;
  }

  function resolveBoomProgramSpec(nameOrHash = "") {
    const ref = String(nameOrHash || "").trim();
    if (!ref || ref === "last") return boomKasmProgramHistory[boomKasmProgramHistory.length - 1] || null;
    const byName = boomKasmProgramRegistry.get(normalizeBoomProgramName(ref));
    if (byName) return byName;
    const resolved = resolveBoomKasmHash(ref)?.record || null;
    return resolved?.kind === "kasm-program-spec" ? resolved : null;
  }

  function boomComputeWorkItemCount(template = "") {
    const normalized = normalizeBoomProgramName(template);
    if (normalized.includes("gpu_cull")) return Math.max(1, (boomScene.items || []).filter((item) => item.visible !== false).length);
    if (normalized.includes("lod_select")) return Math.max(1, Math.ceil(Number(sceneMesh?.faceCount || activeBoomMeshItem()?.meta?.faceCount || 1) / 128));
    if (normalized.includes("metric_eval")) return 1;
    return Math.max(1, Number(sceneMesh?.faceCount || activeBoomMeshItem()?.meta?.faceCount || boomScene.items?.length || 1));
  }

  function boomComputeInputBuffers(inputHash, template = "") {
    const sceneHash = boomKasmCurrentSceneHash();
    const activeMesh = sceneMesh || activeBoomMeshItem()?.mesh || null;
    const meshHash = activeMesh ? boomGeometryHash(activeMesh.display || activeMesh.base || activeMesh) : sceneHash;
    return [
      {
        name: "scene_hash",
        hash: sceneHash,
        access: "read",
        bytes: 32,
      },
      {
        name: "target_hash",
        hash: inputHash || sceneHash,
        access: "read",
        bytes: 32,
      },
      {
        name: "entity_soa",
        hash: boomKasmObjectHash("compute-entity-soa-v1", (boomScene.items || []).map((item) => [item.id, item.type, item.visible !== false, item.renderable !== false])),
        access: "read",
        bytes: Math.max(64, (boomScene.items || []).length * 64),
      },
      {
        name: normalizeBoomProgramName(template).includes("metric_eval") ? "metric_target" : "mesh_or_asset_pages",
        hash: meshHash,
        access: "read",
        bytes: Math.max(32, Number(activeMesh?.display?.pos?.byteLength || activeMesh?.pos?.byteLength || 0)),
      },
    ];
  }

  function boomComputeOutputBuffers(programName, template = "", workItems = 1) {
    const normalized = normalizeBoomProgramName(template);
    const outputName = normalized.includes("gpu_cull")
      ? "visible_instances"
      : normalized.includes("lod_select")
        ? "lod_selection"
        : normalized.includes("metric_eval")
          ? "metric_values"
          : "compute_output";
    const bytesPerItem = normalized.includes("metric_eval") ? 16 : 32;
    const bytes = Math.max(32, Math.ceil(workItems) * bytesPerItem);
    return [
      {
        name: outputName,
        hash: boomKasmObjectHash("compute-output-buffer-template-v1", { programName, template, outputName, workItems }),
        access: "write",
        bytes,
      },
    ];
  }

  function buildBoomComputeProgram(name, options = {}) {
    const programName = normalizeBoomProgramName(name || options.programName || "compute_shader");
    const template = normalizeBoomProgramName(options.template || programName || "compute_shader");
    const inputHash = String(options.inputHash || options.targetHash || boomKasmCurrentSceneHash());
    const workItems = Math.max(1, Number(options.workItems || boomComputeWorkItemCount(template)));
    const workgroupSize = Math.max(1, Math.min(256, Number(options.workgroupSize || 64)));
    const dispatch = stableBoomValue(options.dispatch || {
      x: Math.max(1, Math.ceil(workItems / workgroupSize)),
      y: 1,
      z: 1,
      workgroupSize,
      workItems,
    });
    const inputBuffers = stableBoomValue(options.inputBuffers || boomComputeInputBuffers(inputHash, template));
    const outputBuffers = stableBoomValue(options.outputBuffers || boomComputeOutputBuffers(programName, template, workItems));
    const shaderHash = options.shaderHash || boomKasmObjectHash("compute-shader-template-v1", {
      programName,
      template,
      opcodes: boomComputeTemplateOpcodes(template),
    });
    const backend = options.backend || (gl ? "WebGL2ComputeIR" : "CpuSimComputeIR");
    const program = {
      kind: "kasm-compute-program",
      version: 1,
      name: programName,
      template,
      shaderHash,
      inputBuffers,
      outputBuffers,
      dispatch,
      backend,
      inputSchemaHash: boomKasmObjectHash("compute-input-schema-v1", inputBuffers.map((buffer) => [buffer.name, buffer.access])),
      outputSchemaHash: boomKasmObjectHash("compute-output-schema-v1", outputBuffers.map((buffer) => [buffer.name, buffer.access])),
      sandboxHash: boomKasmObjectHash("compute-sandbox-v1", {
        directShaderSource: false,
        directRenderer: false,
        directFilesystem: false,
        bytecodeOnly: true,
      }),
      budget: stableBoomValue(options.budget || { cpuMs: 2, gpuMs: 4, ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES, vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES }),
      deterministic: true,
    };
    program.id = boomKasmObjectHash("compute-program-v1", program);
    return program;
  }

  function ensureBoomComputeProgram(name, options = {}) {
    const programName = normalizeBoomProgramName(name || "compute_shader");
    const existing = boomKasmComputeRegistry.get(programName);
    if (existing) return existing;
    const program = buildBoomComputeProgram(programName, options);
    boomKasmComputeRegistry.set(programName, program);
    boomKasmSpineStats.computePrograms += 1;
    rememberBoomKasmRecord(boomKasmComputeHistory, program, BOOM_KASM_COMPUTE_HISTORY_LIMIT);
    return program;
  }

  function resolveBoomComputeProgram(nameOrHash = "") {
    const ref = String(nameOrHash || "").trim();
    if (!ref || ref === "last") {
      return [...boomKasmComputeHistory].reverse().find((record) => record?.kind === "kasm-compute-program") || null;
    }
    const byName = boomKasmComputeRegistry.get(normalizeBoomProgramName(ref));
    if (byName) return byName;
    const resolved = resolveBoomKasmHash(ref)?.record || null;
    return resolved?.kind === "kasm-compute-program" ? resolved : null;
  }

  function runBoomComputeProgram(nameOrHash, options = {}) {
    const programSpec = options.programSpec || resolveBoomProgramSpec(nameOrHash);
    const template = normalizeBoomProgramName(options.template || programSpec?.template || nameOrHash || "compute_shader");
    const computeProgram = resolveBoomComputeProgram(nameOrHash) || ensureBoomComputeProgram(programSpec?.name || nameOrHash || "compute_shader", {
      template,
      inputHash: options.inputHash,
      targetHash: options.targetHash,
    });
    const inputHash = String(options.inputHash || options.targetHash || boomKasmCurrentSceneHash());
    const outputBuffers = computeProgram.outputBuffers.map((buffer, index) => ({
      ...buffer,
      hash: boomKasmObjectHash("compute-output-buffer-v1", {
        computeProgramHash: computeProgram.id,
        inputHash,
        bufferHash: buffer.hash,
        index,
      }),
    }));
    const outputHash = boomKasmObjectHash("compute-dispatch-output-v1", {
      computeProgramHash: computeProgram.id,
      inputHash,
      outputBuffers: outputBuffers.map((buffer) => buffer.hash),
    });
    const metricRecords = ["compute_dispatch_count", "compute_buffer_bytes", "run_latency_ms"].map((metricName) => runBoomKasmMetric(metricName, outputHash, {
      targetHash: outputHash,
      target: { id: outputHash, kind: "kasm-compute-output", computeProgramHash: computeProgram.id },
      computeProgram,
      outputBuffers,
      started: options.started || boomNowMs(),
    }));
    const metricHashes = metricRecords.map((metric) => metric.id);
    const proofRecord = {
      kind: "kasm-proof-record",
      version: 1,
      commandHash: options.commandHash || "",
      inputHashes: [inputHash, ...computeProgram.inputBuffers.map((buffer) => buffer.hash)],
      programHashes: [computeProgram.id, programSpec?.id].filter(Boolean),
      sandboxHash: computeProgram.sandboxHash,
      outputHashes: [outputHash, ...outputBuffers.map((buffer) => buffer.hash)],
      metricHashes,
      environmentHash: boomKasmObjectHash("compute-environment-v1", {
        backend: computeProgram.backend,
        renderer: !!gl,
        workgroupSize: computeProgram.dispatch.workgroupSize,
      }),
    };
    proofRecord.id = boomKasmObjectHash("proof-record-v1", proofRecord);
    const runRecord = {
      kind: "kasm-compute-dispatch-record",
      version: 1,
      computeProgramHash: computeProgram.id,
      shaderHash: computeProgram.shaderHash,
      inputHashes: proofRecord.inputHashes,
      outputHashes: proofRecord.outputHashes,
      metricHashes,
      proofHash: proofRecord.id,
      dispatch: computeProgram.dispatch,
      backend: computeProgram.backend,
      status: "ok",
    };
    runRecord.id = boomKasmObjectHash("compute-dispatch-record-v1", runRecord);
    rememberBoomKasmHash({ kind: "kasm-compute-output", version: 1, id: outputHash, computeProgramHash: computeProgram.id, inputHashes: [inputHash] }, "compute-output");
    outputBuffers.forEach((buffer) => rememberBoomKasmHash({ kind: "kasm-compute-buffer", version: 1, id: buffer.hash, ...buffer }, "compute-buffer"));
    boomKasmSpineStats.computeRuns += 1;
    boomKasmSpineStats.proofRecords += 1;
    rememberBoomKasmRecord(boomKasmComputeHistory, runRecord, BOOM_KASM_COMPUTE_HISTORY_LIMIT);
    rememberBoomKasmRecord(boomKasmProofHistory, proofRecord);
    emitBoomAudit("kasm_compute_dispatch", "DIRECT", runRecord.id, 0, computeProgram.dispatch.workItems || 1, "work_items", {
      computeProgramHash: computeProgram.id,
      shaderHash: computeProgram.shaderHash,
      outputHash,
      proofHash: proofRecord.id,
      backend: computeProgram.backend,
    });
    return runRecord;
  }

  function runBoomProgramSpec(nameOrHash, options = {}) {
    const program = resolveBoomProgramSpec(nameOrHash) || ensureBoomProgramSpec(nameOrHash || "default_program", options);
    const inputHash = String(options.inputHash || options.targetHash || boomKasmCurrentSceneHash());
    const computeRun = isBoomComputeProgramTemplate(program.template)
      ? runBoomComputeProgram(program.name, {
          programSpec: program,
          template: program.template,
          inputHash,
          started: options.started || boomNowMs(),
        })
      : null;
    const outputHash = boomKasmObjectHash("program-run-output-v1", {
      programHash: program.id,
      inputHash,
      sceneHash: boomKasmCurrentSceneHash(),
      runSeed: options.runSeed || program.bytecodeHash,
      computeRunHash: computeRun?.id || "",
    });
    rememberBoomKasmHash({
      kind: "kasm-program-output",
      version: 1,
      id: outputHash,
      programHash: program.id,
      inputHashes: [inputHash],
    }, "program-output");
    const metricRecords = (options.metrics || ["scene_complexity", "draw_call_cost", "run_latency_ms"]).map((name) => runBoomKasmMetric(name, outputHash, {
      targetHash: outputHash,
      target: { id: outputHash, kind: "kasm-program-output", programHash: program.id },
      started: options.started || boomNowMs(),
    }));
    const record = {
      kind: "kasm-program-run",
      version: 1,
      programHash: program.id,
      bytecodeHash: program.bytecodeHash,
      inputHashes: [inputHash],
      outputHashes: [...new Set([outputHash, computeRun?.id, computeRun?.proofHash, ...(computeRun?.outputHashes || [])].filter(Boolean))],
      metricHashes: metricRecords.map((metric) => metric.id),
      computeRunHash: computeRun?.id || "",
      budgetHash: program.budgetHash,
      status: "ok",
    };
    record.id = boomKasmObjectHash("program-run-v1", record);
    boomKasmSpineStats.programRuns += 1;
    rememberBoomKasmRecord(boomKasmProgramRunHistory, record, BOOM_KASM_PROGRAM_HISTORY_LIMIT);
    emitBoomAudit("kasm_program_run", "DIRECT", record.id, 0, 1, "program_runs", {
      programName: program.name,
      programHash: program.id,
      outputHash,
      metricHashes: record.metricHashes,
    });
    return record;
  }

  function parseBoomProgramSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = tokens[0] || "run";
    const nameIndex = action === "run" || action === "profile" || action === "test" || action === "promote" ? 1 : 0;
    const programName = tokens[nameIndex] && !tokens[nameIndex].startsWith("--") ? tokens[nameIndex] : "default_program";
    const templateIndex = tokens.findIndex((token) => token === "--template");
    const inputIndex = tokens.findIndex((token) => token === "--input" || token === "--target");
    return {
      action,
      programName: normalizeBoomProgramName(programName),
      template: templateIndex >= 0 ? normalizeBoomProgramName(tokens[templateIndex + 1] || programName) : normalizeBoomProgramName(programName),
      inputHash: inputIndex >= 0 ? tokens[inputIndex + 1] || "last" : boomKasmCurrentSceneHash(),
    };
  }

  function parseBoomMatrixSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = tokens[0] || "run";
    const programName = tokens[1] && !tokens[1].startsWith("--") ? tokens[1] : "default_program";
    const variantsIndex = tokens.findIndex((token) => token === "--variants");
    const metricsIndex = tokens.findIndex((token) => token === "--metrics");
    const rawMetrics = metricsIndex >= 0 ? tokens[metricsIndex + 1] || "" : "scene_complexity,draw_call_cost,ram_cache_fill_pct";
    return {
      action,
      programName: normalizeBoomProgramName(programName),
      variants: variantsIndex >= 0 ? Number(tokens[variantsIndex + 1]) : 128,
      metrics: rawMetrics.split(",").map(normalizeBoomMetricName).filter(Boolean),
    };
  }

  function runBoomKasmMatrix(programRef, options = {}) {
    const program = resolveBoomProgramSpec(programRef) || ensureBoomProgramSpec(programRef || "default_program", options);
    const variantCount = Math.max(1, Math.min(512, Number(options.variants || 128)));
    const metricNames = (options.metrics?.length ? options.metrics : ["scene_complexity", "draw_call_cost", "ram_cache_fill_pct"]).map(normalizeBoomMetricName);
    const metricSpecs = metricNames.map((name) => ensureBoomMetricSpec(name));
    const metricSetHash = boomKasmObjectHash("matrix-metric-set-v1", metricSpecs.map((spec) => spec.id));
    const variants = Array.from({ length: variantCount }, (_, index) => {
      const inputHash = boomKasmObjectHash("matrix-variant-input-v1", {
        programHash: program.id,
        index,
        sceneHash: boomKasmCurrentSceneHash(),
      });
      const outputHash = boomKasmObjectHash("matrix-variant-output-v1", {
        programHash: program.id,
        inputHash,
        metricSetHash,
      });
      if (index < 8) {
        rememberBoomKasmHash({
          kind: "kasm-matrix-variant-output",
          version: 1,
          id: outputHash,
          programHash: program.id,
          inputHashes: [inputHash],
          metricSetHash,
        }, "matrix-variant-output");
      }
      const score = boomKasmHashScore(`${outputHash}:${metricSetHash}`);
      return { index, inputHash, outputHash, score };
    });
    const top = variants.slice().sort((a, b) => b.score - a.score).slice(0, Math.min(8, variants.length));
    const matrixRun = {
      kind: "kasm-matrix-run",
      version: 1,
      programHash: program.id,
      bytecodeHash: program.bytecodeHash,
      variantCount,
      variantHashes: variants.map((variant) => variant.outputHash),
      metricSetHash,
      metricHashes: metricSpecs.map((spec) => spec.id),
      top,
      budgetHash: program.budgetHash,
      status: "ok",
    };
    matrixRun.id = boomKasmObjectHash("matrix-run-v1", matrixRun);
    boomKasmSpineStats.matrixRuns += 1;
    rememberBoomKasmRecord(boomKasmMatrixHistory, matrixRun, BOOM_KASM_MATRIX_HISTORY_LIMIT);
    emitBoomAudit("kasm_matrix_run", "DIRECT", matrixRun.id, 0, variantCount, "variants", {
      programName: program.name,
      programHash: program.id,
      metricSetHash,
      topHash: top[0]?.outputHash || "",
    });
    return matrixRun;
  }

  function normalizeBoomSkillName(name = "") {
    return String(name || "default_skill").trim().replace(/^template\.skill\./, "").replace(/[^\w.-]+/g, "_") || "default_skill";
  }

  function buildBoomSkillSpec(name, options = {}) {
    const skillName = normalizeBoomSkillName(name);
    const programNames = (options.programs?.length ? options.programs : [
      `${skillName}.plan`,
      `${skillName}.emit_world_patch`,
      `${skillName}.score`,
    ]).map(normalizeBoomProgramName);
    const programSpecs = programNames.map((programName) => ensureBoomProgramSpec(programName, {
      template: options.template || skillName,
      rawInput: options.rawInput || skillName,
    }));
    const metricNames = (options.metrics?.length ? options.metrics : ["scene_complexity", "draw_call_cost", "ram_cache_fill_pct", "run_latency_ms"]).map(normalizeBoomMetricName);
    const metricSpecs = metricNames.map((metricName) => ensureBoomMetricSpec(metricName));
    const tests = stableBoomValue(options.tests || [
      { name: "deterministic_replay", expect: "same_output_hash" },
      { name: "proof_required", expect: "proof_hash" },
      { name: "budget_bound", expect: "within_resource_budget" },
    ]);
    const permissions = stableBoomValue(options.permissions || {
      world: true,
      assets: false,
      filesystem: false,
      shell: false,
      network: false,
      renderer: false,
    });
    const inputSchemaHash = boomKasmObjectHash("skill-input-schema-v1", {
      skillName,
      fields: options.inputFields || ["target_hash", "theme", "budget_hash"],
    });
    const outputSchemaHash = boomKasmObjectHash("skill-output-schema-v1", {
      skillName,
      kind: options.outputKind || "world_patch",
      fields: ["world_patch_hash", "metric_report_hash", "proof_hash"],
    });
    const spec = {
      kind: "kasm-skill-spec",
      version: 1,
      skillVersion: Number(options.skillVersion || 1),
      name: skillName,
      programHashes: programSpecs.map((program) => program.id),
      programGraphHash: boomKasmObjectHash("skill-program-graph-v1", programSpecs.map((program) => ({
        programHash: program.id,
        bytecodeHash: program.bytecodeHash,
      }))),
      inputSchemaHash,
      outputSchemaHash,
      metricSetHash: boomKasmObjectHash("skill-metric-set-v1", metricSpecs.map((metric) => metric.id)),
      metricSpecHashes: metricSpecs.map((metric) => metric.id),
      metricNames,
      permissionHash: boomKasmObjectHash("skill-permissions-v1", permissions),
      permissions,
      testSetHash: boomKasmObjectHash("skill-test-set-v1", tests),
      tests,
      outputKind: options.outputKind || "world_patch",
      promotedFromHash: options.promotedFromHash || null,
    };
    spec.id = boomKasmObjectHash("skill-spec-v1", spec);
    return spec;
  }

  function ensureBoomSkillSpec(name, options = {}) {
    const skillName = normalizeBoomSkillName(name);
    const existing = boomKasmSkillRegistry.get(skillName);
    if (existing && !options.promotedFromHash) return existing;
    const spec = buildBoomSkillSpec(skillName, options);
    boomKasmSkillRegistry.set(skillName, spec);
    boomKasmSpineStats.skillSpecs += 1;
    rememberBoomKasmRecord(boomKasmSkillHistory, spec, BOOM_KASM_SKILL_HISTORY_LIMIT);
    return spec;
  }

  function resolveBoomSkillSpec(nameOrHash = "") {
    const ref = String(nameOrHash || "").trim();
    if (!ref || ref === "last") return boomKasmSkillHistory[boomKasmSkillHistory.length - 1] || null;
    const byName = boomKasmSkillRegistry.get(normalizeBoomSkillName(ref));
    if (byName) return byName;
    const resolved = resolveBoomKasmHash(ref)?.record || null;
    return resolved?.kind === "kasm-skill-spec" ? resolved : null;
  }

  function runBoomSkillSpec(nameOrHash, options = {}) {
    const skill = resolveBoomSkillSpec(nameOrHash) || ensureBoomSkillSpec(nameOrHash || "default_skill", options);
    const inputHash = String(options.inputHash === "current" ? boomKasmCurrentSceneHash() : options.inputHash || options.targetHash || boomKasmCurrentSceneHash());
    const programRuns = (skill.programHashes || []).map((programHash, index) => runBoomProgramSpec(programHash, {
      inputHash,
      runSeed: `${skill.programGraphHash}:${index}`,
      metrics: [],
      started: options.started || boomNowMs(),
    }));
    const skillOutputHash = boomKasmObjectHash("skill-output-v1", {
      skillHash: skill.id,
      inputHash,
      programRunHashes: programRuns.map((run) => run.id),
      outputKind: skill.outputKind,
    });
    rememberBoomKasmHash({
      kind: "kasm-skill-output",
      version: 1,
      id: skillOutputHash,
      skillHash: skill.id,
      programGraphHash: skill.programGraphHash,
      inputHashes: [inputHash],
    }, "skill-output");
    const metricRecords = (skill.metricNames || ["scene_complexity", "draw_call_cost"]).map((metricName) => runBoomKasmMetric(metricName, skillOutputHash, {
      targetHash: skillOutputHash,
      target: { id: skillOutputHash, kind: "kasm-skill-output", skillHash: skill.id },
      started: options.started || boomNowMs(),
    }));
    const recordBase = {
      kind: "kasm-skill-run",
      version: 1,
      skillHash: skill.id,
      programGraphHash: skill.programGraphHash,
      inputHashes: [inputHash],
      programRunHashes: programRuns.map((run) => run.id),
      outputHashes: [skillOutputHash, ...programRuns.flatMap((run) => run.outputHashes || [])],
      metricHashes: metricRecords.map((metric) => metric.id),
      proofHash: "",
      status: "ok",
    };
    const proofRecord = {
      kind: "kasm-skill-proof-record",
      version: 1,
      skillHash: skill.id,
      inputHashes: recordBase.inputHashes,
      programHashes: skill.programHashes || [],
      programGraphHash: skill.programGraphHash,
      outputHashes: recordBase.outputHashes,
      metricHashes: recordBase.metricHashes,
      sandboxHash: skill.permissionHash,
      testSetHash: skill.testSetHash,
      environmentHash: boomKasmObjectHash("skill-environment-v1", {
        cacheMaxBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
        renderer: !!gl,
      }),
    };
    proofRecord.id = boomKasmObjectHash("skill-proof-record-v1", proofRecord);
    const record = { ...recordBase, proofHash: proofRecord.id };
    record.id = boomKasmObjectHash("skill-run-v1", record);
    boomKasmSpineStats.skillRuns += 1;
    boomKasmSpineStats.proofRecords += 1;
    rememberBoomKasmRecord(boomKasmSkillRunHistory, record, BOOM_KASM_SKILL_HISTORY_LIMIT);
    rememberBoomKasmRecord(boomKasmProofHistory, proofRecord);
    emitBoomAudit("kasm_skill_run", "DIRECT", record.id, 0, programRuns.length, "programs", {
      skillName: skill.name,
      skillHash: skill.id,
      programGraphHash: skill.programGraphHash,
      proofHash: proofRecord.id,
    });
    return record;
  }

  function parseBoomSkillSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = tokens[0] || "run";
    const nameIndex = action === "create" || action === "run" ? 1 : -1;
    const nameFlagIndex = tokens.findIndex((token) => token === "--name");
    const targetIndex = tokens.findIndex((token) => token === "--target" || token === "--input" || token === "--from");
    const metricsIndex = tokens.findIndex((token) => token === "--metrics");
    const rawName = nameFlagIndex >= 0
      ? tokens[nameFlagIndex + 1]
      : nameIndex >= 0 && tokens[nameIndex] && !tokens[nameIndex].startsWith("--")
        ? tokens[nameIndex]
        : "default_skill";
    const rawMetrics = metricsIndex >= 0 ? tokens[metricsIndex + 1] || "" : "";
    return {
      action,
      skillName: normalizeBoomSkillName(rawName),
      targetHash: targetIndex >= 0 ? tokens[targetIndex + 1] || "current" : "current",
      metrics: rawMetrics ? rawMetrics.split(",").map(normalizeBoomMetricName).filter(Boolean) : [],
    };
  }

  function normalizeBoomRenderMode(mode = "") {
    const normalized = String(mode || "lit").trim().toLowerCase();
    return ["solid", "lit", "wireframe", "collision", "navmesh", "overdraw", "vram", "vramheatmap", "metric", "metricoverlay"].includes(normalized)
      ? normalized.replace("vramheatmap", "vram").replace("metricoverlay", "metric")
      : "lit";
  }

  function boomAssetPagesFromGeometry(kind, label, geometry, residency = "WarmRam", pageBytes = BOOM_KASM_ASSET_PAGE_BYTES) {
    const posBytes = Number(geometry?.pos?.byteLength || 0);
    const nrmBytes = Number(geometry?.nrm?.byteLength || 0);
    const colBytes = Number(geometry?.col?.byteLength || 0);
    const decompressedSize = posBytes + nrmBytes + colBytes;
    const sourceHash = geometry ? boomGeometryHash(geometry) : boomKasmObjectHash("empty-asset-page-v1", { kind, label });
    const buffers = [
      { name: "pos", bytes: posBytes, ratio: 0.58 },
      { name: "nrm", bytes: nrmBytes, ratio: 0.42 },
      { name: "col", bytes: colBytes, ratio: 0.46 },
    ].filter((buffer) => buffer.bytes > 0);
    if (!buffers.length) {
      const emptyPage = {
        kind: "kasm-asset-page",
        version: 1,
        pageKind: kind,
        label,
        sourceHash,
        bufferName: "empty",
        pageIndex: 0,
        byteOffset: 0,
        byteLength: 0,
        compressedBytesHash: boomKasmObjectHash("asset-page-compressed-v2", { sourceHash, kind, label, empty: true }),
        compressedSize: 64,
        decompressedSize: 0,
        totalDecompressedSize: 0,
        residency,
        evictable: residency !== "Pinned",
      };
      emptyPage.id = boomKasmObjectHash("asset-page-v2", emptyPage);
      return [emptyPage];
    }
    const pages = [];
    for (const buffer of buffers) {
      let offset = 0;
      while (offset < buffer.bytes) {
        const byteLength = Math.min(pageBytes, buffer.bytes - offset);
        const page = {
          kind: "kasm-asset-page",
          version: 2,
          pageKind: kind,
          label: `${label}.${buffer.name}.${pages.length}`,
          sourceHash,
          bufferName: buffer.name,
          pageIndex: pages.length,
          byteOffset: offset,
          byteLength,
          compressedBytesHash: boomKasmObjectHash("asset-page-compressed-v2", {
            sourceHash,
            kind,
            label,
            buffer: buffer.name,
            byteOffset: offset,
            byteLength,
          }),
          compressedSize: Math.max(64, Math.ceil(byteLength * buffer.ratio)),
          decompressedSize: byteLength,
          totalDecompressedSize: decompressedSize,
          residency,
          evictable: residency !== "Pinned",
        };
        page.id = boomKasmObjectHash("asset-page-v2", page);
        pages.push(page);
        offset += byteLength;
      }
    }
    return pages;
  }

  function boomAssetPageFromGeometry(kind, label, geometry, residency = "WarmRam") {
    return boomAssetPagesFromGeometry(kind, label, geometry, residency)[0];
  }

  function buildBoomAssetPack(pages, options = {}) {
    const uniquePages = [...new Map((pages || []).map((page) => [page.id, page])).values()];
    const totalCompressedSize = uniquePages.reduce((sum, page) => sum + Number(page.compressedSize || 0), 0);
    const totalDecompressedSize = uniquePages.reduce((sum, page) => sum + Number(page.decompressedSize || 0), 0);
    const sourceBytesByHash = new Map();
    for (const page of uniquePages) {
      const sourceHash = String(page.sourceHash || page.id || "");
      sourceBytesByHash.set(sourceHash, Math.max(Number(sourceBytesByHash.get(sourceHash) || 0), Number(page.totalDecompressedSize || page.decompressedSize || 0)));
    }
    const sourceBytes = [...sourceBytesByHash.values()].reduce((sum, bytes) => sum + bytes, 0);
    const pack = {
      kind: "kasm-asset-pack",
      version: 1,
      name: options.name || "scene_asset_pack",
      pageSize: BOOM_KASM_ASSET_PAGE_BYTES,
      pageHashes: uniquePages.map((page) => page.id),
      pageCount: uniquePages.length,
      totalCompressedSize,
      totalDecompressedSize,
      dedupedSourceBytes: sourceBytes,
      residencyHash: boomKasmObjectHash("asset-pack-residency-v1", uniquePages.map((page) => [page.id, page.residency])),
      budgetHash: boomKasmObjectHash("asset-pack-budget-v1", {
        ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
        vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
      }),
    };
    pack.id = boomKasmObjectHash("asset-pack-v1", pack);
    rememberBoomKasmHash(pack, "asset-pack");
    return pack;
  }

  function activeBoomMeshAssetSource() {
    const activeMesh = sceneMesh || activeBoomMeshItem()?.mesh || null;
    return activeMesh?.display?.pos?.length
      ? activeMesh.display
      : activeMesh?.base?.pos?.length
        ? activeMesh.base
        : activeMesh?.pos?.length
          ? activeMesh
          : null;
  }

  function boomClusterBounds(baseBounds, index, count) {
    const min = Array.isArray(baseBounds?.min) ? baseBounds.min : [-3, -3, -3];
    const max = Array.isArray(baseBounds?.max) ? baseBounds.max : [3, 3, 3];
    const spanX = (Number(max[0]) || 3) - (Number(min[0]) || -3);
    const start = count > 1 ? index / count : 0;
    const end = count > 1 ? (index + 1) / count : 1;
    return {
      min: [
        Number(((Number(min[0]) || -3) + spanX * start).toFixed(4)),
        Number(Number(min[1] || -3).toFixed(4)),
        Number(Number(min[2] || -3).toFixed(4)),
      ],
      max: [
        Number(((Number(min[0]) || -3) + spanX * end).toFixed(4)),
        Number(Number(max[1] || 3).toFixed(4)),
        Number(Number(max[2] || 3).toFixed(4)),
      ],
    };
  }

  function buildBoomGeoClusterAsset(options = {}) {
    const meshSource = activeBoomMeshAssetSource();
    const sourceMeshHash = meshSource ? boomGeometryHash(meshSource) : boomKasmCurrentSceneHash();
    const triangleCount = Math.max(1, Number(meshSource?.faceCount || Math.floor(Number(meshSource?.pos?.length || 9) / 9) || 1));
    const maxTris = Math.max(16, Math.min(512, Number(options.maxTris || 128)));
    const clusterCount = Math.max(1, Math.ceil(triangleCount / maxTris));
    const pageCount = Math.min(clusterCount, 2048);
    const bounds = meshSource?.bounds || sceneMesh?.bounds || { min: [-3, -3, -3], max: [3, 3, 3] };
    const materialHash = boomKasmObjectHash("geocluster-material-slots-v1", {
      activeId: boomScene.activeId,
      material: activeBoomMeshItem()?.material || "default",
    });
    const clusterPages = Array.from({ length: pageCount }, (_, index) => {
      const tris = index === pageCount - 1 ? Math.max(1, triangleCount - maxTris * index) : Math.min(maxTris, triangleCount);
      const vertexCount = Math.max(3, tris * 3);
      const indexCount = tris * 3;
      const pageBounds = boomClusterBounds(bounds, index, pageCount);
      const compressedBytesHash = boomKasmObjectHash("geocluster-compressed-page-v1", {
        sourceMeshHash,
        index,
        tris,
        pageBounds,
      });
      const page = {
        kind: "kasm-geocluster-page",
        version: 1,
        sourceMeshHash,
        pageIndex: index,
        compressedBytesHash,
        vertexCount,
        indexCount,
        bounds: pageBounds,
        lodError: Number(((index + 1) / pageCount / 16).toFixed(5)),
        compressedSize: Math.max(128, Math.ceil(vertexCount * 9.5)),
        decompressedSize: Math.max(256, vertexCount * 24 + indexCount * 4),
        residency: index < 4 ? "HotVram" : "WarmRam",
      };
      page.hash = boomKasmObjectHash("geocluster-page-v1", page);
      return page;
    });
    const asset = {
      kind: "kasm-geocluster-asset",
      version: 1,
      name: options.name || "active_mesh_geocluster",
      sourceMeshHash,
      clusterPages,
      clusterPageHashes: clusterPages.map((page) => page.hash),
      lodTreeHash: boomKasmObjectHash("geocluster-lod-tree-v1", {
        sourceMeshHash,
        pageHashes: clusterPages.map((page) => page.hash),
        lodMode: options.lod || "continuous",
      }),
      boundsTreeHash: boomKasmObjectHash("geocluster-bounds-tree-v1", clusterPages.map((page) => page.bounds)),
      materialSlots: [materialHash],
      maxTris,
      lodMode: options.lod || "continuous",
      triangleCount,
      clusterCount: pageCount,
      budgetHash: boomKasmObjectHash("geocluster-budget-v1", {
        ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
        vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
      }),
    };
    asset.id = boomKasmObjectHash("geocluster-asset-v1", asset);
    boomKasmSpineStats.geoClusters += 1;
    rememberBoomKasmRecord(boomKasmGeoClusterHistory, asset, BOOM_KASM_ASSET_HISTORY_LIMIT);
    return asset;
  }

  function boomAssetPagesFromGeoCluster(geoCluster) {
    return (geoCluster?.clusterPages || []).map((clusterPage) => {
      const page = {
        kind: "kasm-asset-page",
        version: 1,
        pageKind: "GeoClusterPage",
        label: `geocluster.${clusterPage.pageIndex}`,
        sourceHash: geoCluster.id,
        clusterPageHash: clusterPage.hash,
        compressedBytesHash: clusterPage.compressedBytesHash,
        compressedSize: clusterPage.compressedSize,
        decompressedSize: clusterPage.decompressedSize,
        totalDecompressedSize: geoCluster.clusterPages.reduce((sum, item) => sum + Number(item.decompressedSize || 0), 0),
        residency: clusterPage.residency || "WarmRam",
        evictable: clusterPage.residency !== "Pinned",
      };
      page.id = boomKasmObjectHash("asset-page-geocluster-v1", page);
      rememberBoomKasmRecord(boomKasmAssetPageHistory, page, BOOM_KASM_ASSET_HISTORY_LIMIT);
      return page;
    });
  }

  function buildBoomAssetPagesForScene(options = {}) {
    const pageByHash = new Map();
    const addPages = (nextPages) => {
      for (const page of nextPages || []) pageByHash.set(page.id, page);
    };
    const meshSource = activeBoomMeshAssetSource();
    if (meshSource) {
      addPages(boomAssetPagesFromGeometry("Mesh", "active_mesh", meshSource, meshSource.gpuCacheKey ? "HotVram" : "WarmRam"));
    }
    if (slicerPreview?.pos?.length) {
      addPages(boomAssetPagesFromGeometry("SlicerPreview", "slicer_preview", slicerPreview, slicerPreview.gpuCacheKey ? "HotVram" : "WarmRam"));
    }
    const materialHash = boomKasmObjectHash("asset-material-table-v1", {
      activeId: boomScene.activeId,
      items: (boomScene.items || []).map((item) => [item.id, item.type, item.visible !== false]),
    });
    const materialPage = {
      kind: "kasm-asset-page",
      version: 1,
      pageKind: "MaterialTable",
      label: "scene_material_table",
      sourceHash: materialHash,
      compressedBytesHash: boomKasmObjectHash("asset-page-compressed-v1", { materialHash }),
      compressedSize: 256,
      decompressedSize: 512,
      residency: "WarmRam",
      evictable: true,
    };
    materialPage.id = boomKasmObjectHash("asset-page-v1", materialPage);
    addPages([materialPage]);
    if (!pageByHash.size || options.includeScenePage !== false) {
      const sceneHash = boomKasmCurrentSceneHash();
      const scenePage = {
        kind: "kasm-asset-page",
        version: 1,
        pageKind: "SceneGraph",
        label: "scene_graph",
        sourceHash: sceneHash,
        compressedBytesHash: boomKasmObjectHash("asset-page-compressed-v1", { sceneHash }),
        compressedSize: 512,
        decompressedSize: 1024,
        residency: "WarmRam",
        evictable: false,
      };
      scenePage.id = boomKasmObjectHash("asset-page-v1", scenePage);
      addPages([scenePage]);
    }
    const pages = [...pageByHash.values()];
    for (const page of pages) {
      rememberBoomKasmRecord(boomKasmAssetPageHistory, page, BOOM_KASM_ASSET_HISTORY_LIMIT);
    }
    boomKasmSpineStats.assetPages += pages.length;
    return pages;
  }

  function normalizeBoomResidencyState(state = "WarmRam") {
    const normalized = String(state || "WarmRam").replace(/[\s_-]+/g, "").toLowerCase();
    if (normalized === "colddisk" || normalized === "cold") return "ColdDisk";
    if (normalized === "hotvram" || normalized === "vram" || normalized === "hot") return "HotVram";
    if (normalized === "evictable") return "Evictable";
    if (normalized === "pinned" || normalized === "pin") return "Pinned";
    return "WarmRam";
  }

  function normalizeBoomAssetAction(action = "scan") {
    const normalized = String(action || "scan").trim().replace(/-/g, "_").toLowerCase();
    if (normalized === "evict" || normalized === "evict_cold_pages") return "evict_cold";
    if (normalized === "pin" || normalized === "pin_hot_pages") return "pin_hot";
    if (normalized === "stream" || normalized === "streamplan") return "stream_plan";
    return normalized || "scan";
  }

  function boomResidencyBytesForState(pageStates, state) {
    return (pageStates || [])
      .filter((entry) => entry.residency === state)
      .reduce((sum, entry) => sum + Number(entry.decompressedSize || 0), 0);
  }

  function boomAssetPageWithResidency(page, residency, action = "residency") {
    const nextResidency = normalizeBoomResidencyState(residency);
    const next = {
      ...page,
      version: Math.max(2, Number(page?.version || 1) + 1),
      previousPageHash: page?.id || "",
      previousResidency: page?.residency || "",
      residency: nextResidency,
      evictable: nextResidency !== "Pinned" && page?.evictable !== false,
      residencyAction: action,
    };
    next.id = boomKasmObjectHash("asset-page-residency-v1", {
      previousPageHash: next.previousPageHash,
      residency: next.residency,
      action,
      sourceHash: next.sourceHash,
      compressedBytesHash: next.compressedBytesHash,
    });
    rememberBoomKasmRecord(boomKasmAssetPageHistory, next, BOOM_KASM_ASSET_HISTORY_LIMIT);
    return next;
  }

  function buildBoomAssetResidencyPlan(pages, options = {}) {
    const action = normalizeBoomAssetAction(options.action || "residency");
    const uniquePages = [...new Map((pages || []).filter(Boolean).map((page) => [page.id, page])).values()];
    const hotPages = Math.max(1, Math.min(uniquePages.length || 1, Number(options.hotPages || 4)));
    const targetHash = String(options.targetHash || options.target || "").trim();
    const pageStates = uniquePages.map((page, index) => {
      const current = normalizeBoomResidencyState(page.residency);
      const matchesTarget = !!targetHash && targetHash !== "current" && [page.id, page.sourceHash, page.clusterPageHash].includes(targetHash);
      const hotCandidate = index < hotPages || current === "HotVram" || matchesTarget;
      let residency = current;
      if (action === "evict_cold") {
        residency = page.evictable === false || current === "Pinned" ? current : (hotCandidate ? "HotVram" : "ColdDisk");
      } else if (action === "pin_hot") {
        residency = hotCandidate ? "Pinned" : current;
      } else if (action === "stream_plan") {
        residency = hotCandidate ? "HotVram" : (page.evictable === false ? current : "WarmRam");
      }
      return boomAssetPageWithResidency(page, residency, action);
    });
    const stateCounts = pageStates.reduce((acc, page) => {
      acc[page.residency] = (acc[page.residency] || 0) + 1;
      return acc;
    }, {});
    const ramBytes = ["WarmRam", "HotVram", "Pinned"].reduce((sum, state) => sum + boomResidencyBytesForState(pageStates, state), 0);
    const vramBytes = ["HotVram", "Pinned"].reduce((sum, state) => sum + boomResidencyBytesForState(pageStates, state), 0);
    const evictableBytes = pageStates
      .filter((page) => page.evictable !== false && page.residency !== "Pinned")
      .reduce((sum, page) => sum + Number(page.decompressedSize || 0), 0);
    const plan = {
      kind: "kasm-asset-residency-plan",
      version: 1,
      name: `asset_${action}_residency`,
      action,
      policy: action === "evict_cold" ? "keep-hot-evict-rest" : action === "pin_hot" ? "pin-hot-working-set" : "stable-page-table",
      inputHashes: uniquePages.map((page) => page.id),
      pageHashes: pageStates.map((page) => page.id),
      outputHashes: pageStates.map((page) => page.id),
      pageCount: pageStates.length,
      stateCounts,
      residencyHash: boomKasmObjectHash("asset-residency-table-v1", pageStates.map((page) => [page.id, page.residency])),
      assetStoreHash: boomKasmObjectHash("asset-store-single-v1", uniquePages.map((page) => page.sourceHash || page.id)),
      ramBytes,
      vramBytes,
      coldBytes: boomResidencyBytesForState(pageStates, "ColdDisk"),
      evictableBytes,
      pinnedBytes: boomResidencyBytesForState(pageStates, "Pinned"),
      budgetHash: boomKasmObjectHash("asset-residency-budget-v1", {
        ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
        vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES,
      }),
      status: vramBytes <= BOOM_GPU_RESOURCE_CACHE_MAX_BYTES && ramBytes <= BOOM_COMPUTE_CACHE_MAX_BYTES ? "budget-ok" : "over-budget",
    };
    plan.id = boomKasmObjectHash("asset-residency-plan-v1", plan);
    boomKasmSpineStats.assetResidencyPlans += 1;
    rememberBoomKasmRecord(boomKasmAssetResidencyHistory, plan, BOOM_KASM_ASSET_HISTORY_LIMIT);
    emitBoomAudit("kasm_asset_residency_plan", "DIRECT", plan.id, 0, pageStates.length, "asset_pages", {
      action,
      pageCount: plan.pageCount,
      ramBytes,
      vramBytes,
      status: plan.status,
    });
    return { plan, pages: pageStates };
  }

  function buildBoomRenderIR(options = {}) {
    const renderMode = normalizeBoomRenderMode(options.mode || options.renderMode || "lit");
    const sceneHash = boomKasmCurrentSceneHash();
    const activeMesh = sceneMesh || activeBoomMeshItem()?.mesh || null;
    const passes = activeMesh ? meshRenderPasses(activeMesh) : [];
    const assetPages = options.assetPages || buildBoomAssetPagesForScene();
    const entities = (boomScene.items || []).map((item) => ({
      id: item.id,
      type: item.type,
      visible: item.visible !== false,
      transformHash: boomKasmObjectHash("render-transform-v1", item.transform || {}),
      entityHash: boomKasmObjectHash("render-entity-v1", {
        id: item.id,
        type: item.type,
        visible: item.visible !== false,
        renderable: item.renderable !== false,
      }),
    }));
    const renderIR = {
      kind: "kasm-render-ir",
      version: 1,
      sceneHash,
      renderMode,
      entityHashes: entities.map((entity) => entity.entityHash),
      transformBufferHash: boomKasmObjectHash("render-transform-buffer-v1", entities.map((entity) => entity.transformHash)),
      meshInstanceBufferHash: boomKasmObjectHash("render-mesh-instance-buffer-v1", {
        sceneHash,
        passes: passes.length,
        activeMeshHash: activeMesh ? boomGeometryHash(activeMesh.display || activeMesh.base || activeMesh) : "",
      }),
      materialTableHash: boomKasmObjectHash("render-material-table-v1", assetPages.filter((page) => page.pageKind === "MaterialTable").map((page) => page.id)),
      lightTableHash: boomKasmObjectHash("render-light-table-v1", entities.filter((entity) => entity.type === "light").map((entity) => entity.entityHash)),
      cameraHash: boomKasmObjectHash("render-camera-v1", boomItemById("camera")?.transform || {}),
      assetPageHashes: assetPages.map((page) => page.id),
      assetPackHash: buildBoomAssetPack(assetPages, { name: "render_asset_pack" }).id,
      accelerationDataHash: passes.length ? boomKasmObjectHash("render-acceleration-data-v1", { sceneHash, passes: passes.length }) : null,
      frameBudget: stableBoomValue(options.budget || { fps: 60, frameMs: 16.667, ramBytes: BOOM_COMPUTE_CACHE_MAX_BYTES, vramBytes: BOOM_GPU_RESOURCE_CACHE_MAX_BYTES }),
    };
    renderIR.id = boomKasmObjectHash("render-ir-v1", renderIR);
    boomKasmSpineStats.renderIRs += 1;
    rememberBoomKasmRecord(boomKasmRenderHistory, renderIR, BOOM_KASM_RENDER_HISTORY_LIMIT);
    emitBoomAudit("kasm_render_ir", "DIRECT", renderIR.id, 0, entities.length, "entities", {
      sceneHash,
      renderMode,
      assetPages: assetPages.length,
      passes: passes.length,
    });
    return { renderIR, assetPages };
  }

  function parseBoomRenderSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = tokens[0] || "frame";
    const mode = action === "mode" ? tokens[1] || "lit" : tokens[0] && !tokens[0].startsWith("--") ? tokens[0] : "lit";
    return { action, mode: normalizeBoomRenderMode(mode) };
  }

  function parseBoomAssetSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = normalizeBoomAssetAction(tokens[0] || "scan");
    const kindIndex = tokens.findIndex((token) => token === "--kind");
    const rootIndex = tokens.findIndex((token) => token === "--root");
    const targetIndex = tokens.findIndex((token) => token === "--target");
    const maxTrisIndex = tokens.findIndex((token) => token === "--max-tris" || token === "--max_tris");
    const hotPagesIndex = tokens.findIndex((token) => token === "--hot-pages" || token === "--hot_pages");
    const lodIndex = tokens.findIndex((token) => token === "--lod");
    return {
      action,
      kind: kindIndex >= 0 ? String(tokens[kindIndex + 1] || "all") : "all",
      root: rootIndex >= 0 ? String(tokens[rootIndex + 1] || "project") : "project",
      target: targetIndex >= 0 ? String(tokens[targetIndex + 1] || "current") : "current",
      maxTris: maxTrisIndex >= 0 ? Number(tokens[maxTrisIndex + 1]) : 128,
      hotPages: hotPagesIndex >= 0 ? Number(tokens[hotPagesIndex + 1]) : 4,
      lod: lodIndex >= 0 ? String(tokens[lodIndex + 1] || "continuous") : "continuous",
    };
  }

  function normalizeBoomMetricName(name = "") {
    return String(name || "scene_complexity").trim().replace(/^template\.metric\./, "").replace(/[^\w.-]+/g, "_") || "scene_complexity";
  }

  function buildBoomMetricSpec(name, options = {}) {
    const metricName = normalizeBoomMetricName(name);
    const targetKind = String(options.targetKind || options.target || "kasm-object");
    const evaluatorProgramHash = boomKasmObjectHash("metric-evaluator-program-v1", {
      metricName,
      template: options.template || metricName,
      deterministic: true,
    });
    const spec = {
      kind: "kasm-metric-spec",
      version: 1,
      name: metricName,
      evaluatorProgramHash,
      targetSchemaHash: boomKasmObjectHash("metric-target-schema-v1", { metricName, targetKind }),
      aggregation: options.aggregation || "latest",
      threshold: Number.isFinite(Number(options.threshold)) ? Number(options.threshold) : null,
      weight: Number.isFinite(Number(options.weight)) ? Number(options.weight) : 1,
      budget: stableBoomValue(options.budget || { cpuMs: 1, ramBytes: 64 * 1024, gpuMs: 0 }),
    };
    spec.id = boomKasmObjectHash("metric-spec-v1", spec);
    return spec;
  }

  function ensureBoomMetricSpec(name, options = {}) {
    const metricName = normalizeBoomMetricName(name);
    const existing = boomKasmMetricRegistry.get(metricName);
    if (existing) return existing;
    const spec = buildBoomMetricSpec(metricName, options);
    boomKasmMetricRegistry.set(metricName, spec);
    boomKasmSpineStats.metricSpecs += 1;
    rememberBoomKasmMetricRecord(boomKasmMetricSpecHistory, spec);
    return spec;
  }

  function boomSceneComplexityScore() {
    const items = Array.isArray(boomScene.items) ? boomScene.items.length : 0;
    const modifierCount = (boomScene.items || []).reduce((sum, item) => sum + (isBoomMeshItem(item) ? ensureBoomItemModifiers(item).length : 0), 0);
    const faceCount = Number(sceneMesh?.faceCount || activeBoomMeshItem()?.meta?.faceCount || 0);
    const regionCost = boomScene.regionSelection?.cellHashes?.length || 0;
    return Number((items + modifierCount * 2 + faceCount / 1000 + regionCost / 128).toFixed(3));
  }

  function boomDrawCallEstimate() {
    const passes = sceneMesh ? Math.max(1, meshRenderPasses(sceneMesh).length) : 1;
    const visibleItems = (boomScene.items || []).filter((item) => item.visible !== false && item.renderable !== false).length;
    return Math.max(1, passes * Math.max(1, visibleItems));
  }

  function boomMetricValue(metricName, context = {}) {
    const name = normalizeBoomMetricName(metricName);
    if (name === "patch_ops_count") return { value: context.worldPatch?.ops?.length || 0, unit: "ops" };
    if (name === "rollback_ready") return { value: context.rollbackPatch ? 1 : 0, unit: "bool" };
    if (name === "scene_complexity") return { value: boomSceneComplexityScore(), unit: "score" };
    if (name === "draw_call_cost") return { value: boomDrawCallEstimate(), unit: "draws" };
    if (name === "cache_hit_rate") {
      const total = boomCacheStats.hits + boomCacheStats.misses;
      return { value: total ? Number((boomCacheStats.hits / total).toFixed(4)) : 0, unit: "ratio" };
    }
    if (name === "ram_cache_fill_pct") {
      return { value: Number(((boomComputeCacheBytes / BOOM_COMPUTE_CACHE_MAX_BYTES) * 100).toFixed(3)), unit: "pct" };
    }
    if (name === "run_latency_ms") {
      return { value: Number(Math.max(0, boomNowMs() - Number(context.started || boomNowMs())).toFixed(3)), unit: "ms" };
    }
    if (name === "vram_cost") {
      const bytes = context.assetResidencyPlan?.vramBytes ?? boomGpuResourceBytes;
      return { value: Number((bytes / (1024 * 1024)).toFixed(3)), unit: "mb" };
    }
    if (name === "asset_ram_cost") {
      const bytes = Number(context.assetResidencyPlan?.ramBytes || 0);
      return { value: Number((bytes / (1024 * 1024)).toFixed(3)), unit: "mb" };
    }
    if (name === "asset_vram_cost") {
      const bytes = Number(context.assetResidencyPlan?.vramBytes || 0);
      return { value: Number((bytes / (1024 * 1024)).toFixed(3)), unit: "mb" };
    }
    if (name === "asset_evictable_pages") {
      const plan = context.assetResidencyPlan || {};
      const evictablePages = Math.max(0, Number(plan.pageCount || 0) - Number(plan.stateCounts?.Pinned || 0));
      return { value: evictablePages, unit: "pages" };
    }
    if (name === "cluster_vram_cost") {
      const cluster = context.geoCluster || {};
      const bytes = (cluster.clusterPages || []).reduce((sum, page) => sum + Number(page.compressedSize || 0), 0);
      return { value: Number((bytes / (1024 * 1024)).toFixed(3)), unit: "mb" };
    }
    if (name === "cluster_lod_error") {
      const cluster = context.geoCluster || {};
      const pages = cluster.clusterPages || [];
      const maxError = pages.reduce((max, page) => Math.max(max, Number(page.lodError || 0)), 0);
      return { value: Number(maxError.toFixed(4)), unit: "error" };
    }
    if (name === "cluster_draw_cost") {
      const cluster = context.geoCluster || {};
      return { value: Math.max(1, (cluster.clusterPages || []).length), unit: "draws" };
    }
    if (name === "cluster_stream_cost") {
      const cluster = context.geoCluster || {};
      const pages = cluster.clusterPages || [];
      const evictable = pages.filter((page) => page.residency !== "Pinned").length;
      return { value: evictable, unit: "pages" };
    }
    if (name === "compute_dispatch_count") {
      const dispatch = context.computeProgram?.dispatch || {};
      return { value: Math.max(1, Number(dispatch.x || 1) * Number(dispatch.y || 1) * Number(dispatch.z || 1)), unit: "dispatches" };
    }
    if (name === "compute_buffer_bytes") {
      const buffers = [...(context.computeProgram?.inputBuffers || []), ...(context.outputBuffers || context.computeProgram?.outputBuffers || [])];
      const bytes = buffers.reduce((sum, buffer) => sum + Math.max(0, Number(buffer.bytes || 0)), 0);
      return { value: bytes, unit: "bytes" };
    }
    return { value: 0, unit: "unknown" };
  }

  function runBoomKasmMetric(metricName, targetHash = "last", context = {}) {
    const spec = ensureBoomMetricSpec(metricName, context.metricOptions || {});
    const resolved = targetHash && targetHash !== "last" ? resolveBoomKasmHash(targetHash) : null;
    const target = context.target || resolved?.record || boomKasmRunHistory[boomKasmRunHistory.length - 1] || null;
    const hash = String(context.targetHash || resolved?.hash || target?.id || targetHash || "last");
    const measured = boomMetricValue(spec.name, context);
    const thresholdPass = spec.threshold == null ? true : Number(measured.value) <= Number(spec.threshold);
    const record = {
      kind: "kasm-metric-record",
      version: 1,
      metricSpecHash: spec.id,
      evaluatorProgramHash: spec.evaluatorProgramHash,
      targetHash: hash,
      inputHashes: [hash],
      outputHash: boomKasmObjectHash("metric-output-v1", {
        metricSpecHash: spec.id,
        targetHash: hash,
        value: measured.value,
        unit: measured.unit,
      }),
      value: measured.value,
      unit: measured.unit,
      thresholdPass,
      status: thresholdPass ? "ok" : "threshold-failed",
    };
    record.id = boomKasmObjectHash("metric-record-v1", record);
    boomKasmSpineStats.metricRecords += 1;
    rememberBoomKasmMetricRecord(boomKasmMetricHistory, record);
    emitBoomAudit("kasm_metric_record", "DIRECT", record.id, 0, 1, "metrics", {
      metricName: spec.name,
      targetHash: hash,
      value: measured.value,
      unit: measured.unit,
      thresholdPass,
    });
    return record;
  }

  function runBoomDefaultMetrics(context = {}) {
    const names = context.worldPatch
      ? ["patch_ops_count", "rollback_ready", "scene_complexity", "draw_call_cost", "ram_cache_fill_pct", "run_latency_ms"]
      : ["scene_complexity", "draw_call_cost", "ram_cache_fill_pct", "run_latency_ms"];
    return names.map((name) => runBoomKasmMetric(name, context.targetHash || "last", context));
  }

  function boomKasmTargetHash(kind, id = "") {
    return boomKasmObjectHash("world-target-v1", { kind, id: String(id || "") });
  }

  function boomKasmSetPropertyOp(targetKind, targetId, key, before, value, mode = "replace") {
    return {
      op: "SetProperty",
      targetHash: boomKasmTargetHash(targetKind, targetId),
      targetKind,
      targetId: String(targetId || ""),
      key,
      before: stableBoomValue(before),
      value: stableBoomValue(value),
      mode,
    };
  }

  function boomKasmWorldOpsForCommand(commandSpec) {
    const command = commandSpec?.command?.type || "";
    const payload = commandSpec?.command?.payload || {};
    const ops = [];
    const sceneTarget = "scene";
    if (command === "boom.kasm.set_mode") {
      const args = String(payload.args || "").trim();
      const mode = ["create", "inspect", "optimize", "prove"].includes(args) ? args : "inspect";
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "workspaceMode", boomScene.workspaceMode, mode === "optimize" ? "slicer" : "design"));
    } else if (command === "boom.kasm.set_graph_view") {
      const viewId = BOOM_KASM_GRAPH_VIEWS.some((entry) => entry.id === payload.viewId) ? payload.viewId : "world";
      ops.push(boomKasmSetPropertyOp("ui", "projection", "kasmGraphView", boomScene.kasmGraphView || "world", viewId));
    } else if (command === "boom.kasm.select_hash") {
      ops.push(boomKasmSetPropertyOp("ui", "projection", "selectedKasmHash", boomScene.selectedKasmHash || "", String(payload.hash || "")));
    } else if (command === "boom.scene.workspace_mode") {
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "workspaceMode", boomScene.workspaceMode, payload.mode || payload.workspaceMode || "design"));
    } else if (command === "boom.viewport.set_edit_mode") {
      const nextMode = BOOM_EDIT_MODES.some((entry) => entry.id === payload.mode) ? payload.mode : "object";
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "editMode", boomScene.editMode, nextMode));
      if (nextMode === "object" || boomScene.componentSelection?.type !== nextMode) {
        ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "componentSelection", boomScene.componentSelection || null, null));
      }
    } else if (command === "boom.scene.select_item") {
      const itemId = String(payload.itemId || payload.id || "");
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "activeId", boomScene.activeId, itemId));
      if (!payload.preserveMeshComponentSelection) {
        ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "componentSelection", boomScene.componentSelection || null, null));
        ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "regionSelection", boomScene.regionSelection || null, null));
      }
    } else if (command === "boom.modifier.add" || command === "boom.modifier.apply") {
      const activeMesh = activeBoomMeshItem();
      ops.push(boomKasmSetPropertyOp("entity", activeMesh?.id || boomScene.activeId, "modifiers", ensureBoomItemModifiers(activeMesh || {}).length, {
        type: payload.type || "",
        payloadHash: commandSpec.payloadHash,
      }, "append"));
    } else if (command === "boom.slicer.set_workflow") {
      const nextWorkflow = ["prepare", "preview", "print"].includes(String(payload.workflow || "")) ? String(payload.workflow) : "prepare";
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "slicer.workflow", boomScene.slicer?.workflow || "prepare", nextWorkflow));
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "workspaceMode", boomScene.workspaceMode, "slicer"));
    } else if (command === "boom.query.region_from_selection" || command === "boom.query.volume_region_from_selection") {
      const region = command === "boom.query.volume_region_from_selection"
        ? boomKasmQueries?.volumeRegionFromSelection?.()
        : boomKasmQueries?.regionFromSelection?.();
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "regionSelection", boomScene.regionSelection || null, {
        computedFrom: boomScene.componentSelection?.nodeHash || boomScene.activeId,
        region: region ? boomRegionSummary(region) : null,
      }));
    } else if (command === "boom.region.clear") {
      ops.push(boomKasmSetPropertyOp(sceneTarget, "root", "regionSelection", boomScene.regionSelection || null, null));
    } else if (command === "boom.animation.play" || command === "boom.animation.pause") {
      ops.push(boomKasmSetPropertyOp("animation", "active", "playing", !!boomAnimationState?.playing, command === "boom.animation.play"));
    }
    return ops;
  }

  function buildBoomWorldPatchBundle(commandSpec) {
    const ops = boomKasmWorldOpsForCommand(commandSpec);
    if (!ops.length) return null;
    const patchIdentity = {
      kind: "kasm-world-patch",
      version: 1,
      commandHash: commandSpec.id,
      baseSceneHash: commandSpec.sceneHash,
      ops,
      expectedSceneHash: null,
      rollbackPatchHash: null,
      metricExpectations: [],
    };
    const patch = { ...patchIdentity };
    patch.id = boomKasmObjectHash("world-patch-v1", patchIdentity);
    const rollback = {
      kind: "kasm-rollback-patch",
      version: 1,
      basePatchHash: patch.id,
      baseSceneHash: commandSpec.sceneHash,
      ops: ops.map((op) => ({
        op: "SetProperty",
        targetHash: op.targetHash,
        targetKind: op.targetKind,
        targetId: op.targetId,
        key: op.key,
        before: op.value,
        value: op.before,
        mode: "restore",
      })),
    };
    rollback.id = boomKasmObjectHash("world-rollback-patch-v1", rollback);
    patch.rollbackPatchHash = rollback.id;
    return { patch, rollback };
  }

  function boomKasmPatchOpValue(patch, key) {
    return (patch?.ops || []).find((op) => op.key === key)?.value;
  }

  function boomKasmPatchAppliedResult(command, detail = {}) {
    return boomToolResult(command, true, { ...detail, appliedBy: "world_patch" });
  }

  function applyBoomWorldPatchBundle(worldPatchBundle, commandSpec) {
    const patch = worldPatchBundle?.patch;
    const command = commandSpec?.command?.type || "boom.unknown";
    const payload = commandSpec?.command?.payload || {};
    if (!patch?.ops?.length) return boomToolResult(command, false, { error: "empty_world_patch" });

    if (command === "boom.kasm.set_mode") {
      const args = String(payload.args || "").trim();
      const mode = ["create", "inspect", "optimize", "prove"].includes(args) ? args : "inspect";
      setBoomWorkspaceMode(boomKasmPatchOpValue(patch, "workspaceMode") || (mode === "optimize" ? "slicer" : "design"));
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { mode });
    }

    if (command === "boom.kasm.set_graph_view") {
      const viewId = boomKasmPatchOpValue(patch, "kasmGraphView") || payload.viewId || "world";
      boomScene.kasmGraphView = BOOM_KASM_GRAPH_VIEWS.some((entry) => entry.id === viewId) ? viewId : "world";
      renderBoomSidebar();
      return boomKasmPatchAppliedResult(command, { viewId: boomScene.kasmGraphView });
    }

    if (command === "boom.kasm.select_hash") {
      boomScene.selectedKasmHash = String(boomKasmPatchOpValue(patch, "selectedKasmHash") || payload.hash || "");
      renderBoomSidebar();
      return boomKasmPatchAppliedResult(command, { selectedKasmHash: boomScene.selectedKasmHash });
    }

    if (command === "boom.scene.workspace_mode") {
      setBoomWorkspaceMode(boomKasmPatchOpValue(patch, "workspaceMode") || "design");
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { workspaceMode: boomScene.workspaceMode });
    }

    if (command === "boom.viewport.set_edit_mode") {
      const nextMode = BOOM_EDIT_MODES.some((entry) => entry.id === payload.mode) ? payload.mode : "object";
      boomScene.editMode = nextMode;
      if (boomScene.editMode === "object") clearBoomComponentSelection();
      else if (boomScene.componentSelection?.type !== boomScene.editMode) clearBoomComponentSelection();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { editMode: boomScene.editMode });
    }

    if (command === "boom.scene.select_item") {
      const item = boomItemById(payload.itemId || payload.id || "");
      if (!item) return boomToolResult(command, false, { error: "item_not_found" });
      boomScene.activeId = item.id;
      const preserveComponentSelection = !!payload.preserveMeshComponentSelection && isBoomMeshItem(item);
      if (!preserveComponentSelection) {
        clearBoomComponentSelection();
        clearBoomRegionSelection();
      }
      if (boomScene.propertyTab === "scene") boomScene.propertyTab = "object";
      if (boomScene.propertyTab === "modifiers" && !isBoomMeshItem(item)) boomScene.propertyTab = "object";
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { item: { id: item.id, name: item.name, type: item.type } });
    }

    if (command === "boom.modifier.add" || command === "boom.modifier.apply") {
      const activeMesh = activeBoomMeshItem();
      if (!activeMesh) return boomToolResult(command, false, { error: "mesh_not_found" });
      const preset = boomModifierPresetByType(payload.type);
      if (!preset) return boomToolResult(command, false, { error: "unknown_modifier_type", type: payload.type || "" });
      const modifier = createBoomModifier(preset);
      modifier.id = `modifier-${String(commandSpec.id || "").slice(0, 12)}`;
      boomApplyModifierPayload(modifier, payload);
      const modifiers = ensureBoomItemModifiers(activeMesh);
      const existing = modifiers.find((entry) => entry.id === modifier.id);
      if (!existing) modifiers.push(modifier);
      refreshBoomMeshPreview(activeMesh);
      boomScene.propertyTab = "modifiers";
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, {
        modifier: {
          id: modifier.id,
          type: modifier.type,
          title: modifier.title,
        },
      });
    }

    if (command === "boom.slicer.set_workflow") {
      const nextWorkflow = ["prepare", "preview", "print"].includes(String(payload.workflow || "")) ? String(payload.workflow) : "prepare";
      boomScene.slicer.workflow = nextWorkflow;
      setBoomWorkspaceMode("slicer");
      rebuildBoomSlicerPreview();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { workflow: boomScene.slicer.workflow });
    }

    if (command === "boom.query.region_from_selection" || command === "boom.query.volume_region_from_selection") {
      const region = command === "boom.query.volume_region_from_selection"
        ? boomKasmQueries?.volumeRegionFromSelection?.()
        : boomKasmQueries?.regionFromSelection?.();
      if (payload.activate !== false) setBoomRegionSelection(region);
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { region: region ? boomRegionSummary(region) : null });
    }

    if (command === "boom.region.clear") {
      clearBoomRegionSelection();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomKasmPatchAppliedResult(command, { cleared: true });
    }

    if (command === "boom.animation.play" || command === "boom.animation.pause") {
      if (!boomAnimationState) return boomToolResult(command, false, { error: "no_animation" });
      const playing = command === "boom.animation.play";
      boomAnimationState.playing = playing;
      if (playing) {
        boomAnimationState.startedAtMs = 0;
        requestBoomRender("animation-play", 250);
      } else {
        boomRenderContinuousUntil = 0;
        requestBoomRender("animation-pause");
      }
      renderBoomSidebar();
      return boomKasmPatchAppliedResult(command, { playing });
    }

    return boomToolResult(command, false, { error: "unsupported_world_patch_command" });
  }

  function applyBoomRollbackPatch(rollbackPatch) {
    if (!rollbackPatch?.ops?.length) return boomToolResult("boom.kasm.rollback", false, { error: "rollback_not_found" });
    let applied = 0;
    for (const op of rollbackPatch.ops) {
      if (op.op !== "SetProperty") continue;
      if (op.targetKind === "scene") {
        if (op.key === "workspaceMode") {
          setBoomWorkspaceMode(op.value || "design");
          applied += 1;
        } else if (op.key === "editMode") {
          boomScene.editMode = BOOM_EDIT_MODES.some((entry) => entry.id === op.value) ? op.value : "object";
          applied += 1;
        } else if (op.key === "activeId") {
          const item = boomItemById(op.value || "");
          if (item) {
            boomScene.activeId = item.id;
            applied += 1;
          }
        } else if (op.key === "componentSelection") {
          if (op.value) setBoomComponentSelection(op.value);
          else clearBoomComponentSelection();
          applied += 1;
        } else if (op.key === "regionSelection") {
          if (op.value) setBoomRegionSelection(op.value);
          else clearBoomRegionSelection();
          applied += 1;
        } else if (op.key === "slicer.workflow") {
          boomScene.slicer.workflow = ["prepare", "preview", "print"].includes(String(op.value || "")) ? String(op.value) : "prepare";
          applied += 1;
        }
      } else if (op.targetKind === "entity" && op.key === "modifiers") {
        const item = boomItemById(op.targetId);
        if (isBoomMeshItem(item)) {
          const modifiers = ensureBoomItemModifiers(item);
          modifiers.length = Math.max(0, Math.min(modifiers.length, Number(op.value || 0)));
          refreshBoomMeshPreview(item);
          applied += 1;
        }
      } else if (op.targetKind === "animation" && op.key === "playing" && boomAnimationState) {
        boomAnimationState.playing = !!op.value;
        if (!boomAnimationState.playing) boomRenderContinuousUntil = 0;
        applied += 1;
      } else if (op.targetKind === "ui" && op.key === "kasmGraphView") {
        boomScene.kasmGraphView = BOOM_KASM_GRAPH_VIEWS.some((entry) => entry.id === op.value) ? op.value : "world";
        applied += 1;
      } else if (op.targetKind === "ui" && op.key === "selectedKasmHash") {
        boomScene.selectedKasmHash = String(op.value || "");
        applied += 1;
      }
    }
    renderBoomSidebar();
    renderBoomViewportHud();
    return boomToolResult("boom.kasm.rollback", applied > 0, {
      rollbackHash: rollbackPatch.id || "",
      basePatchHash: rollbackPatch.basePatchHash || "",
      opsApplied: applied,
      sceneHash: boomKasmCurrentSceneHash(),
      appliedBy: "world_rollback_patch",
    });
  }

  function resolveBoomRollbackPatch(targetHash = "last") {
    const target = String(targetHash || "last").trim() || "last";
    if (target === "last") return boomKasmRollbackHistory[boomKasmRollbackHistory.length - 1] || null;
    const resolved = resolveBoomKasmHash(target)?.record || null;
    if (!resolved) return null;
    if (resolved.kind === "kasm-rollback-patch") return resolved;
    if (resolved.kind === "kasm-world-patch") {
      return resolveBoomKasmHash(resolved.rollbackPatchHash)?.record || null;
    }
    if (resolved.kind === "kasm-run-record") {
      const patchHash = (resolved.outputHashes || [])
        .map((hash) => resolveBoomKasmHash(hash)?.record)
        .find((record) => record?.kind === "kasm-world-patch")?.id;
      return patchHash ? resolveBoomRollbackPatch(patchHash) : null;
    }
    return null;
  }

  function parseBoomMetricSlash(args = "") {
    const tokens = String(args || "").trim().split(/\s+/).filter(Boolean);
    const action = tokens[0] || "run";
    const metricName = tokens[1] && !tokens[1].startsWith("--") ? tokens[1] : "scene_complexity";
    const targetIndex = tokens.findIndex((token) => token === "--target");
    const templateIndex = tokens.findIndex((token) => token === "--template");
    const thresholdIndex = tokens.findIndex((token) => token === "--threshold");
    return {
      action,
      metricName: normalizeBoomMetricName(metricName),
      targetHash: targetIndex >= 0 ? tokens[targetIndex + 1] || "last" : "last",
      template: templateIndex >= 0 ? tokens[templateIndex + 1] || metricName : metricName,
      threshold: thresholdIndex >= 0 ? Number(tokens[thresholdIndex + 1]) : null,
    };
  }

  function executeBoomKasmCommandSpec(command, payload, applyFn, options = {}) {
    const started = boomNowMs();
    const specResult = boomCachedCompute(
      "kasm_command_spec",
      { command, payload, sceneHash: boomKasmCurrentSceneHash(), rawInput: options.rawInput || command },
      1,
      "commands",
      () => buildBoomKasmCommandSpec(command, payload, options)
    );
    const commandSpec = specResult.value;
    rememberBoomKasmHash(commandSpec, "command-spec");
    boomKasmSpineStats.commandSpecs += specResult.status === "MISS" ? 1 : 0;
    const programResult = boomCachedCompute(
      "kasm_bytecode_program",
      { commandSpecHash: commandSpec.id, payloadHash: commandSpec.payloadHash },
      1,
      "programs",
      () => buildBoomKasmBytecodeProgram(commandSpec)
    );
    const program = programResult.value;
    rememberBoomKasmHash(program, "bytecode-program");
    boomKasmSpineStats.bytecodePrograms += programResult.status === "MISS" ? 1 : 0;
    const sandboxResult = boomCachedCompute(
      "kasm_sandbox_matrix",
      { permissionsHash: commandSpec.permissionsHash, budgetHash: commandSpec.budgetHash },
      1,
      "sandboxes",
      () => buildBoomKasmSandboxMatrix(commandSpec)
    );
    const sandbox = sandboxResult.value;
    rememberBoomKasmHash(sandbox, "sandbox-matrix");
    boomKasmSpineStats.sandboxMatrices += sandboxResult.status === "MISS" ? 1 : 0;
    const worldPatchBundle = buildBoomWorldPatchBundle(commandSpec);

    let result;
    try {
      result = worldPatchBundle?.patch && options.applyWorldPatch !== false
        ? applyBoomWorldPatchBundle(worldPatchBundle, commandSpec)
        : applyFn(commandSpec, program, sandbox);
    } catch (err) {
      result = boomToolResult(command, false, { error: String(err?.message || err || "kasm_apply_failed") });
    }
    if (!result || typeof result !== "object") {
      result = boomToolResult(command, false, { error: "invalid_kasm_output" });
    }
    const outputHash = boomKasmObjectHash("kasm-output-v1", {
      ok: !!result.ok,
      tool: result.tool || command,
      detail: result.detail || {},
      sceneHash: boomKasmCurrentSceneHash(),
    });
    rememberBoomKasmHash({
      kind: "kasm-output-hash",
      version: 1,
      id: outputHash,
      commandHash: commandSpec.id,
      sceneHash: boomKasmCurrentSceneHash(),
      ok: !!result.ok,
    }, "output-hash");
    let worldPatch = null;
    let rollbackPatch = null;
    if (result.ok && worldPatchBundle?.patch) {
      worldPatch = {
        ...worldPatchBundle.patch,
        expectedSceneHash: boomKasmCurrentSceneHash(),
        outputHash,
      };
      rollbackPatch = worldPatchBundle.rollback;
      boomKasmSpineStats.worldPatches += 1;
      boomKasmSpineStats.rollbackPatches += rollbackPatch ? 1 : 0;
      rememberBoomKasmRecord(boomKasmPatchHistory, worldPatch);
      if (rollbackPatch) rememberBoomKasmRecord(boomKasmRollbackHistory, rollbackPatch);
      emitBoomAudit("kasm_world_patch", "DIRECT", worldPatch.id, boomNowMs() - started, worldPatch.ops.length, "ops", {
        command,
        commandSpecHash: commandSpec.id,
        baseSceneHash: worldPatch.baseSceneHash,
        expectedSceneHash: worldPatch.expectedSceneHash,
        rollbackPatchHash: worldPatch.rollbackPatchHash,
      });
    }
    const extraOutputHashes = Array.isArray(result?.detail?.outputHashes)
      ? result.detail.outputHashes.filter(Boolean)
      : [];
    const metricRecords = runBoomDefaultMetrics({
      commandSpec,
      program,
      sandbox,
      result,
      outputHash,
      worldPatch,
      rollbackPatch,
      target: worldPatch || result,
      targetHash: worldPatch?.id || outputHash,
      started,
    });
    const metricHashes = metricRecords.map((metric) => metric.id);
    const outputHashes = [...new Set(worldPatch ? [outputHash, worldPatch.id, ...extraOutputHashes] : [outputHash, ...extraOutputHashes])];
    const logHash = boomKasmObjectHash("kasm-run-log-v1", {
      commandSpecHash: commandSpec.id,
      outputHash,
      worldPatchHash: worldPatch?.id || null,
      metricHashes,
      elapsedMs: Number((boomNowMs() - started).toFixed(3)),
    });
    const runRecord = {
      kind: "kasm-run-record",
      version: 1,
      commandHash: commandSpec.id,
      programHash: program.id,
      inputHashes: commandSpec.inputHashes,
      outputHashes,
      metricHashes,
      proofHash: "",
      logHash,
      status: result.ok ? "ok" : "error",
    };
    runRecord.id = boomKasmObjectHash("run-record-v1", runRecord);
    const proofRecord = {
      kind: "kasm-proof-record",
      version: 1,
      commandHash: commandSpec.id,
      inputHashes: commandSpec.inputHashes,
      programHashes: [program.id],
      sandboxHash: sandbox.id,
      outputHashes,
      metricHashes,
      environmentHash: boomKasmObjectHash("environment-v1", {
        userAgent: typeof navigator !== "undefined" ? navigator.userAgent : "",
        renderer: !!gl,
        cacheMaxBytes: BOOM_COMPUTE_CACHE_MAX_BYTES,
      }),
    };
    proofRecord.id = boomKasmObjectHash("proof-record-v1", proofRecord);
    runRecord.proofHash = proofRecord.id;
    boomKasmSpineStats.runRecords += 1;
    boomKasmSpineStats.proofRecords += 1;
    rememberBoomKasmRecord(boomKasmRunHistory, runRecord);
    rememberBoomKasmRecord(boomKasmProofHistory, proofRecord);
    emitBoomAudit("kasm_run_record", "DIRECT", runRecord.id, boomNowMs() - started, 1, "runs", {
      command,
      commandSpecHash: commandSpec.id,
      programHash: program.id,
      sandboxHash: sandbox.id,
      proofHash: proofRecord.id,
      outputHash,
      status: runRecord.status,
      metricHashes,
    });
    result.kasm = {
      commandSpecHash: commandSpec.id,
      programHash: program.id,
      sandboxHash: sandbox.id,
      runHash: runRecord.id,
      proofHash: proofRecord.id,
      outputHash,
      metricHashes,
      worldPatchHash: worldPatch?.id || null,
      rollbackPatchHash: rollbackPatch?.id || null,
    };
    result.detail = { ...(result.detail || {}), kasm: result.kasm };
    exposeBoomAuditState();
    return result;
  }

  function runBoomSlashCommand(rawInput = "", options = {}) {
    const raw = String(rawInput || "").trim();
    const [head, ...rest] = raw.split(/\s+/);
    const slash = head.startsWith("/") ? head.slice(1) : "prompt";
    const args = rest.join(" ");
    const worldRollback = slash === "world" && /^rollback\b/.test(args);
    const metricRequest = slash === "metric" ? parseBoomMetricSlash(args) : null;
    const createMetricName = slash === "create_metric" ? normalizeBoomMetricName(args.split(/\s+/)[0] || "scene_complexity") : "";
    const createProgramRequest = slash === "create_program" ? parseBoomProgramSlash(args) : null;
    const programRequest = slash === "program" ? parseBoomProgramSlash(args) : null;
    const matrixRequest = slash === "matrix" ? parseBoomMatrixSlash(args) : null;
    const skillRequest = slash === "skill" ? parseBoomSkillSlash(args) : null;
    const renderRequest = slash === "render" ? parseBoomRenderSlash(args) : null;
    const assetRequest = slash === "asset" ? parseBoomAssetSlash(args) : null;
    const targetHash = worldRollback
      ? args.replace(/^rollback\b/, "").trim() || "last"
      : metricRequest?.targetHash
        ? metricRequest.targetHash
      : skillRequest?.targetHash
        ? skillRequest.targetHash
      : args.trim() || "last";
    const payload = {
      raw,
      args,
      targetHash,
      metric: metricRequest,
      metricName: metricRequest?.metricName || createMetricName,
      program: programRequest || createProgramRequest,
      matrix: matrixRequest,
      skill: skillRequest,
      render: renderRequest,
      asset: assetRequest,
      source: options.modelSource || "user",
      facadeHash: options.facadeHash || "",
      mcpToolName: options.mcpToolName || "",
    };
    const command = slash === "prove"
      ? "boom.kasm.prove"
      : slash === "explain"
        ? "boom.kasm.explain_hash"
        : worldRollback
          ? "boom.kasm.rollback"
          : slash === "create_metric"
            ? "boom.kasm.create_metric"
            : slash === "metric" && metricRequest?.action === "run"
              ? "boom.kasm.run_metric"
              : slash === "create_program"
                ? "boom.kasm.create_program"
                : slash === "program" && ["run", "profile", "test"].includes(programRequest?.action)
                  ? "boom.kasm.run_program"
                  : slash === "matrix" && matrixRequest?.action === "run"
                    ? "boom.kasm.run_matrix"
                    : slash === "skill" && skillRequest?.action === "create"
                      ? "boom.kasm.create_skill"
                      : slash === "skill" && skillRequest?.action === "run"
                        ? "boom.kasm.run_skill"
                        : slash === "skill" && skillRequest?.action === "promote"
                          ? "boom.kasm.promote_skill"
                          : slash === "render"
                            ? "boom.kasm.render_ir"
                            : slash === "asset"
                              ? "boom.kasm.asset_pages"
                              : slash === "cache"
                                ? "boom.kasm.cache_stats"
                                : slash === "status"
                                  ? "boom.kasm.status"
                                  : slash === "mode"
                                    ? "boom.kasm.set_mode"
                                    : "boom.kasm.prompt";
    const commandUsesCompute = (command === "boom.kasm.create_program" && isBoomComputeProgramTemplate(createProgramRequest?.template))
      || (command === "boom.kasm.run_program" && isBoomComputeProgramTemplate((resolveBoomProgramSpec(programRequest?.programName)?.template) || programRequest?.programName));
    return executeBoomKasmCommandSpec(command, payload, () => {
      if (command === "boom.kasm.prove") {
        const resolved = args ? resolveBoomKasmHash(targetHash)?.record || null : null;
        const proof = resolved?.kind === "kasm-proof-record"
          ? resolved
          : resolved?.proofHash
            ? resolveBoomKasmHash(resolved.proofHash)?.record || null
            : boomKasmProofHistory[boomKasmProofHistory.length - 1] || null;
        return boomToolResult(command, !!proof, { proof });
      }
      if (command === "boom.kasm.explain_hash") {
        const explanation = explainBoomKasmHash(targetHash);
        return boomToolResult(command, !!explanation, { explanation, targetHash });
      }
      if (command === "boom.kasm.rollback") {
        const rollback = resolveBoomRollbackPatch(targetHash);
        if (!rollback) return boomToolResult(command, false, { error: "rollback_not_found", targetHash });
        return applyBoomRollbackPatch(rollback);
      }
      if (command === "boom.kasm.create_metric") {
        const spec = ensureBoomMetricSpec(payload.metricName, {
          template: payload.metricName,
          threshold: Number.isFinite(Number(metricRequest?.threshold)) ? Number(metricRequest.threshold) : null,
        });
        return boomToolResult(command, true, { metricSpec: spec });
      }
      if (command === "boom.kasm.run_metric") {
        const record = runBoomKasmMetric(payload.metricName, targetHash, {
          targetHash,
          target: resolveBoomKasmHash(targetHash)?.record || null,
          started: boomNowMs(),
        });
        return boomToolResult(command, true, { metric: record });
      }
      if (command === "boom.kasm.create_program") {
        const spec = ensureBoomProgramSpec(createProgramRequest.programName, {
          template: createProgramRequest.template,
          rawInput: raw,
        });
        const computeProgram = isBoomComputeProgramTemplate(spec.template)
          ? ensureBoomComputeProgram(spec.name, { template: spec.template, inputHash: payload.targetHash })
          : null;
        return boomToolResult(command, true, {
          programSpec: spec,
          computeProgram,
          outputHashes: [spec.id, computeProgram?.id].filter(Boolean),
        });
      }
      if (command === "boom.kasm.run_program") {
        const run = runBoomProgramSpec(programRequest.programName, {
          inputHash: programRequest.inputHash === "last"
            ? boomKasmRunHistory[boomKasmRunHistory.length - 1]?.id || boomKasmCurrentSceneHash()
            : programRequest.inputHash,
          started: boomNowMs(),
        });
        return boomToolResult(command, true, { programRun: run, outputHashes: run.outputHashes });
      }
      if (command === "boom.kasm.run_matrix") {
        const matrixRun = runBoomKasmMatrix(matrixRequest.programName, {
          variants: matrixRequest.variants,
          metrics: matrixRequest.metrics,
          rawInput: raw,
        });
        return boomToolResult(command, true, { matrixRun });
      }
      if (command === "boom.kasm.create_skill") {
        const spec = ensureBoomSkillSpec(skillRequest.skillName, {
          metrics: skillRequest.metrics,
          rawInput: raw,
        });
        return boomToolResult(command, true, { skillSpec: spec });
      }
      if (command === "boom.kasm.run_skill") {
        const run = runBoomSkillSpec(skillRequest.skillName, {
          inputHash: skillRequest.targetHash,
          started: boomNowMs(),
        });
        return boomToolResult(command, true, { skillRun: run });
      }
      if (command === "boom.kasm.promote_skill") {
        const promotedFromHash = skillRequest.targetHash === "current"
          ? boomKasmRunHistory[boomKasmRunHistory.length - 1]?.id || null
          : skillRequest.targetHash;
        const spec = ensureBoomSkillSpec(skillRequest.skillName, {
          promotedFromHash,
          metrics: skillRequest.metrics,
          rawInput: raw,
        });
        return boomToolResult(command, true, { skillSpec: spec, promotedFromHash });
      }
      if (command === "boom.kasm.render_ir") {
        const projection = buildBoomRenderIR({ mode: renderRequest.mode });
        requestBoomRender(`kasm-render-${renderRequest.action || "frame"}`);
        return boomToolResult(command, true, {
          renderIR: projection.renderIR,
          assetPages: projection.assetPages,
          outputHashes: [projection.renderIR.id, ...projection.assetPages.map((page) => page.id)],
        });
      }
      if (command === "boom.kasm.asset_pages") {
        const geoCluster = assetRequest.action === "meshletize"
          ? buildBoomGeoClusterAsset({
              target: assetRequest.target,
              maxTris: assetRequest.maxTris,
              lod: assetRequest.lod,
            })
          : null;
        const rawPages = geoCluster
          ? boomAssetPagesFromGeoCluster(geoCluster)
          : buildBoomAssetPagesForScene({ kind: assetRequest.kind });
        const residency = buildBoomAssetResidencyPlan(rawPages, {
          action: assetRequest.action,
          target: assetRequest.target,
          hotPages: assetRequest.hotPages,
        });
        const pages = residency.pages;
        const assetPack = buildBoomAssetPack(pages, { name: geoCluster ? "geocluster_asset_pack" : `asset_${assetRequest.action || "scan"}_pack` });
        const metricRecords = [
          ...["asset_ram_cost", "asset_vram_cost", "asset_evictable_pages"].map((metricName) => runBoomKasmMetric(metricName, residency.plan.id, {
            targetHash: residency.plan.id,
            target: residency.plan,
            assetResidencyPlan: residency.plan,
            started: boomNowMs(),
          })),
          ...(geoCluster
            ? ["cluster_vram_cost", "cluster_lod_error", "cluster_draw_cost", "cluster_stream_cost"].map((metricName) => runBoomKasmMetric(metricName, geoCluster.id, {
                targetHash: geoCluster.id,
                target: geoCluster,
                geoCluster,
                started: boomNowMs(),
              }))
            : []),
        ];
        return boomToolResult(command, true, {
          assetPack,
          assetPages: pages,
          assetResidencyPlan: residency.plan,
          geoCluster,
          metricHashes: metricRecords.map((metric) => metric.id),
          outputHashes: [assetPack.id, residency.plan.id, residency.plan.residencyHash, geoCluster?.id, ...(geoCluster?.clusterPageHashes || []), ...pages.map((page) => page.id), ...metricRecords.map((metric) => metric.id)].filter(Boolean),
        });
      }
      if (command === "boom.kasm.cache_stats") {
        return boomToolResult(command, true, { summary: boomCacheStatusSummary() });
      }
      if (command === "boom.kasm.status") {
        return boomToolResult(command, true, { run: boomKasmRunHistory[boomKasmRunHistory.length - 1] || null, stats: boomKasmSpineStats });
      }
      if (command === "boom.kasm.set_mode") {
        const mode = ["create", "inspect", "optimize", "prove"].includes(args) ? args : "inspect";
        boomScene.workspaceMode = mode === "optimize" ? "slicer" : "design";
        renderBoomSidebar();
        renderBoomViewportHud();
        return boomToolResult(command, true, { mode });
      }
      return boomToolResult(command, false, { error: "prompt_requires_program_template", raw });
    }, {
      rawInput: raw || command,
      modelSource: options.modelSource || null,
      permissions: { world: command === "boom.kasm.set_mode", assets: command === "boom.kasm.asset_pages" || commandUsesCompute, filesystem: false, shell: false, network: false, renderer: command === "boom.kasm.render_ir", gpuCompute: commandUsesCompute },
    });
  }

  function applyBoomToolDirect(toolName, payload = {}) {
    const normalized = String(toolName || "").trim();
    if (!normalized) return boomToolResult("boom.unknown", false, { error: "missing_tool" });
    if (normalized === "boom.query.region_from_selection") {
      const region = boomKasmQueries?.regionFromSelection?.() || null;
      if (payload.activate !== false) setBoomRegionSelection(region);
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, !!region, {
        region: region ? boomRegionSummary(region) : null,
      });
    }
    if (normalized === "boom.query.volume_region_from_selection") {
      const region = boomKasmQueries?.volumeRegionFromSelection?.() || null;
      if (payload.activate !== false) setBoomRegionSelection(region);
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, !!region, {
        region: region ? boomRegionSummary(region) : null,
      });
    }
    if (normalized === "boom.region.clear") {
      clearBoomRegionSelection();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, { cleared: true });
    }
    if (normalized === "boom.scene.workspace_mode") {
      setBoomWorkspaceMode(payload.mode || payload.workspaceMode || "design");
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, {
        workspaceMode: boomScene.workspaceMode,
      });
    }
    if (normalized === "boom.viewport.set_edit_mode") {
      const nextMode = BOOM_EDIT_MODES.some((entry) => entry.id === payload.mode) ? payload.mode : "object";
      boomScene.editMode = nextMode;
      if (boomScene.editMode === "object") clearBoomComponentSelection();
      else if (boomScene.componentSelection?.type !== boomScene.editMode) clearBoomComponentSelection();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, {
        editMode: boomScene.editMode,
      });
    }
    if (normalized === "boom.scene.select_item") {
      const item = boomItemById(payload.itemId || payload.id || "");
      if (!item) return boomToolResult(normalized, false, { error: "item_not_found" });
      boomScene.activeId = item.id;
      const preserveComponentSelection = !!payload.preserveMeshComponentSelection && isBoomMeshItem(item);
      if (!preserveComponentSelection) {
        clearBoomComponentSelection();
        clearBoomRegionSelection();
      }
      if (boomScene.propertyTab === "scene") boomScene.propertyTab = "object";
      if (boomScene.propertyTab === "modifiers" && !isBoomMeshItem(item)) boomScene.propertyTab = "object";
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, {
        item: { id: item.id, name: item.name, type: item.type },
      });
    }
    if (normalized === "boom.modifier.add" || normalized === "boom.modifier.apply") {
      const activeMesh = activeBoomMeshItem();
      if (!activeMesh) return boomToolResult(normalized, false, { error: "mesh_not_found" });
      const preset = boomModifierPresetByType(payload.type);
      if (!preset) {
        return boomToolResult(normalized, false, { error: "unknown_modifier_type", type: payload.type || "" });
      }
      const modifier = createBoomModifier(preset);
      boomApplyModifierPayload(modifier, payload);
      ensureBoomItemModifiers(activeMesh).push(modifier);
      refreshBoomMeshPreview(activeMesh);
      boomScene.propertyTab = "modifiers";
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, {
        modifier: {
          id: modifier.id,
          type: modifier.type,
          title: modifier.title,
        },
      });
    }
    if (normalized === "boom.slicer.set_workflow") {
      const nextWorkflow = ["prepare", "preview", "print"].includes(String(payload.workflow || "")) ? String(payload.workflow) : "prepare";
      boomScene.slicer.workflow = nextWorkflow;
      setBoomWorkspaceMode("slicer");
      rebuildBoomSlicerPreview();
      renderBoomSidebar();
      renderBoomViewportHud();
      return boomToolResult(normalized, true, {
        workflow: boomScene.slicer.workflow,
      });
    }
    if (normalized === "boom.animation.export_js") {
      const exported = exportBoomAnimationBridge("js");
      return boomToolResult(normalized, !!exported, exported || { error: "no_mesh" });
    }
    if (normalized === "boom.animation.export_json") {
      const exported = exportBoomAnimationBridge("json");
      return boomToolResult(normalized, !!exported, exported || { error: "no_mesh" });
    }
    if (normalized === "boom.animation.play") {
      if (boomAnimationState) {
        boomAnimationState.playing = true;
        boomAnimationState.startedAtMs = 0;
        renderBoomSidebar();
        requestBoomRender("animation-play", 250);
        return boomToolResult(normalized, true, { playing: true });
      }
      return boomToolResult(normalized, false, { error: "no_animation" });
    }
    if (normalized === "boom.animation.pause") {
      if (boomAnimationState) {
        boomAnimationState.playing = false;
        boomRenderContinuousUntil = 0;
        renderBoomSidebar();
        requestBoomRender("animation-pause");
        return boomToolResult(normalized, true, { playing: false });
      }
      return boomToolResult(normalized, false, { error: "no_animation" });
    }
    return boomToolResult(normalized, false, { error: "unsupported_tool" });
  }

  function executeBoomTool(toolName, payload = {}) {
    const normalized = String(toolName || "").trim() || "boom.unknown";
    return executeBoomKasmCommandSpec(
      normalized,
      payload,
      () => applyBoomToolDirect(normalized, payload),
      { rawInput: normalized }
    );
  }

  function boomModeHeadline() {
    const current = BOOM_EDIT_MODES.find((mode) => mode.id === boomScene.editMode);
    return current?.title || "Object";
  }

  function boomSlicerEstimate() {
    const slicer = boomScene.slicer || {};
    const mesh = activeBoomMeshItem();
    const passCount = sceneMesh ? Math.max(1, meshRenderPasses(sceneMesh).length) : 1;
    const baseFaces = Number(mesh?.meta?.faceCount || sceneMesh?.faceCount || 0);
    const heightSpan = Number(sceneMesh?.bounds?.span?.[2] || 24);
    const layerHeight = Math.max(0.08, Number(slicer.layerHeight || 0.2));
    const speed = Math.max(40, Number(slicer.printSpeed || 160));
    const infill = Math.max(0, Number(slicer.infillDensity || 0));
    const walls = Math.max(1, Number(slicer.wallLoops || 2));
    const projectedLayers = Math.max(1, Math.round((heightSpan * 2.4) / layerHeight));
    const printMinutes = Math.max(
      12,
      Math.round((projectedLayers * 0.42) + (baseFaces * passCount * 0.0038) * (160 / speed) + (infill * 0.55) + walls * 6)
    );
    const materialGrams = Math.max(
      4,
      Math.round((baseFaces * passCount * 0.0014) + (infill * 0.32) + walls * 2.3)
    );
    return {
      projectedLayers,
      printMinutes,
      materialGrams,
      passCount,
    };
  }

  function boomDesignTabs() {
    return BOOM_PROPERTY_TABS.filter((tab) => tab.id !== "slicer");
  }

  function boomVisiblePropertyTabs() {
    return boomScene.workspaceMode === "slicer"
      ? BOOM_PROPERTY_TABS.filter((tab) => tab.id === "slicer")
      : boomDesignTabs();
  }

  function setBoomWorkspaceMode(mode) {
    boomScene.workspaceMode = mode === "slicer" ? "slicer" : "design";
    if (boomScene.workspaceMode === "slicer") {
      boomScene.propertyTab = "slicer";
    } else if (boomScene.propertyTab === "slicer") {
      boomScene.propertyTab = "object";
    }
  }

  async function refreshBoomPrinterDiscovery(force = false) {
    if (boomScene.workspaceMode !== "slicer" && !force) return null;
    if (boomScene.slicer.discoveryState === "scanning" && !force) return null;
    boomScene.slicer.discoveryState = "scanning";
    renderBoomSidebar();
    const response = await backendInvoke("banger_detect_printers");
    if (response) {
      boomScene.slicer.profiles = Array.isArray(response.profiles) ? response.profiles : [];
      boomScene.slicer.devices = Array.isArray(response.detected) ? response.detected : [];
      boomScene.slicer.discoveryWarnings = Array.isArray(response.warnings) ? response.warnings : [];
      boomScene.slicer.discoveryBackend = String(response.backend || "");
      if (Array.isArray(response.profiles) && response.profiles.length) {
        const existing = response.profiles.some((profile) => profile.label === boomScene.slicer.printerProfile);
        if (!existing && response.profiles[0]?.label) {
          boomScene.slicer.printerProfile = response.profiles[0].label;
        }
      }
      boomScene.slicer.discoveryState = "ready";
    } else {
      boomScene.slicer.discoveryWarnings = ["Printer discovery unavailable from backend."];
      boomScene.slicer.discoveryState = "error";
    }
    renderBoomSidebar();
    renderBoomViewportHud();
    return response;
  }

  function boomImportedMeshStats(item) {
    const vertices = Number(item?.meta?.vertexCount || 0);
    const faces = Number(item?.meta?.faceCount || 0);
    const bits = [];
    if (vertices > 0) bits.push(`${vertices.toLocaleString("en-US")} verts`);
    if (faces > 0) bits.push(`${faces.toLocaleString("en-US")} faces`);
    return bits.join(" · ");
  }

  function boomItemById(id) {
    return boomScene.items.find((item) => item.id === id) || boomScene.items[0] || null;
  }

  function kasmHashString(input) {
    let hash = 2166136261;
    const source = String(input || "");
    for (let i = 0; i < source.length; i += 1) {
      hash ^= source.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    return `kasm-${(hash >>> 0).toString(16).padStart(8, "0")}`;
  }

  function kasmQuantize(value) {
    return Math.round(Number(value || 0) * 100000);
  }

  function kasmQuantizedCoordNode(axis, value) {
    const quantized = kasmQuantize(value);
    return {
      axis,
      value: Number(Number(value || 0).toFixed(5)),
      quantized,
      hash: kasmHashString(`coord-axis|${axis}|${quantized}`),
    };
  }

  function kasmSpatialCellNode(position, cellSize) {
    const x = Math.floor(Number(position?.[0] || 0) / cellSize);
    const y = Math.floor(Number(position?.[1] || 0) / cellSize);
    const z = Math.floor(Number(position?.[2] || 0) / cellSize);
    const key = `${cellSize}|${x}|${y}|${z}`;
    return {
      cellSize,
      index: [x, y, z],
      hash: kasmHashString(`coord-cell|${key}`),
    };
  }

  function boomSpatialCellHashesForPoint(position) {
    return [1, 4, 16].map((cellSize) => kasmSpatialCellNode(position, cellSize));
  }

  function serializeModifierParams(modifier) {
    if (!modifier) return {};
    const out = {};
    if (modifier.axis != null) out.axis = modifier.axis;
    if (modifier.count != null) out.count = Number(modifier.count);
    if (modifier.offset != null) out.offset = Number(modifier.offset);
    if (modifier.amount != null) out.amount = Number(modifier.amount);
    if (modifier.width != null) out.width = Number(modifier.width);
    if (modifier.levels != null) out.levels = Number(modifier.levels);
    if (modifier.thickness != null) out.thickness = Number(modifier.thickness);
    return out;
  }

  function buildBoomKasmTopology(meshData, item) {
    if (!meshData?.pos?.length || !item) return null;
    const vertexMap = new Map();
    const edgeMap = new Map();
    const cellMap = new Map();
    const vertices = [];
    const edges = [];
    const faces = [];
    const coordinates = [];
    const cells = [];
    const pos = meshData.pos;

    const ensureCell = (position, cellSize) => {
      const x = Math.floor(Number(position?.[0] || 0) / cellSize);
      const y = Math.floor(Number(position?.[1] || 0) / cellSize);
      const z = Math.floor(Number(position?.[2] || 0) / cellSize);
      const key = `${cellSize}|${x}|${y}|${z}`;
      if (cellMap.has(key)) return cellMap.get(key);
      const node = {
        id: `cell:${cellMap.size}`,
        tag: "cell",
        cellSize,
        index: [x, y, z],
        hash: kasmHashString(`coord-cell|${key}`),
      };
      cellMap.set(key, node);
      cells.push(node);
      return node;
    };

    const ensureVertex = (x, y, z) => {
      const key = `${kasmQuantize(x)},${kasmQuantize(y)},${kasmQuantize(z)}`;
      if (vertexMap.has(key)) return vertexMap.get(key);
      const axisNodes = {
        x: kasmQuantizedCoordNode("x", x),
        y: kasmQuantizedCoordNode("y", y),
        z: kasmQuantizedCoordNode("z", z),
      };
      const cellNodes = [
        ensureCell([x, y, z], 1),
        ensureCell([x, y, z], 4),
        ensureCell([x, y, z], 16),
      ];
      const coordNode = {
        id: `coord:${vertexMap.size}`,
        tag: "coordinate",
        hash: kasmHashString(`coord-cartesian|${key}`),
        cartesianKey: key,
        position: [Number(x.toFixed(5)), Number(y.toFixed(5)), Number(z.toFixed(5))],
        axes: axisNodes,
        cells: cellNodes,
      };
      const id = `vertex:${vertexMap.size}`;
      const node = {
        id,
        tag: "vertex",
        hash: kasmHashString(`vertex|${key}`),
        position: [Number(x.toFixed(5)), Number(y.toFixed(5)), Number(z.toFixed(5))],
        coordinate: coordNode.id,
        coordinateHash: coordNode.hash,
        cellIds: cellNodes.map((cell) => cell.id),
        axisHashes: {
          x: axisNodes.x.hash,
          y: axisNodes.y.hash,
          z: axisNodes.z.hash,
        },
        cellHashes: cellNodes.map((cell) => cell.hash),
      };
      coordinates.push(coordNode);
      vertexMap.set(key, node);
      vertices.push(node);
      return node;
    };

    const ensureEdge = (a, b, faceId) => {
      const pair = [a.id, b.id].sort();
      const key = pair.join("|");
      let edge = edgeMap.get(key);
      if (!edge) {
        edge = {
          id: `edge:${edgeMap.size}`,
          tag: "edge",
          hash: kasmHashString(`edge|${key}`),
          vertices: pair,
          faces: [],
          cellIds: [...new Set([...(a.cellIds || []), ...(b.cellIds || [])])],
          cellHashes: [...new Set([...(a.cellHashes || []), ...(b.cellHashes || [])])],
        };
        edgeMap.set(key, edge);
        edges.push(edge);
      }
      if (!edge.faces.includes(faceId)) edge.faces.push(faceId);
      return edge;
    };

    for (let i = 0; i < pos.length; i += 9) {
      const va = ensureVertex(pos[i], pos[i + 1], pos[i + 2]);
      const vb = ensureVertex(pos[i + 3], pos[i + 4], pos[i + 5]);
      const vc = ensureVertex(pos[i + 6], pos[i + 7], pos[i + 8]);
      const faceVerts = [va.id, vb.id, vc.id];
      const faceId = `face:${faces.length}`;
      const faceHash = kasmHashString(`face|${faceVerts.join("|")}`);
      const edgeRefs = [
        ensureEdge(va, vb, faceId).id,
        ensureEdge(vb, vc, faceId).id,
        ensureEdge(vc, va, faceId).id,
      ];
      faces.push({
        id: faceId,
        tag: "face",
        hash: faceHash,
        vertices: faceVerts,
        edges: edgeRefs,
        cellIds: [...new Set([...(va.cellIds || []), ...(vb.cellIds || []), ...(vc.cellIds || [])])],
        cellHashes: [...new Set([...(va.cellHashes || []), ...(vb.cellHashes || []), ...(vc.cellHashes || [])])],
      });
    }

    const modifiers = ensureBoomItemModifiers(item).map((modifier) => {
      const params = serializeModifierParams(modifier);
      return {
        id: `modifier:${modifier.id}`,
        tag: "modifier",
        hash: kasmHashString(`modifier|${modifier.type}|${modifier.enabled !== false}|${JSON.stringify(params)}`),
        type: modifier.type,
        enabled: modifier.enabled !== false,
        params,
      };
    });

    const objectPayload = {
      objectId: item.id,
      sourceName: item.meta?.sourceName || item.name,
      cellHashes: cells.map((cell) => cell.hash),
      coordinateHashes: coordinates.map((coord) => coord.hash),
      vertexHashes: vertices.map((vertex) => vertex.hash),
      edgeHashes: edges.map((edge) => edge.hash),
      faceHashes: faces.map((face) => face.hash),
      modifierHashes: modifiers.map((modifier) => modifier.hash),
    };

    const object = {
      id: `object:${item.id}`,
      tag: "object",
      hash: kasmHashString(JSON.stringify(objectPayload)),
      runtimeItemId: item.id,
      runtimeMeshName: item.name,
      cells: cells.map((cell) => cell.id),
      coordinates: coordinates.map((coord) => coord.id),
      vertices: vertices.map((vertex) => vertex.id),
      edges: edges.map((edge) => edge.id),
      faces: faces.map((face) => face.id),
      modifiers: modifiers.map((modifier) => modifier.id),
    };

    return {
      kind: "boom-kasm-topology",
      version: 2,
      object,
      cells,
      coordinates,
      vertices,
      edges,
      faces,
      modifiers,
      runtime: {
        renderPasses: sceneMesh ? meshRenderPasses(sceneMesh).length : 0,
        bounds: meshData.bounds || null,
        gridOrigin: {
          hash: kasmHashString("grid-origin|0|0|0"),
          position: [0, 0, 0],
          axes: {
            x: kasmQuantizedCoordNode("x", 0),
            y: kasmQuantizedCoordNode("y", 0),
            z: kasmQuantizedCoordNode("z", 0),
          },
        },
      },
    };
  }

  function boomBoundingBoxFromPositions(positions) {
    if (!Array.isArray(positions) || !positions.length) return null;
    const min = [Infinity, Infinity, Infinity];
    const max = [-Infinity, -Infinity, -Infinity];
    for (const point of positions) {
      if (!Array.isArray(point) || point.length < 3) continue;
      for (let axis = 0; axis < 3; axis += 1) {
        if (point[axis] < min[axis]) min[axis] = point[axis];
        if (point[axis] > max[axis]) max[axis] = point[axis];
      }
    }
    if (!Number.isFinite(min[0]) || !Number.isFinite(max[0])) return null;
    return {
      min: min.map((value) => Number(value.toFixed(5))),
      max: max.map((value) => Number(value.toFixed(5))),
      span: max.map((value, axis) => Number((value - min[axis]).toFixed(5))),
      center: max.map((value, axis) => Number(((value + min[axis]) * 0.5).toFixed(5))),
    };
  }

  function boomUniquePush(map, key, value) {
    if (!key || !value) return;
    if (!map[key]) map[key] = [];
    if (!map[key].includes(value)) map[key].push(value);
  }

  function boomBuildSpatialRegion(graph, indexes, source) {
    if (!graph || !indexes) return null;
    const cellHashes = [...new Set(source?.cellHashes || [])];
    if (!cellHashes.length) return null;
    const vertexIds = [...new Set(source?.vertexIds || [])];
    const edgeIds = [...new Set(source?.edgeIds || [])];
    const faceIds = [...new Set(source?.faceIds || [])];
    const positions = vertexIds
      .map((id) => indexes.vertexMap.get(id)?.position || null)
      .filter(Boolean);
    const coarseCells = cellHashes
      .map((hash) => indexes.cellMap.get(hash))
      .filter((cell) => cell && cell.cellSize >= 4)
      .map((cell) => cell.hash);
    const bounds = boomBoundingBoxFromPositions(positions);
    const regionPayload = {
      sourceType: source?.sourceType || "selection",
      sourceNodeId: source?.sourceNodeId || "",
      cellHashes: [...cellHashes].sort(),
      vertexIds: [...vertexIds].sort(),
      edgeIds: [...edgeIds].sort(),
      faceIds: [...faceIds].sort(),
      coarseCells: [...coarseCells].sort(),
    };
    return {
      id: `region:${kasmHashString(JSON.stringify(regionPayload)).slice(5)}`,
      tag: "region",
      hash: kasmHashString(`region|${JSON.stringify(regionPayload)}`),
      sourceType: source?.sourceType || "selection",
      sourceNodeId: source?.sourceNodeId || "",
      sourceHash: source?.sourceHash || "",
      componentType: source?.componentType || "",
      layerIndex: Number.isFinite(source?.layerIndex) ? source.layerIndex : null,
      cellHashes,
      coarseCellHashes: coarseCells,
      vertexIds,
      edgeIds,
      faceIds,
      bounds,
      geonodeSeedHash: kasmHashString(`geonode-seed|${cellHashes.slice().sort().join("|")}`),
    };
  }

  function buildBoomKasmQueries(graph) {
    if (!graph) return null;
    const indexes = {
      vertexToEdges: {},
      vertexToFaces: {},
      vertexToCells: {},
      edgeToFaces: {},
      edgeToCells: {},
      faceToCells: {},
      cellToCoordinates: {},
      cellToVertices: {},
      cellToEdges: {},
      cellToFaces: {},
      cellToLayers: {},
      vertexMap: new Map((graph.vertices || []).map((entry) => [entry.id, entry])),
      edgeMap: new Map((graph.edges || []).map((entry) => [entry.id, entry])),
      faceMap: new Map((graph.faces || []).map((entry) => [entry.id, entry])),
      coordMap: new Map((graph.coordinates || []).map((entry) => [entry.id, entry])),
      cellMap: new Map((graph.cells || []).map((entry) => [entry.hash, entry])),
    };
    for (const vertex of graph.vertices || []) {
      for (const cellHash of vertex.cellHashes || []) {
        boomUniquePush(indexes.vertexToCells, vertex.id, cellHash);
        boomUniquePush(indexes.cellToVertices, cellHash, vertex.id);
      }
      const coord = indexes.coordMap.get(vertex.coordinate);
      if (coord) {
        for (const cell of coord.cells || []) {
          boomUniquePush(indexes.cellToCoordinates, cell.hash, coord.id);
        }
      }
    }
    for (const edge of graph.edges || []) {
      indexes.edgeToFaces[edge.id] = [...new Set(edge.faces || [])];
      indexes.edgeToCells[edge.id] = [...new Set(edge.cellHashes || [])];
      for (const vertexId of edge.vertices || []) {
        boomUniquePush(indexes.vertexToEdges, vertexId, edge.id);
      }
      for (const cellHash of edge.cellHashes || []) {
        boomUniquePush(indexes.cellToEdges, cellHash, edge.id);
      }
    }
    for (const face of graph.faces || []) {
      indexes.faceToCells[face.id] = [...new Set(face.cellHashes || [])];
      for (const vertexId of face.vertices || []) {
        boomUniquePush(indexes.vertexToFaces, vertexId, face.id);
      }
      for (const cellHash of face.cellHashes || []) {
        boomUniquePush(indexes.cellToFaces, cellHash, face.id);
      }
    }
    const query = {
      kind: "boom-kasm-queries",
      version: 1,
      hash: kasmHashString(`queries|${graph.object?.hash || "none"}|${(graph.cells || []).length}`),
      indexes,
    };
    query.vertexNeighborhood = (vertexId) => {
      const vertex = indexes.vertexMap.get(vertexId);
      if (!vertex) return null;
      const cellHashes = [...new Set(indexes.vertexToCells[vertexId] || [])];
      return boomBuildSpatialRegion(graph, indexes, {
        sourceType: "vertex-neighborhood",
        sourceNodeId: vertex.id,
        sourceHash: vertex.hash,
        componentType: "vertex",
        cellHashes,
        vertexIds: [vertex.id],
        edgeIds: indexes.vertexToEdges[vertex.id] || [],
        faceIds: indexes.vertexToFaces[vertex.id] || [],
      });
    };
    query.edgeNeighborhood = (edgeId) => {
      const edge = indexes.edgeMap.get(edgeId);
      if (!edge) return null;
      const vertexIds = [...new Set(edge.vertices || [])];
      const faceIds = [...new Set(edge.faces || [])];
      return boomBuildSpatialRegion(graph, indexes, {
        sourceType: "edge-neighborhood",
        sourceNodeId: edge.id,
        sourceHash: edge.hash,
        componentType: "edge",
        cellHashes: indexes.edgeToCells[edge.id] || [],
        vertexIds,
        edgeIds: [edge.id],
        faceIds,
      });
    };
    query.faceNeighborhood = (faceId) => {
      const face = indexes.faceMap.get(faceId);
      if (!face) return null;
      const vertexIds = [...new Set(face.vertices || [])];
      const edgeIds = [...new Set(face.edges || [])];
      return boomBuildSpatialRegion(graph, indexes, {
        sourceType: "face-neighborhood",
        sourceNodeId: face.id,
        sourceHash: face.hash,
        componentType: "face",
        cellHashes: indexes.faceToCells[face.id] || [],
        vertexIds,
        edgeIds,
        faceIds: [face.id],
      });
    };
    query.regionFromCells = (cellHashes, sourceType = "cell-region") => {
      const regionCells = [...new Set((cellHashes || []).filter(Boolean))];
      if (!regionCells.length) return null;
      const vertexIds = [...new Set(regionCells.flatMap((hash) => indexes.cellToVertices[hash] || []))];
      const edgeIds = [...new Set(regionCells.flatMap((hash) => indexes.cellToEdges[hash] || []))];
      const faceIds = [...new Set(regionCells.flatMap((hash) => indexes.cellToFaces[hash] || []))];
      return boomBuildSpatialRegion(graph, indexes, {
        sourceType,
        cellHashes: regionCells,
        vertexIds,
        edgeIds,
        faceIds,
      });
    };
    query.regionFromSelection = (selection = boomScene.componentSelection) => {
      if (!selection) {
        const activeMesh = activeBoomMeshItem();
        if (activeMesh?.id === "imported-mesh") {
          return query.regionFromCells((graph.cells || []).map((cell) => cell.hash), "mesh-volume");
        }
        return null;
      }
      if (selection.type === "vertex") return query.vertexNeighborhood(selection.nodeId);
      if (selection.type === "edge") return query.edgeNeighborhood(selection.nodeId);
      if (selection.type === "face") return query.faceNeighborhood(selection.nodeId);
      if (selection.itemId === "imported-mesh") {
        return query.regionFromCells((graph.cells || []).map((cell) => cell.hash), "mesh-volume");
      }
      return null;
    };
    query.volumeRegionFromSelection = (selection = boomScene.componentSelection) => {
      const base = query.regionFromSelection(selection);
      if (!base) return null;
      const coarse = base.coarseCellHashes?.length ? base.coarseCellHashes : base.cellHashes;
      return query.regionFromCells(coarse, "volume-region");
    };
    query.attachLayerRegion = (layerIndex, cellHashes) => {
      const region = query.regionFromCells(cellHashes, "slicer-layer");
      if (!region) return null;
      region.layerIndex = layerIndex;
      for (const hash of region.cellHashes) {
        boomUniquePush(indexes.cellToLayers, hash, `layer:${layerIndex}`);
      }
      return region;
    };
    return query;
  }

  function clearBoomRegionSelection() {
    boomScene.regionSelection = null;
  }

  function setBoomRegionSelection(region) {
    boomScene.regionSelection = region || null;
    return boomScene.regionSelection;
  }

  function activeBoomRegionSelection() {
    return boomScene.regionSelection || null;
  }

  function syncBoomSpatialTools() {
    if (!boomKasmGraph || !boomKasmQueries) {
      boomSpatialTools = null;
      if (typeof window !== "undefined") {
        window.__forgeBoomKasmQueries = null;
        window.__forgeBoomSpatialTools = null;
      }
      return null;
    }
    boomSpatialTools = {
      activeRegion: () => activeBoomRegionSelection(),
      regionFromSelection: () => boomKasmQueries.regionFromSelection(),
      volumeRegionFromSelection: () => boomKasmQueries.volumeRegionFromSelection(),
      selectRegionByCellHash: (cellHash) => {
        const region = boomKasmQueries.regionFromCells([cellHash], "cell-region");
        return setBoomRegionSelection(region);
      },
      applyRegionTool: (toolName, payload = {}) => {
        const region = activeBoomRegionSelection();
        if (!region) return null;
        return {
          hash: kasmHashString(`region-tool|${toolName}|${region.hash}|${stableBoomStringify(payload)}`),
          tool: toolName,
          regionHash: region.hash,
          cellHashes: region.cellHashes,
          payload,
        };
      },
      slicerLayerRegion: (layerIndex) => slicerPreview?.layers?.[layerIndex]?.region || null,
      geoNodeSeedFromRegion: (region = activeBoomRegionSelection()) => {
        if (!region) return null;
        return {
          id: `geonode:${region.geonodeSeedHash.slice(5)}`,
          hash: region.geonodeSeedHash,
          sourceRegionHash: region.hash,
          cellHashes: [...region.cellHashes],
          bounds: region.bounds,
        };
      },
    };
    if (typeof window !== "undefined") {
      window.__forgeBoomKasmQueries = boomKasmQueries;
      window.__forgeBoomSpatialTools = boomSpatialTools;
    }
    return boomSpatialTools;
  }

  function applyBoomKasmGraph(graph, item) {
    boomKasmGraph = graph || null;
    boomKasmQueries = graph ? buildBoomKasmQueries(graph) : null;
    clearBoomPickHandle("kasm-graph");
    if (item) {
      item.meta = {
        ...(item.meta || {}),
        kasm: graph ? {
          objectHash: graph.object.hash,
          cellCount: graph.cells.length,
          coordinateCount: graph.coordinates.length,
          vertexCount: graph.vertices.length,
          edgeCount: graph.edges.length,
          faceCount: graph.faces.length,
          modifierCount: graph.modifiers.length,
        } : null,
      };
    }
    if (typeof window !== "undefined") {
      window.__forgeBoomKasmGraph = boomKasmGraph;
      window.__forgeBoomKasmSnapshot = graph ? {
        objectHash: graph.object.hash,
        cells: graph.cells.length,
        coordinates: graph.coordinates.length,
        vertices: graph.vertices.length,
        edges: graph.edges.length,
        faces: graph.faces.length,
        modifiers: graph.modifiers.length,
      } : null;
    }
    if (boomScene.regionSelection && (!graph || !graph.cells?.some((cell) => boomScene.regionSelection.cellHashes?.includes(cell.hash)))) {
      clearBoomRegionSelection();
    }
    syncBoomSpatialTools();
  }

  function syncBoomKasmGraph(item, meshData = sceneMesh) {
    if (!item || !meshData?.pos?.length) {
      applyBoomKasmGraph(null, item || findBoomItem("imported-mesh"));
      return null;
    }
    const modifiers = ensureBoomItemModifiers(item);
    const meshHash = boomGeometryHash(meshData);
    const graphResult = boomCachedCompute(
      "kasm_topology",
      {
        meshHash,
        itemId: item.id,
        sourceName: item.meta?.sourceName || item.name || "",
        modifiers: boomModifierStackHash(modifiers),
      },
      meshData.faceCount || meshData.pos.length / 9,
      "faces",
      () => buildBoomKasmTopology(meshData, item),
    );
    const graph = graphResult.value;
    applyBoomKasmGraph(graph, item);
    return graph;
  }

  function normalizeVec3(x, y, z) {
    const len = Math.hypot(x, y, z) || 1;
    return [x / len, y / len, z / len];
  }

  function midpointVec3(a, b) {
    return [
      (a[0] + b[0]) * 0.5,
      (a[1] + b[1]) * 0.5,
      (a[2] + b[2]) * 0.5,
    ];
  }

  function mixVec3(a, b, t) {
    return [
      a[0] + (b[0] - a[0]) * t,
      a[1] + (b[1] - a[1]) * t,
      a[2] + (b[2] - a[2]) * t,
    ];
  }

  function appendTriangleWithNormals(posOut, nrmOut, a, b, c, na, nb, nc) {
    posOut.push(...a, ...b, ...c);
    nrmOut.push(...na, ...nb, ...nc);
  }

  function buildMeshGeometryBuffers(posSource, nrmSource) {
    const pos = posSource instanceof Float32Array ? posSource : new Float32Array(posSource);
    const nrm = nrmSource instanceof Float32Array ? nrmSource : new Float32Array(nrmSource);
    return {
      pos,
      nrm,
      count: pos.length / 3,
      faceCount: pos.length / 9,
    };
  }

  function applyBevelPreviewGeometry(geometry, modifier) {
    const width = Math.max(0.02, Math.min(0.42, Number(modifier.width || 0.14)));
    const insetT = Math.min(0.42, width);
    const posOut = [];
    const nrmOut = [];
    for (let i = 0; i < geometry.pos.length; i += 9) {
      const a = [geometry.pos[i], geometry.pos[i + 1], geometry.pos[i + 2]];
      const b = [geometry.pos[i + 3], geometry.pos[i + 4], geometry.pos[i + 5]];
      const c = [geometry.pos[i + 6], geometry.pos[i + 7], geometry.pos[i + 8]];
      const na = normalizeVec3(geometry.nrm[i], geometry.nrm[i + 1], geometry.nrm[i + 2]);
      const nb = normalizeVec3(geometry.nrm[i + 3], geometry.nrm[i + 4], geometry.nrm[i + 5]);
      const nc = normalizeVec3(geometry.nrm[i + 6], geometry.nrm[i + 7], geometry.nrm[i + 8]);
      const centroid = [
        (a[0] + b[0] + c[0]) / 3,
        (a[1] + b[1] + c[1]) / 3,
        (a[2] + b[2] + c[2]) / 3,
      ];
      const ia = mixVec3(a, centroid, insetT);
      const ib = mixVec3(b, centroid, insetT);
      const ic = mixVec3(c, centroid, insetT);
      appendTriangleWithNormals(posOut, nrmOut, ia, ib, ic, na, nb, nc);
      appendTriangleWithNormals(posOut, nrmOut, a, b, ib, na, nb, nb);
      appendTriangleWithNormals(posOut, nrmOut, a, ib, ia, na, nb, na);
      appendTriangleWithNormals(posOut, nrmOut, b, c, ic, nb, nc, nc);
      appendTriangleWithNormals(posOut, nrmOut, b, ic, ib, nb, nc, nb);
      appendTriangleWithNormals(posOut, nrmOut, c, a, ia, nc, na, na);
      appendTriangleWithNormals(posOut, nrmOut, c, ia, ic, nc, na, nc);
    }
    return buildMeshGeometryBuffers(posOut, nrmOut);
  }

  function applySubdivideGeometry(geometry, modifier) {
    const levels = Math.max(1, Math.min(3, Math.round(Number(modifier.levels || 1))));
    let current = geometry;
    for (let level = 0; level < levels; level += 1) {
      const posOut = [];
      const nrmOut = [];
      for (let i = 0; i < current.pos.length; i += 9) {
        const a = [current.pos[i], current.pos[i + 1], current.pos[i + 2]];
        const b = [current.pos[i + 3], current.pos[i + 4], current.pos[i + 5]];
        const c = [current.pos[i + 6], current.pos[i + 7], current.pos[i + 8]];
        const na = normalizeVec3(current.nrm[i], current.nrm[i + 1], current.nrm[i + 2]);
        const nb = normalizeVec3(current.nrm[i + 3], current.nrm[i + 4], current.nrm[i + 5]);
        const nc = normalizeVec3(current.nrm[i + 6], current.nrm[i + 7], current.nrm[i + 8]);
        const ab = midpointVec3(a, b);
        const bc = midpointVec3(b, c);
        const ca = midpointVec3(c, a);
        const nab = normalizeVec3(na[0] + nb[0], na[1] + nb[1], na[2] + nb[2]);
        const nbc = normalizeVec3(nb[0] + nc[0], nb[1] + nc[1], nb[2] + nc[2]);
        const nca = normalizeVec3(nc[0] + na[0], nc[1] + na[1], nc[2] + na[2]);
        appendTriangleWithNormals(posOut, nrmOut, a, ab, ca, na, nab, nca);
        appendTriangleWithNormals(posOut, nrmOut, ab, b, bc, nab, nb, nbc);
        appendTriangleWithNormals(posOut, nrmOut, ca, bc, c, nca, nbc, nc);
        appendTriangleWithNormals(posOut, nrmOut, ab, bc, ca, nab, nbc, nca);
      }
      current = buildMeshGeometryBuffers(posOut, nrmOut);
    }
    return current;
  }

  function applySolidifyGeometry(geometry, modifier) {
    const thickness = Math.max(0.02, Math.min(0.7, Number(modifier.thickness || 0.2)));
    const half = thickness * 0.5;
    const posOut = [];
    const nrmOut = [];
    for (let i = 0; i < geometry.pos.length; i += 9) {
      const a = [geometry.pos[i], geometry.pos[i + 1], geometry.pos[i + 2]];
      const b = [geometry.pos[i + 3], geometry.pos[i + 4], geometry.pos[i + 5]];
      const c = [geometry.pos[i + 6], geometry.pos[i + 7], geometry.pos[i + 8]];
      const na = normalizeVec3(geometry.nrm[i], geometry.nrm[i + 1], geometry.nrm[i + 2]);
      const nb = normalizeVec3(geometry.nrm[i + 3], geometry.nrm[i + 4], geometry.nrm[i + 5]);
      const nc = normalizeVec3(geometry.nrm[i + 6], geometry.nrm[i + 7], geometry.nrm[i + 8]);
      const oa = [a[0] + na[0] * half, a[1] + na[1] * half, a[2] + na[2] * half];
      const ob = [b[0] + nb[0] * half, b[1] + nb[1] * half, b[2] + nb[2] * half];
      const oc = [c[0] + nc[0] * half, c[1] + nc[1] * half, c[2] + nc[2] * half];
      const ia = [a[0] - na[0] * half, a[1] - na[1] * half, a[2] - na[2] * half];
      const ib = [b[0] - nb[0] * half, b[1] - nb[1] * half, b[2] - nb[2] * half];
      const ic = [c[0] - nc[0] * half, c[1] - nc[1] * half, c[2] - nc[2] * half];
      appendTriangleWithNormals(posOut, nrmOut, oa, ob, oc, na, nb, nc);
      appendTriangleWithNormals(posOut, nrmOut, ic, ib, ia, [-nc[0], -nc[1], -nc[2]], [-nb[0], -nb[1], -nb[2]], [-na[0], -na[1], -na[2]]);
    }
    return buildMeshGeometryBuffers(posOut, nrmOut);
  }

  function activeNativeBoomModifiers() {
    return ensureBoomItemModifiers(findBoomItem("imported-mesh")).filter((modifier) =>
      modifier.enabled !== false && (modifier.type === "bevel" || modifier.type === "subdivide" || modifier.type === "solidify")
    );
  }

  function boomNativeModifierStackHash(item = findBoomItem("imported-mesh")) {
    return boomModifierStackHash(ensureBoomItemModifiers(item).filter((modifier) =>
      modifier.enabled !== false && (modifier.type === "bevel" || modifier.type === "subdivide" || modifier.type === "solidify")
    ));
  }

  function rememberBoomModifierPlan(mesh = sceneMesh, item = findBoomItem("imported-mesh")) {
    if (!mesh || !item) return "";
    const nativeHash = boomNativeModifierStackHash(item);
    const allHash = boomModifierStackHash(ensureBoomItemModifiers(item));
    const planKey = kasmHashString(`modifier-plan|${boomGeometryHash(mesh.base || mesh)}|${nativeHash}|${allHash}`);
    const samePlan = mesh.modifierPlanKey === planKey;
    mesh.modifierPlanKey = planKey;
    emitBoomAudit("modifier_stack_plan", samePlan ? "HIT" : "MISS", planKey, 0, ensureBoomItemModifiers(item).length, "modifiers", {
      nativeHash,
      stackHash: allHash,
      materializationRequired: nativeHash !== boomModifierStackHash([]),
    });
    return nativeHash;
  }

  function slicerPreviewEnabled() {
    const meshItem = findBoomItem("imported-mesh");
    return !!sceneMesh
      && !!meshItem
      && meshItem.visible !== false
      && meshItem.renderable !== false
      && boomScene.propertyTab === "slicer"
      && boomScene.slicer?.workflow !== "prepare";
  }

  function releaseBoomSlicerPreview() {
    if (!slicerPreview) return;
    if (gl && slicerPreview.vao && !slicerPreview.gpuCacheKey) {
      try {
        for (const buffer of slicerPreview.buffers || []) gl.deleteBuffer(buffer);
        gl.deleteVertexArray(slicerPreview.vao);
      } catch (err) {
        console.warn("[banger] releaseBoomSlicerPreview error:", err);
      }
    }
    slicerPreview = null;
    requestBoomRender("slicer-preview-release");
  }

  function sliceEdgeAtZ(a, b, z, epsilon = 1e-5) {
    const az = a[2] - z;
    const bz = b[2] - z;
    if (Math.abs(az) <= epsilon && Math.abs(bz) <= epsilon) return null;
    if ((az > epsilon && bz > epsilon) || (az < -epsilon && bz < -epsilon)) return null;
    const denom = b[2] - a[2];
    if (Math.abs(denom) <= epsilon) return null;
    const t = (z - a[2]) / denom;
    if (t < -epsilon || t > 1 + epsilon) return null;
    return [
      a[0] + (b[0] - a[0]) * t,
      a[1] + (b[1] - a[1]) * t,
      z,
    ];
  }

  function sliceTriangleAtZ(a, b, c, z) {
    const hits = [];
    const pushHit = (point) => {
      if (!point) return;
      if (hits.some((entry) => Math.hypot(entry[0] - point[0], entry[1] - point[1], entry[2] - point[2]) < 1e-4)) return;
      hits.push(point);
    };
    pushHit(sliceEdgeAtZ(a, b, z));
    pushHit(sliceEdgeAtZ(b, c, z));
    pushHit(sliceEdgeAtZ(c, a, z));
    return hits.length === 2 ? hits : null;
  }

  function boomRenderPassHash(passes) {
    return kasmHashString(`render-pass-list|${stableBoomStringify((passes || []).map((pass) => ({
      transform: pass.transform,
      color: pass.color,
    })))}`);
  }

  function computeBoomSlicerLayerSegments(source, passModels, z) {
    const pos = [];
    const segmentCellHashes = [];
    const layerCellHashes = new Set();
    for (const pass of passModels) {
      for (let i = 0; i < source.pos.length; i += 9) {
        const a = transformPointWithModel(pass.model, source.pos[i], source.pos[i + 1], source.pos[i + 2]);
        const b = transformPointWithModel(pass.model, source.pos[i + 3], source.pos[i + 4], source.pos[i + 5]);
        const c = transformPointWithModel(pass.model, source.pos[i + 6], source.pos[i + 7], source.pos[i + 8]);
        const segment = sliceTriangleAtZ(a, b, c, z);
        if (!segment) continue;
        const midpoint = midpointVec3(segment[0], segment[1]);
        const cells = boomSpatialCellHashesForPoint(midpoint).map((cell) => cell.hash);
        for (const hash of cells) layerCellHashes.add(hash);
        pos.push(...segment[0], ...segment[1]);
        segmentCellHashes.push(cells);
      }
    }
    return {
      pos,
      segmentCellHashes,
      cellHashes: [...layerCellHashes],
      segmentCount: segmentCellHashes.length,
    };
  }

  function appendBoomSlicerLayer(posOut, colOut, layer, layerColor, regionColor, regionCells) {
    const sourcePos = layer?.pos || [];
    const segmentCells = layer?.segmentCellHashes || [];
    for (let segmentIndex = 0, offset = 0; offset < sourcePos.length; segmentIndex += 1, offset += 6) {
      const cells = segmentCells[segmentIndex] || [];
      const matchRegion = cells.some((hash) => regionCells.has(hash));
      const color = matchRegion ? regionColor : layerColor;
      posOut.push(
        sourcePos[offset],
        sourcePos[offset + 1],
        sourcePos[offset + 2],
        sourcePos[offset + 3],
        sourcePos[offset + 4],
        sourcePos[offset + 5],
      );
      colOut.push(...color, ...color);
    }
  }

  function computeBoomWorldBounds(source, passModels, sourceHash, passHash) {
    return boomCachedCompute(
      "world_bounds",
      { sourceHash, passHash },
      (source.count || source.pos.length / 3) * passModels.length,
      "vertex_transforms",
      () => {
        let minZ = Infinity;
        let maxZ = -Infinity;
        for (const pass of passModels) {
          for (let i = 2; i < source.pos.length; i += 3) {
            const world = transformPointWithModel(pass.model, source.pos[i - 2], source.pos[i - 1], source.pos[i]);
            if (world[2] < minZ) minZ = world[2];
            if (world[2] > maxZ) maxZ = world[2];
          }
        }
        return { minZ, maxZ, span: maxZ - minZ };
      },
    ).value;
  }

  function computeBoomSlicerPreviewGeometry(mesh = sceneMesh) {
    if (!mesh) return null;
    const source = mesh.display?.pos?.length ? mesh.display : mesh.base?.pos?.length ? mesh.base : mesh;
    if (!source?.pos?.length) return null;
    const passes = meshRenderPasses(mesh);
    if (!passes.length) return null;
    const sourceHash = boomGeometryHash(source);
    const passHash = boomRenderPassHash(passes);
    const passModels = passes;
    const bounds = computeBoomWorldBounds(source, passModels, sourceHash, passHash);
    const minZ = bounds.minZ;
    const maxZ = bounds.maxZ;
    if (!Number.isFinite(minZ) || !Number.isFinite(maxZ) || maxZ - minZ < 1e-4) return null;
    const slicer = boomScene.slicer || {};
    const baseScale = Math.max(0.0001, Number(mesh.bounds?.scale || 0.2));
    let layerStep = Math.max(0.03, Number(slicer.layerHeight || 0.2) * baseScale);
    const span = maxZ - minZ;
    let layerCount = Math.max(1, Math.floor(span / layerStep) + 1);
    if (layerCount > 240) {
      const factor = Math.ceil(layerCount / 240);
      layerStep *= factor;
      layerCount = Math.max(1, Math.floor(span / layerStep) + 1);
    }
    const adaptive = !!slicer.adaptiveLayers;
    const regionSelection = activeBoomRegionSelection();
    const layerPlan = [];
    let planZ = minZ + layerStep * 0.5;
    for (let layerIndex = 0; layerIndex < layerCount && planZ <= maxZ + 1e-4; layerIndex += 1) {
      const layerZ = Number(planZ.toFixed(5));
      const layerKeyParts = {
        sourceHash,
        passHash,
        layerIndex,
        z: kasmQuantize(layerZ),
      };
      layerPlan.push({
        index: layerIndex,
        z: planZ,
        layerZ,
        keyParts: layerKeyParts,
        cacheKey: boomComputeCacheKey("slicer_layer", layerKeyParts),
      });
      const stepFactor = adaptive ? (layerIndex % 6 === 0 ? 0.8 : layerIndex % 3 === 0 ? 1.2 : 1) : 1;
      planZ += layerStep * stepFactor;
    }
    const previewPlanKey = kasmHashString(`slicer-preview-plan|${sourceHash}|${passHash}|${kasmQuantize(layerStep)}|${regionSelection?.hash || ""}|${boomScene.slicer?.workflow || ""}|${layerPlan.map((layer) => layer.cacheKey).join("|")}`);
    if (slicerPreview?.cacheKey === previewPlanKey && ((gl && slicerPreview.vao) || (!gl && !slicerPreview.vao))) {
      emitBoomAudit("slicer_preview_reuse_gate", "HIT", previewPlanKey, 0, layerPlan.length, "layers");
      return {
        cacheKey: previewPlanKey,
        layerCount: slicerPreview.layerCount || layerPlan.length,
        reused: true,
      };
    }
    emitBoomAudit("slicer_preview_reuse_gate", "MISS", previewPlanKey, 0, layerPlan.length, "layers");
    const posOut = [];
    const colOut = [];
    const highlightColor = boomScene.slicer?.workflow === "print"
      ? [1.0, 0.76, 0.36]
      : [0.98, 0.68, 0.28];
    const faintColor = boomScene.slicer?.workflow === "print"
      ? [0.88, 0.42, 0.18]
      : [0.82, 0.52, 0.22];
    const regionColor = [1.0, 0.86, 0.46];
    const regionCells = new Set(regionSelection?.cellHashes || []);
    const layers = [];
    let emittedLayers = 0;
    for (const plannedLayer of layerPlan) {
      const layerIndex = plannedLayer.index;
      const emphasis = layerIndex % 5 === 0;
      const layerColor = emphasis ? highlightColor : faintColor;
      const layerResult = boomCachedCompute(
        "slicer_layer",
        plannedLayer.keyParts,
        (source.faceCount || source.pos.length / 9) * passModels.length,
        "triangle_layer_tests",
        () => computeBoomSlicerLayerSegments(source, passModels, plannedLayer.z),
      );
      const layer = layerResult.value;
      appendBoomSlicerLayer(posOut, colOut, layer, layerColor, regionColor, regionCells);
      const layerCellHashes = layer?.cellHashes || [];
      const layerRegion = boomKasmQueries?.attachLayerRegion?.(layerIndex, layerCellHashes) || null;
      layers.push({
        index: layerIndex,
        z: plannedLayer.layerZ,
        hash: kasmHashString(`slice-layer|${sourceHash}|${passHash}|${layerIndex}|${kasmQuantize(plannedLayer.layerZ)}|${layerCellHashes.slice().sort().join("|")}`),
        cellHashes: [...layerCellHashes],
        cacheKey: layerResult.key,
        segmentCount: layer?.segmentCount || 0,
        region: layerRegion,
      });
      emittedLayers += 1;
    }
    if (!posOut.length) return null;
    const activeLayerIndex = Math.max(0, Math.min(layers.length - 1, Math.floor(layers.length * 0.5)));
    return {
      pos: new Float32Array(posOut),
      col: new Float32Array(colOut),
      count: posOut.length / 3,
      layerCount: emittedLayers,
      step: layerStep,
      span,
      layers,
      activeLayerIndex,
      activeRegion: layers[activeLayerIndex]?.region || null,
      cacheKey: previewPlanKey,
    };
  }

  function uploadBoomSlicerPreview(geometry) {
    if (!gl || !geometry?.pos?.length || !geometry?.col?.length) return;
    const cacheKey = boomGpuResourceKey("slicer-preview", geometry.cacheKey || "preview");
    const cachedResource = lookupBoomGpuResource("gpu_slicer_upload", cacheKey, geometry.layerCount || 0, "layers");
    if (cachedResource) {
      slicerPreview = {
        ...geometry,
        vao: cachedResource.vao,
        buffers: cachedResource.buffers,
        gpuCacheKey: cacheKey,
      };
      return;
    }
    const started = boomNowMs();
    const uploadBytes = (geometry.pos.byteLength || 0) + (geometry.col.byteLength || 0);
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const posBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, geometry.pos, gl.STATIC_DRAW);
    const aPosL = gl.getAttribLocation(lineProg, "aPos");
    gl.enableVertexAttribArray(aPosL);
    gl.vertexAttribPointer(aPosL, 3, gl.FLOAT, false, 0, 0);
    const colBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, colBuf);
    gl.bufferData(gl.ARRAY_BUFFER, geometry.col, gl.STATIC_DRAW);
    const aColorL = gl.getAttribLocation(lineProg, "aColor");
    gl.enableVertexAttribArray(aColorL);
    gl.vertexAttribPointer(aColorL, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    slicerPreview = {
      ...geometry,
      vao,
      buffers: [posBuf, colBuf],
      gpuCacheKey: cacheKey,
    };
    const store = rememberBoomGpuResource(cacheKey, { vao, buffers: [posBuf, colBuf] }, uploadBytes, "slicer-preview");
    emitBoomAudit("gpu_slicer_upload", "MISS", cacheKey, boomNowMs() - started, geometry.layerCount || 0, "layers", {
      bytes: uploadBytes,
      stored: store.stored,
      evicted: store.evicted,
      evictedBytes: store.evictedBytes,
    });
  }

  function rebuildBoomSlicerPreview() {
    if (!slicerPreviewEnabled()) {
      releaseBoomSlicerPreview();
      requestBoomRender("slicer-preview-disabled");
      return;
    }
    const geometry = computeBoomSlicerPreviewGeometry(sceneMesh);
    if (!geometry) {
      releaseBoomSlicerPreview();
      requestBoomRender("slicer-preview-empty");
      return;
    }
    if (slicerPreview?.cacheKey === geometry.cacheKey && ((gl && slicerPreview.vao) || (!gl && !slicerPreview.vao))) {
      emitBoomAudit("slicer_preview_upload", "HIT", geometry.cacheKey, 0, geometry.layerCount || 0, "layers");
      requestBoomRender("slicer-preview-hit");
      return;
    }
    releaseBoomSlicerPreview();
    if (!gl) {
      slicerPreview = { ...geometry, vao: null, buffers: [] };
      requestBoomRender("slicer-preview-js");
      return;
    }
    uploadBoomSlicerPreview(geometry);
    requestBoomRender("slicer-preview-upload");
  }

  function releaseBoomDerivedMesh(mesh = sceneMesh) {
    if (!mesh?.display) return;
    if (gl && mesh.display.vao && !mesh.display.gpuCacheKey) {
      try {
        for (const buffer of mesh.display.buffers || []) gl.deleteBuffer(buffer);
        gl.deleteVertexArray(mesh.display.vao);
      } catch (err) {
        console.warn("[banger] releaseBoomDerivedMesh error:", err);
      }
    }
    mesh.display = null;
  }

  function uploadBoomDisplayGeometry(mesh, geometry, displayKey = "") {
    if (!gl || !mesh || !geometry?.pos?.length || !geometry?.nrm?.length) return;
    const cacheKey = boomGpuResourceKey("display-mesh", displayKey || boomGeometryHash(geometry));
    const cachedResource = lookupBoomGpuResource("gpu_display_upload", cacheKey, geometry.faceCount || geometry.pos.length / 9, "faces");
    if (cachedResource) {
      mesh.display = {
        ...geometry,
        vao: cachedResource.vao,
        buffers: cachedResource.buffers,
        gpuCacheKey: cacheKey,
      };
      return;
    }
    const started = boomNowMs();
    const uploadBytes = (geometry.pos.byteLength || 0) + (geometry.nrm.byteLength || 0);
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const posBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, geometry.pos, gl.STATIC_DRAW);
    const aPosM = gl.getAttribLocation(meshProg, "aPos");
    gl.enableVertexAttribArray(aPosM);
    gl.vertexAttribPointer(aPosM, 3, gl.FLOAT, false, 0, 0);
    const nrmBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, nrmBuf);
    gl.bufferData(gl.ARRAY_BUFFER, geometry.nrm, gl.STATIC_DRAW);
    const aNormalM = gl.getAttribLocation(meshProg, "aNormal");
    gl.enableVertexAttribArray(aNormalM);
    gl.vertexAttribPointer(aNormalM, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    mesh.display = {
      ...geometry,
      vao,
      buffers: [posBuf, nrmBuf],
      gpuCacheKey: cacheKey,
    };
    const store = rememberBoomGpuResource(cacheKey, { vao, buffers: [posBuf, nrmBuf] }, uploadBytes, "display-mesh");
    emitBoomAudit("gpu_display_upload", "MISS", cacheKey, boomNowMs() - started, geometry.faceCount || geometry.pos.length / 9, "faces", {
      bytes: uploadBytes,
      stored: store.stored,
      evicted: store.evicted,
      evictedBytes: store.evictedBytes,
    });
  }

  function rebuildBoomDisplayMesh(mesh = sceneMesh) {
    if (!mesh?.base?.pos?.length || !mesh?.base?.nrm?.length) return;
    const modifiers = activeNativeBoomModifiers();
    const baseGeometry = mesh.base;
    const baseHash = boomGeometryHash(baseGeometry);
    const nativeModifierHash = boomModifierStackHash(modifiers);
    const displayKey = kasmHashString(`display-mesh|${baseHash}|${nativeModifierHash}`);
    const displayStarted = boomNowMs();
    if (mesh.displayCacheKey === displayKey && (mesh.display?.pos?.length || modifiers.length === 0)) {
      mesh.nativeModifierStackHash = nativeModifierHash;
      emitBoomAudit("display_mesh", "HIT", displayKey, boomNowMs() - displayStarted, mesh.base.faceCount || baseGeometry.faceCount, "faces");
      requestBoomRender("display-mesh-hit");
      return;
    }
    releaseBoomDerivedMesh(mesh);
    let geometry = baseGeometry;
    for (const modifier of modifiers) {
      const inputHash = boomGeometryHash(geometry);
      const result = boomCachedCompute(
        `modifier_${modifier.type}`,
        {
          inputHash,
          modifier: boomModifierCachePayload(modifier),
        },
        geometry.faceCount || geometry.pos.length / 9,
        "faces",
        () => {
          if (modifier.type === "bevel") return applyBevelPreviewGeometry(geometry, modifier);
          if (modifier.type === "subdivide") return applySubdivideGeometry(geometry, modifier);
          if (modifier.type === "solidify") return applySolidifyGeometry(geometry, modifier);
          return geometry;
        },
      );
      geometry = result.value;
    }
    if (geometry.count === mesh.base.count) {
      mesh.displayCacheKey = displayKey;
      mesh.displayHash = baseHash;
      mesh.nativeModifierStackHash = nativeModifierHash;
      clearBoomPickHandle("display-mesh-base");
      emitBoomAudit("display_mesh", "MISS", displayKey, boomNowMs() - displayStarted, mesh.base.faceCount || baseGeometry.faceCount, "faces", { output: "base" });
      requestBoomRender("display-mesh-base");
      return;
    }
    const geometryHash = boomGeometryHash(geometry);
    if (!gl) {
      mesh.display = { ...geometry, vao: null, buffers: [] };
      try {
        Object.defineProperty(mesh.display, "kasmHash", { value: geometryHash, configurable: true, enumerable: false });
      } catch (_) {
        mesh.display.kasmHash = geometryHash;
      }
      mesh.displayCacheKey = displayKey;
      mesh.displayHash = geometryHash;
      mesh.nativeModifierStackHash = nativeModifierHash;
      clearBoomPickHandle("display-mesh-js");
      emitBoomAudit("display_mesh", "MISS", displayKey, boomNowMs() - displayStarted, geometry.faceCount || geometry.pos.length / 9, "faces", { output: "derived-js" });
      requestBoomRender("display-mesh-js");
      return;
    }
    uploadBoomDisplayGeometry(mesh, geometry, displayKey);
    if (mesh.display) {
      try {
        Object.defineProperty(mesh.display, "kasmHash", { value: geometryHash, configurable: true, enumerable: false });
      } catch (_) {
        mesh.display.kasmHash = geometryHash;
      }
    }
    mesh.displayCacheKey = displayKey;
    mesh.displayHash = geometryHash;
    mesh.nativeModifierStackHash = nativeModifierHash;
    clearBoomPickHandle("display-mesh-gpu");
    emitBoomAudit("display_mesh", "MISS", displayKey, boomNowMs() - displayStarted, geometry.faceCount || geometry.pos.length / 9, "faces", { output: "derived-gpu" });
    requestBoomRender("display-mesh-gpu");
  }

  function refreshBoomMeshPreview(item = activeBoomMeshItem()) {
    if (!sceneMesh) return;
    const nativeHash = rememberBoomModifierPlan(sceneMesh, item);
    if (sceneMesh.nativeModifierStackHash !== nativeHash || !sceneMesh.displayCacheKey) {
      rebuildBoomDisplayMesh(sceneMesh);
    } else {
      emitBoomAudit("display_mesh", "HIT", sceneMesh.displayCacheKey, 0, sceneMesh.base.faceCount || sceneMesh.faceCount || 0, "faces", {
        output: "modifier-plan-skip",
      });
    }
    syncBoomKasmGraph(item);
    rebuildBoomSlicerPreview();
    requestBoomRender("mesh-preview-refresh");
  }

  function findBoomItem(id) {
    return boomScene.items.find((item) => item.id === id) || null;
  }

  function syncCameraSceneSnapshot() {
    const item = boomItemById("camera");
    if (!item) return;
    const eye = cameraEye();
    item.transform.location = eye.map((value) => Number(value.toFixed(3)));
    item.transform.rotation = [
      Number((90 - radToDeg(camera.elevation)).toFixed(1)),
      0,
      Number((((radToDeg(camera.azimuth) + 360) % 360)).toFixed(1)),
    ];
  }

  function activeBoomItem() {
    syncCameraSceneSnapshot();
    return boomItemById(boomScene.activeId);
  }

  function activeBoomTransform() {
    return activeBoomItem()?.transform || null;
  }

  function boomIcon(name) {
    switch (name) {
      case "collection":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 4.5h4l1 1h6v6.5a1 1 0 0 1-1 1h-10a1 1 0 0 1-1-1v-6.5a1 1 0 0 1 1-1Z"/><path d="M2 5.5h12"/></svg>';
      case "camera":
      case "camera-tab":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="5.5" width="7.5" height="5.5" rx="1.4"/><path d="m10 7 3-1.8v6.6L10 10"/></svg>';
      case "grid":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 3.5h10v9H3z"/><path d="M6.3 3.5v9M9.7 3.5v9M3 6.5h10M3 9.5h10"/></svg>';
      case "mesh":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m8 2.5 4.7 2.6v5.8L8 13.5l-4.7-2.6V5.1z"/><path d="M8 2.5v5.8m4.7-3.2L8 8.3 3.3 5.1"/><path d="M5 9.8 8 11.5l3-1.7"/></svg>';
      case "light":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.5a3.2 3.2 0 0 1 2.4 5.3c-.5.6-.9 1.1-1 1.8H6.6c-.1-.7-.5-1.2-1-1.8A3.2 3.2 0 0 1 8 2.5Z"/><path d="M6.2 11.2h3.6"/><path d="M6.7 13h2.6"/></svg>';
      case "world":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="5.5"/><path d="M2.5 8h11"/><path d="M8 2.5c1.6 1.7 1.6 9.3 0 11"/><path d="M8 2.5c-1.6 1.7-1.6 9.3 0 11"/></svg>';
      case "object":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m8 2.5 4.7 2.6v5.8L8 13.5l-4.7-2.6V5.1z"/><path d="M8 2.5v5.8m4.7-3.2L8 8.3 3.3 5.1"/></svg>';
      case "scene":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="3" width="11" height="10" rx="1.4"/><path d="M5 6h6M5 8.5h6M5 11h3.5"/></svg>';
      case "material":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M8 2.5c1.8 2 4 4.2 4 6.3A4 4 0 0 1 4 8.8c0-2.1 2.2-4.3 4-6.3Z"/></svg>';
      case "printer":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 5V2.8h8V5"/><rect x="2.5" y="5" width="11" height="5.5" rx="1.4"/><path d="M5.2 10.5h5.6V13H5.2z"/><circle cx="11.2" cy="7.8" r=".7"/></svg>';
      case "layers":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m8 2.5 5 2.7-5 2.7-5-2.7z"/><path d="m3 8.5 5 2.7 5-2.7"/><path d="m3 11.1 5 2.4 5-2.4"/></svg>';
      case "filament":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4.4a3.6 3.6 0 1 1 0 7.2 3.6 3.6 0 0 1 0-7.2Z"/><path d="M4 6.4a1.6 1.6 0 1 1 0 3.2 1.6 1.6 0 0 1 0-3.2Z"/><path d="M7.4 8h4.6a1.5 1.5 0 0 1 0 3"/></svg>';
      case "speed":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 11.5a5.5 5.5 0 1 1 10 0"/><path d="M8 8l2.8-1.8"/><path d="M5.2 11.5h5.6"/></svg>';
      case "support":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 12.5V8.8l4-2.3 4 2.3v3.7"/><path d="M3 13.5h10"/><path d="M6 8.8v4.7M10 8.8v4.7"/></svg>';
      case "wrench":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M9.8 2.7a3 3 0 0 0 3.5 3.5L8.2 11.3a1.5 1.5 0 1 1-2.1-2.1l5.1-5.1A3 3 0 0 0 9.8 2.7Z"/></svg>';
      case "eye":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M1.8 8s2.2-3.2 6.2-3.2S14.2 8 14.2 8s-2.2 3.2-6.2 3.2S1.8 8 1.8 8Z"/><circle cx="8" cy="8" r="1.8"/></svg>';
      case "cursor":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3 2.5 8 5.2-3.2.9 1.8 3.4-1.6.9-1.8-3.4-2.2 2V2.5Z"/></svg>';
      case "render":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2.5" y="4" width="8" height="7" rx="1.4"/><path d="m10.5 6 3-1.8v7.6l-3-1.8"/></svg>';
      case "chevron":
        return '<svg viewBox="0 0 12 12" aria-hidden="true"><path d="m4 2 4 4-4 4"/></svg>';
      case "filter":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2.5 3.5h11l-4.1 4.4v3.8l-2.8 1.3V7.9z"/></svg>';
      case "search":
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="7" cy="7" r="3.8"/><path d="m10 10 3.2 3.2"/></svg>';
      default:
        return '<svg viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="4.5"/></svg>';
    }
  }

  function boomObjectRows() {
    const filter = boomScene.filter.trim().toLowerCase();
    return boomScene.items.filter((item) => !filter || item.name.toLowerCase().includes(filter));
  }

  function shortBoomKasmHash(hash, head = 10, tail = 6) {
    const text = String(hash || "");
    if (text.length <= head + tail + 3) return text;
    return `${text.slice(0, head)}...${text.slice(-tail)}`;
  }

  function boomKasmGraphViewId() {
    return BOOM_KASM_GRAPH_VIEWS.some((view) => view.id === boomScene.kasmGraphView)
      ? boomScene.kasmGraphView
      : "world";
  }

  function boomKasmEntryTitle(entry, resolved = null) {
    const record = resolved?.record || {};
    return entry?.name
      || record.name
      || record.programName
      || record.skillName
      || record.metricName
      || record.label
      || record.pageKind
      || record.renderMode
      || record.kind
      || "KASM hash";
  }

  function boomKasmFallbackEntry(viewId, projection) {
    const sceneHash = projection?.sceneHash || boomKasmCurrentSceneHash();
    const fallback = viewId === "assets"
      ? {
          kind: "kasm-asset-root",
          version: 1,
          id: boomKasmObjectHash("asset-root-v1", { sceneHash }),
          name: "AssetStore",
          status: "ready",
          outputHashes: [sceneHash],
        }
      : viewId === "runs"
        ? {
            kind: "kasm-run-root",
            version: 1,
            id: projection?.id || boomKasmObjectHash("run-root-v1", { sceneHash }),
            name: "RunHistory",
            status: "ready",
            outputHashes: [sceneHash],
          }
        : {
            kind: "kasm-scene-snapshot",
            version: 1,
            id: sceneHash,
            name: "SceneHash",
            status: "live",
            outputHashes: [sceneHash],
          };
    rememberBoomKasmHash(fallback, `${viewId}-root`);
    return compactBoomKasmRecord(fallback);
  }

  function boomKasmGraphEntriesForView(projection, viewId) {
    const rawEntries = Array.isArray(projection?.views?.[viewId]) ? projection.views[viewId] : [];
    const entries = [];
    const seen = new Set();
    for (const entry of rawEntries) {
      const id = String(entry?.id || "");
      if (!id || seen.has(id)) continue;
      seen.add(id);
      entries.push(entry);
    }
    if (!entries.length) entries.push(boomKasmFallbackEntry(viewId, projection));
    return entries.slice(0, 28);
  }

  function boomKasmGraphRowsMarkup(projection) {
    const viewId = boomKasmGraphViewId();
    const view = BOOM_KASM_GRAPH_VIEWS.find((entry) => entry.id === viewId) || BOOM_KASM_GRAPH_VIEWS[0];
    const entries = boomKasmGraphEntriesForView(projection, viewId);
    const selectedHash = boomScene.selectedKasmHash || projection.sceneHash || "";
    const rows = entries.map((entry) => {
      const hash = String(entry?.id || "");
      const resolved = resolveBoomKasmHash(hash);
      const title = boomKasmEntryTitle(entry, resolved);
      const role = resolved?.role || entry?.kind || "kasm-object";
      const status = entry?.status || resolved?.record?.status || resolved?.record?.residency || "";
      const isActive = hash === selectedHash;
      return `
        <button class="boom-kasm-row${isActive ? " is-active" : ""}" data-action="select-kasm-hash" data-kasm-hash="${escapeBoomHtml(hash)}" title="${escapeBoomHtml(hash)}">
          <span class="boom-kasm-row-icon boom-outliner-type-${escapeBoomHtml(view.icon)}" aria-hidden="true">${boomIcon(view.icon)}</span>
          <span class="boom-kasm-row-copy">
            <span class="boom-kasm-row-title">${escapeBoomHtml(title)}</span>
            <span class="boom-kasm-row-meta">
              <span>${escapeBoomHtml(role)}</span>
              <code>${escapeBoomHtml(shortBoomKasmHash(hash))}</code>
            </span>
          </span>
          ${status ? `<span class="boom-kasm-row-status">${escapeBoomHtml(status)}</span>` : ""}
        </button>
      `;
    }).join("");
    return `
      <div class="boom-kasm-browser" data-kasm-view="${escapeBoomHtml(viewId)}">
        <div class="boom-kasm-tabs" role="tablist" aria-label="KASM graph views">
          ${BOOM_KASM_GRAPH_VIEWS.map((entry) => `
            <button class="boom-kasm-tab${entry.id === viewId ? " is-active" : ""}" data-action="kasm-graph-view" data-kasm-view="${entry.id}" role="tab" aria-selected="${entry.id === viewId ? "true" : "false"}" title="${entry.label}">
              ${boomIcon(entry.icon)}
              <span>${entry.label}</span>
            </button>
          `).join("")}
        </div>
        <div class="boom-kasm-list" role="tree" aria-label="${escapeBoomHtml(view.label)} KASM hashes">
          ${rows}
        </div>
      </div>
    `;
  }

  function boomKasmDataRows(rows) {
    return rows
      .filter((row) => row && row[1] !== "" && row[1] != null)
      .map(([label, value]) => `
        <div>
          <span>${escapeBoomHtml(label)}</span>
          <strong${String(value).length > 18 ? ' class="boom-kasm-hash"' : ""}>${escapeBoomHtml(value)}</strong>
        </div>
      `).join("");
  }

  function boomKasmHashListMarkup(title, hashes, empty = "None") {
    const items = (hashes || []).filter(Boolean).slice(0, 5);
    if (!items.length) {
      return `
        <div class="boom-kasm-proof-block">
          <span>${escapeBoomHtml(title)}</span>
          <strong>${escapeBoomHtml(empty)}</strong>
        </div>
      `;
    }
    return `
      <div class="boom-kasm-proof-block">
        <span>${escapeBoomHtml(title)}</span>
        <div class="boom-kasm-proof-values">
          ${items.map((hash) => `<code title="${escapeBoomHtml(hash)}">${escapeBoomHtml(shortBoomKasmHash(hash, 12, 8))}</code>`).join("")}
        </div>
      </div>
    `;
  }

  function boomKasmSelectedHashInspectorMarkup(activeKasm = null, projection = null) {
    const graph = projection || buildBoomKasmGraphProjection();
    const hash = boomScene.selectedKasmHash || activeKasm?.objectHash || graph.sceneHash || "";
    const resolved = resolveBoomKasmHash(hash);
    const explanation = explainBoomKasmHash(hash) || {
      hash,
      role: resolved?.role || "scene-hash",
      objectKind: resolved?.record?.kind || "kasm-scene-snapshot",
      outputHashes: [hash],
      inputHashes: [],
      metricHashes: [],
    };
    const record = resolved?.record || {};
    const cache = boomCacheStatusSummary();
    const metricHashes = explanation.metricHashes?.length
      ? explanation.metricHashes
      : boomKasmMetricHistory.filter((metric) => metric.targetHash === hash).slice(-4).map((metric) => metric.id);
    const dependencyHashes = [
      explanation.commandHash,
      explanation.programHash,
      explanation.computeProgramHash,
      explanation.bytecodeHash,
      explanation.shaderHash,
      explanation.sandboxHash,
      explanation.sourceHash,
      explanation.programGraphHash,
      explanation.metricSetHash,
      explanation.sceneHash,
      explanation.assetStoreHash,
      explanation.assetPackHash,
      explanation.residencyHash,
      explanation.lodTreeHash,
      explanation.boundsTreeHash,
    ].filter(Boolean);
    const proofHash = explanation.objectKind === "kasm-proof-record"
      ? explanation.hash
      : explanation.proofHash || "";
    const rollbackHash = explanation.rollbackPatchHash || record.rollbackPatchHash || "";
    const frameBudget = record.frameBudget || record.budget || {};
    const perfCost = frameBudget.frameMs
      ? `${formatScalar(frameBudget.frameMs, 2)} ms`
      : `${formatScalar(cache.p95, 2)} ms p95`;
    const memoryCost = frameBudget.ramBytes
      ? formatBoomBytes(frameBudget.ramBytes)
      : record.ramBytes
        ? formatBoomBytes(record.ramBytes)
      : `${formatBoomBytes(cache.cacheBytes)} / ${formatBoomBytes(cache.cacheMaxBytes)}`;
    const gpuCost = frameBudget.vramBytes
      ? formatBoomBytes(frameBudget.vramBytes)
      : record.vramBytes
        ? formatBoomBytes(record.vramBytes)
      : `${formatBoomBytes(cache.gpuResourceBytes)} / ${formatBoomBytes(cache.gpuResourceMaxBytes)}`;
    const propertyRows = boomKasmDataRows([
      ["Selected Hash", shortBoomKasmHash(explanation.hash || hash, 12, 8)],
      ["Role", explanation.role || "hash"],
      ["Kind", explanation.objectKind || record.kind || "kasm-object"],
      ["Status", explanation.status || record.status || "ready"],
      ["Action", explanation.action || record.action || ""],
      ["Residency", explanation.residency || record.residency || ""],
      ["Page Count", explanation.pageCount || record.pageCount || ""],
      ["SceneHash", explanation.sceneHash ? shortBoomKasmHash(explanation.sceneHash, 12, 8) : ""],
    ]);
    const costRows = boomKasmDataRows([
      ["Performance Cost", perfCost],
      ["Memory Cost", memoryCost],
      ["GPU Cost", gpuCost],
      ["Cache", `${cache.hits}/${cache.misses}`],
    ]);
    return `
      <section class="boom-inspector-card boom-kasm-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Selected Hash</span>
            <span class="boom-inspector-card-title">${escapeBoomHtml(shortBoomKasmHash(explanation.hash || hash, 16, 10))}</span>
          </div>
          <span class="boom-inspector-card-badge">${escapeBoomHtml(explanation.role || "KASM")}</span>
        </div>
        <div class="boom-scene-grid boom-kasm-grid boom-kasm-inspector-grid">
          ${propertyRows}
          ${costRows}
        </div>
        <div class="boom-kasm-proof-grid">
          ${boomKasmHashListMarkup("Proof", proofHash ? [proofHash] : [])}
          ${boomKasmHashListMarkup("Dependencies", dependencyHashes)}
          ${boomKasmHashListMarkup("Metrics", metricHashes)}
          ${boomKasmHashListMarkup("Rollback", rollbackHash ? [rollbackHash] : [])}
          ${boomKasmHashListMarkup("Cluster Pages", explanation.clusterPageHashes || [])}
          ${boomKasmHashListMarkup("Asset Pages", explanation.assetPageHashes || explanation.pageHashes || [])}
          ${boomKasmHashListMarkup("Residency", explanation.residencyHash ? [explanation.residencyHash] : [])}
          ${boomKasmHashListMarkup("Outputs", explanation.outputHashes || record.outputHashes || [])}
        </div>
      </section>
    `;
  }

  function flushBoomSidebar() {
    if (!boomSidebarRoot) return false;
    const active = activeBoomItem();
    const activeTransform = activeBoomTransform();
    const activeIsMesh = isBoomMeshItem(active);
    const activeModifiers = activeIsMesh ? ensureBoomItemModifiers(active) : [];
    const currentMode = boomModeHeadline();
    const activeKasm = active?.meta?.kasm || null;
    const kasmProjection = buildBoomKasmGraphProjection();
    const kasmGraphMarkup = boomKasmGraphRowsMarkup(kasmProjection);
    const kasmInspectorMarkup = boomKasmSelectedHashInspectorMarkup(activeKasm, kasmProjection);
    const animationSummary = currentBoomAnimationSummary();
    const componentSelection = activeBoomComponentSelection();
    const componentSummary = boomComponentSummary(componentSelection, boomKasmGraph);
    const regionSummary = boomRegionSummary();
    const objectRows = boomScene.collectionExpanded
      ? boomObjectRows().map((item) => {
          const isActive = item.id === boomScene.activeId;
          const itemName = escapeBoomHtml(item.name);
          const itemMeta = escapeBoomHtml(boomImportedMeshStats(item) || "Imported mesh");
          return `
            <div class="boom-outliner-row boom-outliner-row-object${isActive ? " is-active" : ""}" data-action="select" data-id="${item.id}" role="treeitem" aria-selected="${isActive ? "true" : "false"}">
              <span class="boom-outliner-indent" aria-hidden="true"></span>
              <span class="boom-outliner-type boom-outliner-type-${item.type}" aria-hidden="true">${boomIcon(item.type)}</span>
              <span class="boom-outliner-copy">
                <span class="boom-outliner-label">${itemName}</span>
                ${item.type === "mesh" && item.meta?.imported
                  ? `<span class="boom-outliner-meta">${itemMeta}</span>`
                  : ""}
              </span>
              <span class="boom-outliner-toggles">
                <button class="boom-toggle${item.visible ? " is-on" : ""}" data-action="toggle-visible" data-id="${item.id}" title="Viewport visibility">${boomIcon("eye")}</button>
                <button class="boom-toggle${item.selectable ? " is-on" : ""}" data-action="toggle-selectable" data-id="${item.id}" title="Selectable">${boomIcon("cursor")}</button>
                <button class="boom-toggle${item.renderable ? " is-on" : ""}" data-action="toggle-renderable" data-id="${item.id}" title="Render">${boomIcon("render")}</button>
              </span>
            </div>
          `;
        }).join("")
      : "";

    const propertyTabs = boomVisiblePropertyTabs().map((tab) => `
      <button class="boom-inspector-tab${tab.id === boomScene.propertyTab ? " is-active" : ""}" data-action="tab" data-tab="${tab.id}" title="${tab.title}">
        ${boomIcon(tab.icon === "camera" ? "camera-tab" : tab.icon)}
        <span>${tab.title}</span>
      </button>
    `).join("");
    const workflowTabs = `
      <div class="boom-workflow-bar" role="tablist" aria-label="BOOM workflow">
        <button class="boom-workflow-btn${boomScene.workspaceMode === "design" ? " is-active" : ""}" data-action="workspace-mode" data-workspace-mode="design" role="tab" aria-selected="${boomScene.workspaceMode === "design" ? "true" : "false"}">Design</button>
        <button class="boom-workflow-btn${boomScene.workspaceMode === "slicer" ? " is-active" : ""}" data-action="workspace-mode" data-workspace-mode="slicer" role="tab" aria-selected="${boomScene.workspaceMode === "slicer" ? "true" : "false"}">Slicer</button>
      </div>
    `;

    const transformMarkup = activeTransform ? `
      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Transform</span>
            <span class="boom-inspector-card-title">Placement and orientation</span>
          </div>
          <span class="boom-inspector-card-badge">${currentMode}</span>
        </div>
        <div class="boom-transform-grid">
          <div class="boom-transform-label">Location</div>
          <div class="boom-transform-triplet">
            <label><span>X</span><input data-field="location" data-axis="0" value="${formatScalar(activeTransform.location?.[0] ?? 0)}"></label>
            <label><span>Y</span><input data-field="location" data-axis="1" value="${formatScalar(activeTransform.location?.[1] ?? 0)}"></label>
            <label><span>Z</span><input data-field="location" data-axis="2" value="${formatScalar(activeTransform.location?.[2] ?? 0)}"></label>
          </div>
          <div class="boom-transform-label">Rotation</div>
          <div class="boom-transform-triplet">
            <label><span>X</span><input data-field="rotation" data-axis="0" value="${formatAngle(activeTransform.rotation?.[0] ?? 0)}"></label>
            <label><span>Y</span><input data-field="rotation" data-axis="1" value="${formatAngle(activeTransform.rotation?.[1] ?? 0)}"></label>
            <label><span>Z</span><input data-field="rotation" data-axis="2" value="${formatAngle(activeTransform.rotation?.[2] ?? 0)}"></label>
          </div>
          <div class="boom-transform-label">Mode</div>
          <div class="boom-transform-mode">
            <select data-field="mode">
              <option${activeTransform.mode === "XYZ Euler" ? " selected" : ""}>XYZ Euler</option>
              <option${activeTransform.mode === "Quaternion" ? " selected" : ""}>Quaternion</option>
            </select>
          </div>
          <div class="boom-transform-label">Scale</div>
          <div class="boom-transform-triplet">
            <label><span>X</span><input data-field="scale" data-axis="0" value="${formatScalar(activeTransform.scale?.[0] ?? 1)}"></label>
            <label><span>Y</span><input data-field="scale" data-axis="1" value="${formatScalar(activeTransform.scale?.[1] ?? 1)}"></label>
            <label><span>Z</span><input data-field="scale" data-axis="2" value="${formatScalar(activeTransform.scale?.[2] ?? 1)}"></label>
          </div>
        </div>
      </section>
    ` : '<div class="boom-props-placeholder">Select an object to inspect its transform.</div>';

    const objectSummary = `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">${activeIsMesh ? "Object" : "Selection"}</span>
            <span class="boom-inspector-card-title">${escapeBoomHtml(active?.name || "Selection")}</span>
          </div>
        </div>
        <div class="boom-inspector-pillrow">
          <span class="boom-inspector-pill">${escapeBoomHtml(currentMode)} mode</span>
          <span class="boom-inspector-pill">${active?.visible === false ? "Hidden" : "Visible"}</span>
          <span class="boom-inspector-pill">${active?.renderable === false ? "No render" : "Renderable"}</span>
          ${activeIsMesh ? `<span class="boom-inspector-pill is-accent">${escapeBoomHtml(boomImportedMeshStats(active) || "Mesh")}</span>` : ""}
        </div>
        ${activeIsMesh && active?.meta?.sourceName
          ? `<p class="boom-inspector-note">Imported from <strong>${escapeBoomHtml(active.meta.sourceName)}</strong>.</p>`
          : `<p class="boom-inspector-note">This inspector stays focused on the currently selected scene item, with larger touch targets and fewer hidden options.</p>`}
      </section>
    `;

    const componentMarkup = activeIsMesh && boomScene.editMode !== "object" ? `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Edit mode</span>
            <span class="boom-inspector-card-title">${escapeBoomHtml(currentMode)} workflow</span>
          </div>
          <span class="boom-inspector-card-badge">Viewport</span>
        </div>
        ${componentSummary ? `
          <div class="boom-scene-grid boom-kasm-grid">
            ${componentSummary.details.map(([label, value]) => `<div><span>${escapeBoomHtml(label)}</span><strong>${escapeBoomHtml(value)}</strong></div>`).join("")}
            <div><span>Node hash</span><strong class="boom-kasm-hash">${escapeBoomHtml(componentSummary.hash)}</strong></div>
            ${componentSummary.coordHash ? `<div><span>XYZ hash</span><strong class="boom-kasm-hash">${escapeBoomHtml(componentSummary.coordHash)}</strong></div>` : ""}
            <div><span>Selection</span><strong>${escapeBoomHtml(componentSummary.subtitle)}</strong></div>
          </div>
          <p class="boom-inspector-note"><strong>${escapeBoomHtml(componentSummary.title)}</strong> is live-selected in the viewport. Click another ${escapeBoomHtml(currentMode.toLowerCase())} to retarget the tool stack.</p>
        ` : `
          <p class="boom-inspector-note">The mode bar is now wired to viewport picking. Click directly in the matrix to select a ${escapeBoomHtml(currentMode.toLowerCase())}; the inspector and future tools will follow that component.</p>
        `}
      </section>
    ` : "";

    const topologyMarkup = activeIsMesh && activeKasm ? `
      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">KASM topology</span>
            <span class="boom-inspector-card-title">Symbolic layer above runtime mesh</span>
          </div>
          <span class="boom-inspector-card-badge">${activeKasm.modifierCount || 0} mod</span>
        </div>
        <div class="boom-scene-grid boom-kasm-grid">
          <div><span>Cells</span><strong>${activeKasm.cellCount || 0}</strong></div>
          <div><span>Coordinates</span><strong>${activeKasm.coordinateCount || 0}</strong></div>
          <div><span>Vertices</span><strong>${activeKasm.vertexCount}</strong></div>
          <div><span>Edges</span><strong>${activeKasm.edgeCount}</strong></div>
          <div><span>Faces</span><strong>${activeKasm.faceCount}</strong></div>
          <div><span>Object hash</span><strong class="boom-kasm-hash">${escapeBoomHtml(activeKasm.objectHash)}</strong></div>
        </div>
        <p class="boom-inspector-note">BOOM now tracks this mesh as a KASM graph with explicit <strong>cell / coordinate / vertex / edge / face / object / modifier</strong> nodes. XYZ positions carry their own axis hashes and spatial cell hashes, so geometry can be queried and cached as real space, not just as triangles.</p>
      </section>
    ` : "";

    const regionMarkup = activeIsMesh && regionSummary ? `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Spatial region</span>
            <span class="boom-inspector-card-title">${escapeBoomHtml(regionSummary.title)}</span>
          </div>
          <span class="boom-inspector-card-badge">${regionSummary.details[0]?.[1] || "0"}</span>
        </div>
        <div class="boom-scene-grid boom-kasm-grid">
          ${regionSummary.details.map(([label, value]) => `<div><span>${escapeBoomHtml(label)}</span><strong>${escapeBoomHtml(value)}</strong></div>`).join("")}
          <div><span>Region hash</span><strong class="boom-kasm-hash">${escapeBoomHtml(regionSummary.hash)}</strong></div>
          <div><span>Geo seed</span><strong class="boom-kasm-hash">${escapeBoomHtml(regionSummary.geonodeSeedHash)}</strong></div>
        </div>
        ${regionSummary.bounds ? `<p class="boom-inspector-note">Bounds ${escapeBoomHtml(regionSummary.bounds.min.join(", "))} â†’ ${escapeBoomHtml(regionSummary.bounds.max.join(", "))}. This region can now drive slicer layers, volume tools and future symbolic geonodes.</p>` : ""}
      </section>
    ` : "";

    const modifierCards = activeIsMesh && activeModifiers.length
      ? activeModifiers.map((modifier, index) => {
          const title = escapeBoomHtml(boomModifierTitle(modifier));
          const meta = escapeBoomHtml(boomModifierMeta(modifier));
          const axisOptions = ["X", "Y", "Z"].map((axis) => `<option${modifier.axis === axis ? " selected" : ""}>${axis}</option>`).join("");
          let controls = "";
          if (modifier.type === "mirror") {
            controls = `
              <label class="boom-modifier-field">
                <span>Axis</span>
                <select data-modifier-id="${modifier.id}" data-modifier-field="axis">${axisOptions}</select>
              </label>
            `;
          } else if (modifier.type === "array") {
            controls = `
              <label class="boom-modifier-field">
                <span>Axis</span>
                <select data-modifier-id="${modifier.id}" data-modifier-field="axis">${axisOptions}</select>
              </label>
              <label class="boom-modifier-field">
                <span>Count</span>
                <input type="number" min="2" max="6" step="1" data-modifier-id="${modifier.id}" data-modifier-field="count" value="${Number(modifier.count || 3)}">
              </label>
              <label class="boom-modifier-field">
                <span>Offset</span>
                <input type="number" min="0.25" max="8" step="0.05" data-modifier-id="${modifier.id}" data-modifier-field="offset" value="${Number(modifier.offset || 2.25).toFixed(2)}">
              </label>
            `;
          } else if (modifier.type === "inflate") {
            controls = `
              <label class="boom-modifier-field">
                <span>Amount</span>
                <input type="number" min="0.8" max="1.8" step="0.02" data-modifier-id="${modifier.id}" data-modifier-field="amount" value="${Number(modifier.amount || 1.08).toFixed(2)}">
              </label>
            `;
          } else if (modifier.type === "bevel") {
            controls = `
              <label class="boom-modifier-field">
                <span>Width</span>
                <input type="number" min="0.02" max="0.42" step="0.01" data-modifier-id="${modifier.id}" data-modifier-field="width" value="${Number(modifier.width || 0.14).toFixed(2)}">
              </label>
            `;
          } else if (modifier.type === "subdivide") {
            controls = `
              <label class="boom-modifier-field">
                <span>Levels</span>
                <input type="number" min="1" max="3" step="1" data-modifier-id="${modifier.id}" data-modifier-field="levels" value="${Math.max(1, Number(modifier.levels || 1))}">
              </label>
            `;
          } else if (modifier.type === "solidify") {
            controls = `
              <label class="boom-modifier-field">
                <span>Thickness</span>
                <input type="number" min="0.02" max="0.70" step="0.01" data-modifier-id="${modifier.id}" data-modifier-field="thickness" value="${Number(modifier.thickness || 0.2).toFixed(2)}">
              </label>
            `;
          }
          return `
            <section class="boom-modifier-card${modifier.enabled === false ? " is-disabled" : ""}">
              <div class="boom-modifier-head">
                <button class="boom-modifier-toggle" data-action="modifier-expand" data-modifier-id="${modifier.id}" title="Expand modifier">${boomIcon("chevron")}</button>
                <div class="boom-modifier-copy">
                  <span class="boom-modifier-title">${title}</span>
                  <span class="boom-modifier-meta">${meta}</span>
                </div>
                <div class="boom-modifier-actions">
                  <button class="boom-icon-chip${modifier.enabled === false ? "" : " is-active"}" data-action="modifier-toggle" data-modifier-id="${modifier.id}" title="Enable or disable modifier">${modifier.enabled === false ? "Off" : "On"}</button>
                  <button class="boom-icon-chip" data-action="modifier-up" data-modifier-id="${modifier.id}" title="Move up"${index === 0 ? " disabled" : ""}>↑</button>
                  <button class="boom-icon-chip" data-action="modifier-down" data-modifier-id="${modifier.id}" title="Move down"${index === activeModifiers.length - 1 ? " disabled" : ""}>↓</button>
                  <button class="boom-icon-chip" data-action="modifier-remove" data-modifier-id="${modifier.id}" title="Remove modifier">×</button>
                </div>
              </div>
              ${modifier.expanded === false ? "" : `<div class="boom-modifier-body">${controls}</div>`}
            </section>
          `;
        }).join("")
      : '<div class="boom-props-placeholder">No modifier yet. Add one below to start shaping the imported mesh without leaving BOOM.</div>';

    const modifierShelf = activeIsMesh ? `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Quick modifiers</span>
            <span class="boom-inspector-card-title">A first non-destructive workflow</span>
          </div>
        </div>
        <div class="boom-modifier-shelf">
          ${BOOM_MODIFIER_PRESETS.map((preset) => `
            <button class="boom-modifier-preset" data-action="modifier-add" data-preset="${preset.type}" title="Add ${preset.title}">
              <span class="boom-modifier-preset-title">${preset.title}</span>
              <span class="boom-modifier-preset-copy">${preset.copy}</span>
            </button>
          `).join("")}
        </div>
      </section>
    ` : `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Modifiers</span>
            <span class="boom-inspector-card-title">Select a mesh to start</span>
          </div>
        </div>
        <p class="boom-inspector-note">Modifiers are currently scoped to imported meshes so the workflow stays predictable while BOOM grows.</p>
      </section>
    `;

    const materialMarkup = activeIsMesh ? `
      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Material</span>
            <span class="boom-inspector-card-title">Clay preview surface</span>
          </div>
        </div>
        <div class="boom-material-row">
          <span class="boom-material-swatch" aria-hidden="true"></span>
          <div class="boom-material-copy">
            <strong>BOOM Clay</strong>
            <span>Neutral studio material for shape-first iteration before shading.</span>
          </div>
        </div>
      </section>
    ` : '<div class="boom-props-placeholder">Materials will appear here when a mesh is selected.</div>';

    const sceneMarkup = `
      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Scene</span>
            <span class="boom-inspector-card-title">Viewport atmosphere</span>
          </div>
          <span class="boom-inspector-card-badge">${runtimeStatus?.backendReady ? "Warm" : "Cold"}</span>
        </div>
        <div class="boom-scene-grid">
          <div><span>Camera distance</span><strong>${formatScalar(camera.distance, 2)}</strong></div>
          <div><span>Active mode</span><strong>${escapeBoomHtml(currentMode)}</strong></div>
          <div><span>Programs</span><strong>${Number(runtimeStatus?.installedPrograms || 0)}</strong></div>
          <div><span>Caches</span><strong>${Number(runtimeStatus?.runCacheEntries || 0) + Number(runtimeStatus?.inspectCacheEntries || 0)}</strong></div>
        </div>
      </section>
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Animation bridge</span>
            <span class="boom-inspector-card-title">3D file to JS animation and back</span>
          </div>
          <span class="boom-inspector-card-badge">${animationSummary ? (animationSummary.playing ? "Playing" : "Loaded") : "Ready"}</span>
        </div>
        <div class="boom-slicer-chiprow boom-slicer-chiprow-padded">
          <button class="boom-slicer-chip" data-action="export-animation-js">Export JS</button>
          <button class="boom-slicer-chip" data-action="export-animation-json">Export JSON</button>
          ${animationSummary ? `<button class="boom-slicer-chip${animationSummary.playing ? " is-active" : ""}" data-action="${animationSummary.playing ? "pause-animation" : "play-animation"}">${animationSummary.playing ? "Pause" : "Play"}</button>` : ""}
        </div>
        <p class="boom-inspector-note">Export the current mesh as a BOOM animation script or JSON payload, then drag the generated file back into BOOM to reconstruct the scene and replay the animation.</p>
        ${animationSummary ? `<p class="boom-inspector-note"><strong>${escapeBoomHtml(animationSummary.name)}</strong> · ${formatScalar(animationSummary.durationMs / 1000, 2)}s · ${animationSummary.trackCount} tracks${animationSummary.sourceName ? ` · source ${escapeBoomHtml(animationSummary.sourceName)}` : ""}</p>` : ""}
      </section>
    `;

    const slicerEstimate = boomSlicerEstimate();
    const slicer = boomScene.slicer || {};
    const printerDevices = Array.isArray(slicer.devices) ? slicer.devices : [];
    const printerProfiles = Array.isArray(slicer.profiles) && slicer.profiles.length
      ? slicer.profiles.map((profile) => profile.label)
      : ["CoreXY 0.4 nozzle", "Bedslinger 0.4 nozzle", "High-flow 0.6 nozzle", "Resin 192 x 120 x 200"];
    const deviceCard = `
      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Printer link</span>
            <span class="boom-inspector-card-title">Detect physical machines</span>
          </div>
          <span class="boom-inspector-card-badge">${escapeBoomHtml(slicer.discoveryState || "idle")}</span>
        </div>
        <div class="boom-slicer-chiprow boom-slicer-chiprow-padded">
          <button class="boom-slicer-chip${slicer.discoveryState === "ready" ? " is-active" : ""}" data-action="refresh-printers">Rescan printers</button>
          ${slicer.discoveryBackend ? `<span class="boom-slicer-chip boom-slicer-chip-passive">${escapeBoomHtml(slicer.discoveryBackend)}</span>` : ""}
        </div>
        ${printerDevices.length ? `
          <div class="boom-printer-device-list">
            ${printerDevices.map((device) => `
              <div class="boom-printer-device${device.likely3dPrinter ? " is-active" : ""}">
                <strong>${escapeBoomHtml(device.name || "Device")}</strong>
                <span>${escapeBoomHtml(device.port || device.vendor || device.source || "detected device")}</span>
              </div>
            `).join("")}
          </div>
        ` : `<p class="boom-inspector-note">No connected printer exposed yet. BOOM keeps real slicer profiles available even when the machine is offline.</p>`}
        ${slicer.discoveryWarnings?.length ? `<p class="boom-inspector-note">${escapeBoomHtml(slicer.discoveryWarnings.join(" "))}</p>` : ""}
      </section>
    `;
    const activeLayerRegion = slicerPreview?.activeRegion || null;
    const slicerMarkup = `
      ${deviceCard}
      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Slicer</span>
            <span class="boom-inspector-card-title">From design to fabrication</span>
          </div>
          <span class="boom-inspector-card-badge">${slicer.mode === "recommended" ? "Recommended" : "Custom"}</span>
        </div>
        <div class="boom-slicer-stagebar">
          ${["prepare", "preview", "print"].map((stage) => `
            <button class="boom-stage-chip${slicer.workflow === stage ? " is-active" : ""}" data-action="slicer-workflow" data-slicer-value="${stage}">
              ${stage[0].toUpperCase() + stage.slice(1)}
            </button>
          `).join("")}
        </div>
        <div class="boom-slicer-dualrow">
          <div class="boom-slicer-fieldgroup">
            <span class="boom-slicer-label">Setup</span>
            <div class="boom-slicer-chiprow">
              ${["recommended", "custom"].map((mode) => `
                <button class="boom-slicer-chip${slicer.mode === mode ? " is-active" : ""}" data-action="slicer-mode" data-slicer-value="${mode}">
                  ${mode[0].toUpperCase() + mode.slice(1)}
                </button>
              `).join("")}
            </div>
          </div>
          <div class="boom-slicer-fieldgroup">
            <span class="boom-slicer-label">Detail level</span>
            <div class="boom-slicer-chiprow">
              ${["simple", "advanced", "expert"].map((level) => `
                <button class="boom-slicer-chip${slicer.level === level ? " is-active" : ""}" data-action="slicer-level" data-slicer-value="${level}">
                  ${level[0].toUpperCase() + level.slice(1)}
                </button>
              `).join("")}
            </div>
          </div>
        </div>
      </section>

      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Profiles</span>
            <span class="boom-inspector-card-title">Printer, material, quality</span>
          </div>
        </div>
        <div class="boom-slicer-grid">
          <label class="boom-slicer-field">
            <span>${boomIcon("printer")} Printer</span>
            <select data-slicer-field="printerProfile">
              ${printerProfiles.map((value) => `<option${slicer.printerProfile === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>${boomIcon("filament")} Material</span>
            <select data-slicer-field="materialProfile">
              ${["PLA 1.75","PETG 1.75","ABS 1.75","TPU 95A"].map((value) => `<option${slicer.materialProfile === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>${boomIcon("layers")} Quality</span>
            <select data-slicer-field="qualityPreset">
              ${["0.12 mm Fine","0.20 mm Balanced","0.28 mm Draft"].map((value) => `<option${slicer.qualityPreset === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
        </div>
      </section>

      <section class="boom-inspector-card">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Process</span>
            <span class="boom-inspector-card-title">Layer, infill, support, adhesion</span>
          </div>
          <span class="boom-inspector-card-badge">${escapeBoomHtml(slicer.level || "advanced")}</span>
        </div>
        <div class="boom-slicer-grid">
          <label class="boom-slicer-field">
            <span>Layer height (mm)</span>
            <input type="number" min="0.08" max="0.40" step="0.01" data-slicer-field="layerHeight" value="${Number(slicer.layerHeight || 0.2).toFixed(2)}">
          </label>
          <label class="boom-slicer-field">
            <span>Wall loops</span>
            <input type="number" min="1" max="8" step="1" data-slicer-field="wallLoops" value="${Number(slicer.wallLoops || 3)}">
          </label>
          <label class="boom-slicer-field">
            <span>${boomIcon("speed")} Speed (mm/s)</span>
            <input type="number" min="40" max="400" step="5" data-slicer-field="printSpeed" value="${Number(slicer.printSpeed || 160)}">
          </label>
          <label class="boom-slicer-field">
            <span>Infill density (%)</span>
            <input type="number" min="0" max="100" step="1" data-slicer-field="infillDensity" value="${Number(slicer.infillDensity || 18)}">
          </label>
          <label class="boom-slicer-field">
            <span>Infill pattern</span>
            <select data-slicer-field="infillPattern">
              ${["Gyroid","Grid","Cubic","Lightning"].map((value) => `<option${slicer.infillPattern === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>${boomIcon("support")} Supports</span>
            <select data-slicer-field="supportMode">
              ${["None","Build plate","Everywhere","Organic"].map((value) => `<option${slicer.supportMode === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>Adhesion</span>
            <select data-slicer-field="adhesion">
              ${["None","Skirt","Brim","Raft"].map((value) => `<option${slicer.adhesion === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>Seam</span>
            <select data-slicer-field="seam">
              ${["Aligned","Nearest","Rear","Random"].map((value) => `<option${slicer.seam === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>Speed preset</span>
            <select data-slicer-field="speedPreset">
              ${["Detail","Balanced","Fast"].map((value) => `<option${slicer.speedPreset === value ? " selected" : ""}>${value}</option>`).join("")}
            </select>
          </label>
          <label class="boom-slicer-field">
            <span>Nozzle temp (°C)</span>
            <input type="number" min="170" max="320" step="1" data-slicer-field="nozzleTemp" value="${Number(slicer.nozzleTemp || 210)}">
          </label>
          <label class="boom-slicer-field">
            <span>Bed temp (°C)</span>
            <input type="number" min="0" max="130" step="1" data-slicer-field="bedTemp" value="${Number(slicer.bedTemp || 60)}">
          </label>
          <label class="boom-slicer-toggle">
            <input type="checkbox" data-slicer-field="adaptiveLayers"${slicer.adaptiveLayers ? " checked" : ""}>
            <span>Adaptive layers</span>
          </label>
        </div>
      </section>

      <section class="boom-inspector-card boom-inspector-card-soft">
        <div class="boom-inspector-card-head">
          <div class="boom-inspector-card-copy">
            <span class="boom-inspector-card-kicker">Dry run</span>
            <span class="boom-inspector-card-title">Slicer-style preview signals</span>
          </div>
          <span class="boom-inspector-card-badge">${escapeBoomHtml(slicer.workflow || "prepare")}</span>
        </div>
        <div class="boom-scene-grid boom-slicer-estimate-grid">
          <div><span>Projected layers</span><strong>${slicerEstimate.projectedLayers}</strong></div>
          <div><span>Estimated time</span><strong>${slicerEstimate.printMinutes} min</strong></div>
          <div><span>Material</span><strong>${slicerEstimate.materialGrams} g</strong></div>
          <div><span>Mesh passes</span><strong>${slicerEstimate.passCount}</strong></div>
        </div>
        ${activeLayerRegion ? `
          <div class="boom-scene-grid boom-slicer-estimate-grid">
            <div><span>Active layer</span><strong>${Number(activeLayerRegion.layerIndex || 0) + 1}</strong></div>
            <div><span>Layer cells</span><strong>${activeLayerRegion.cellHashes?.length || 0}</strong></div>
            <div><span>Layer faces</span><strong>${activeLayerRegion.faceIds?.length || 0}</strong></div>
            <div><span>Layer hash</span><strong class="boom-kasm-hash">${escapeBoomHtml(activeLayerRegion.hash)}</strong></div>
          </div>
        ` : ""}
        <p class="boom-inspector-note">${activeIsMesh
          ? `The BOOM workflow now treats <strong>Design</strong> and <strong>Slicer</strong> as two complementary phases: build the object first, then move here to target a real printer, inspect layers and prepare fabrication for <strong>${escapeBoomHtml(active?.name || "mesh")}</strong>.`
          : "Select a mesh to drive these slicer settings from a real printable object. The panel is already scene-aware, but the selected mesh is the anchor for print preparation."}</p>
      </section>
    `;

    let propertiesBody = kasmInspectorMarkup + objectSummary + regionMarkup + topologyMarkup + transformMarkup + componentMarkup;
    if (boomScene.propertyTab === "slicer") {
      propertiesBody = kasmInspectorMarkup + slicerMarkup;
    } else if (boomScene.propertyTab === "modifiers") {
      propertiesBody = `
        ${kasmInspectorMarkup}
        ${modifierShelf}
        <section class="boom-inspector-card">
          <div class="boom-inspector-card-head">
            <div class="boom-inspector-card-copy">
              <span class="boom-inspector-card-kicker">Stack</span>
              <span class="boom-inspector-card-title">Reorder, tune, iterate</span>
            </div>
            ${activeIsMesh ? `<span class="boom-inspector-card-badge">${activeModifiers.length}</span>` : ""}
          </div>
          <div class="boom-modifier-stack">${modifierCards}</div>
        </section>
      `;
    } else if (boomScene.propertyTab === "material") {
      propertiesBody = kasmInspectorMarkup + objectSummary + materialMarkup;
    } else if (boomScene.propertyTab === "scene") {
      propertiesBody = kasmInspectorMarkup + sceneMarkup;
    }

    const html = `
      ${workflowTabs}
      <div class="boom-outliner">
        <div class="boom-panel-head">
          <div class="boom-panel-title">
            <span class="boom-panel-title-icon" aria-hidden="true">${boomIcon("collection")}</span>
            <span class="boom-panel-title-label">Scene Collection</span>
          </div>
          <label class="boom-search">
            <span class="boom-search-icon" aria-hidden="true">${boomIcon("search")}</span>
            <input type="search" data-action="filter" placeholder="Search" value="${escapeBoomHtml(boomScene.filter)}">
          </label>
          <button class="boom-panel-icon" data-action="noop" title="Filter">${boomIcon("filter")}</button>
        </div>
        <div class="boom-outliner-tree" role="tree" aria-label="Scene collection">
          <div class="boom-outliner-row boom-outliner-row-collection boom-outliner-row-collection-top" data-action="toggle-collection" role="treeitem" aria-expanded="${boomScene.collectionExpanded ? "true" : "false"}">
            <span class="boom-outliner-indent boom-outliner-indent-top" aria-hidden="true"></span>
            <span class="boom-outliner-disclosure${boomScene.collectionExpanded ? " is-open" : ""}">${boomIcon("chevron")}</span>
            <span class="boom-outliner-type boom-outliner-type-collection" aria-hidden="true">${boomIcon("collection")}</span>
            <span class="boom-outliner-label">Collection</span>
            <span class="boom-outliner-toggles boom-outliner-toggles-passive">
              <span class="boom-toggle is-on">${boomIcon("eye")}</span>
              <span class="boom-toggle is-on">${boomIcon("cursor")}</span>
              <span class="boom-toggle is-on">${boomIcon("render")}</span>
            </span>
          </div>
          ${objectRows || '<div class="boom-outliner-empty">No object matches the current filter.</div>'}
          ${kasmGraphMarkup}
        </div>
      </div>
      <div class="boom-properties">
        <div class="boom-properties-main boom-inspector-shell">
          <div class="boom-props-header boom-inspector-header">
            <div class="boom-props-active-icon boom-outliner-type-${active?.type || "object"}">${boomIcon(active?.type || "object")}</div>
            <div class="boom-props-active-copy">
              <div class="boom-props-kicker">Inspector</div>
              <div class="boom-props-title">${escapeBoomHtml(active?.name || "Selection")}</div>
            </div>
            <div class="boom-inspector-tabrow">
              ${propertyTabs}
            </div>
          </div>
          <div class="boom-props-body">
            ${propertiesBody}
          </div>
        </div>
      </div>
    `;
    const htmlHash = kasmHashString(`ui-sidebar|${html}`);
    if (boomSidebarHtmlHash === htmlHash && boomSidebarRoot.dataset.boomHtmlHash === htmlHash) {
      boomUiRenderStats.sidebarSkips += 1;
      return false;
    }
    boomSidebarHtmlHash = htmlHash;
    boomSidebarRoot.dataset.boomHtmlHash = htmlHash;
    boomSidebarRoot.innerHTML = html;
    boomUiRenderStats.sidebarFlushes += 1;
    return true;
  }

  function ensureBoomSidebar() {
    if (!els.leftPanel) return;
    if (!boomSidebarRoot || !boomSidebarRoot.isConnected) {
      boomSidebarRoot = document.createElement("section");
      boomSidebarRoot.className = "boom-blender-panel";
      els.leftPanel.appendChild(boomSidebarRoot);
    }
    if (!boomSidebarBound) {
      boomSidebarRoot.addEventListener("click", (event) => {
        const trigger = event.target.closest("[data-action]");
        if (!trigger) return;
        const action = trigger.dataset.action;
        const id = trigger.dataset.id;
        if (action !== "noop") event.preventDefault();
        if (action === "select" && id) {
          const item = boomItemById(id);
          if (!item?.selectable) return;
          executeBoomTool("boom.scene.select_item", { itemId: id, preserveMeshComponentSelection: true });
        } else if (action === "workspace-mode") {
          executeBoomTool("boom.scene.workspace_mode", { mode: trigger.dataset.workspaceMode || "design" });
          if (boomScene.workspaceMode === "slicer") {
            void refreshBoomPrinterDiscovery();
          }
        } else if (action === "toggle-collection") {
          boomScene.collectionExpanded = !boomScene.collectionExpanded;
          renderBoomSidebar();
        } else if (action === "kasm-graph-view") {
          const viewId = trigger.dataset.kasmView || "world";
          executeBoomTool("boom.kasm.set_graph_view", { viewId });
        } else if (action === "select-kasm-hash") {
          const hash = String(trigger.dataset.kasmHash || "").trim();
          if (!hash) return;
          executeBoomTool("boom.kasm.select_hash", { hash });
        } else if (action === "toggle-visible" || action === "toggle-selectable" || action === "toggle-renderable") {
          const item = boomItemById(id);
          if (!item) return;
          const field = action === "toggle-visible"
            ? "visible"
            : action === "toggle-selectable"
              ? "selectable"
              : "renderable";
          item[field] = !item[field];
          if (field === "selectable" && !item[field] && boomScene.activeId === item.id) {
            boomScene.activeId = "grid";
            clearBoomComponentSelection();
            clearBoomRegionSelection();
          }
          if (item.id === "imported-mesh" && sceneMesh) {
            if (field === "visible" || field === "renderable") sceneMesh.visible = item[field];
          }
          renderBoomSidebar();
          renderBoomViewportHud();
        } else if (action === "tab") {
          boomScene.propertyTab = trigger.dataset.tab || "object";
          setBoomWorkspaceMode(boomScene.propertyTab === "slicer" ? "slicer" : "design");
          rebuildBoomSlicerPreview();
          renderBoomViewportHud();
          renderBoomSidebar();
        } else if (action === "slicer-mode") {
          setBoomWorkspaceMode("slicer");
          boomScene.propertyTab = "slicer";
          boomScene.slicer.mode = trigger.dataset.slicerValue || "recommended";
          rebuildBoomSlicerPreview();
          renderBoomViewportHud();
          renderBoomSidebar();
        } else if (action === "slicer-level") {
          setBoomWorkspaceMode("slicer");
          boomScene.propertyTab = "slicer";
          boomScene.slicer.level = trigger.dataset.slicerValue || "advanced";
          rebuildBoomSlicerPreview();
          renderBoomViewportHud();
          renderBoomSidebar();
        } else if (action === "slicer-workflow") {
          boomScene.propertyTab = "slicer";
          executeBoomTool("boom.slicer.set_workflow", { workflow: trigger.dataset.slicerValue || "prepare" });
        } else if (action === "refresh-printers") {
          setBoomWorkspaceMode("slicer");
          renderBoomSidebar();
          void refreshBoomPrinterDiscovery(true);
        } else if (action === "modifier-add") {
          const preset = BOOM_MODIFIER_PRESETS.find((entry) => entry.type === String(trigger.dataset.preset || ""));
          if (!preset) return;
          executeBoomTool("boom.modifier.add", { type: preset.type });
        } else if (action === "modifier-toggle") {
          const activeMesh = activeBoomMeshItem();
          const modifier = ensureBoomItemModifiers(activeMesh).find((entry) => entry.id === trigger.dataset.modifierId);
          if (!modifier) return;
          modifier.enabled = modifier.enabled === false;
          refreshBoomMeshPreview(activeMesh);
          renderBoomSidebar();
          renderBoomViewportHud();
        } else if (action === "modifier-expand") {
          const activeMesh = activeBoomMeshItem();
          const modifier = ensureBoomItemModifiers(activeMesh).find((entry) => entry.id === trigger.dataset.modifierId);
          if (!modifier) return;
          modifier.expanded = modifier.expanded === false;
          renderBoomSidebar();
        } else if (action === "modifier-up" || action === "modifier-down") {
          const activeMesh = activeBoomMeshItem();
          if (!activeMesh) return;
          if (moveBoomModifier(activeMesh, trigger.dataset.modifierId, action === "modifier-up" ? -1 : 1)) {
            refreshBoomMeshPreview(activeMesh);
            renderBoomSidebar();
            renderBoomViewportHud();
          }
        } else if (action === "modifier-remove") {
          const activeMesh = activeBoomMeshItem();
          const modifiers = ensureBoomItemModifiers(activeMesh);
          const index = modifiers.findIndex((entry) => entry.id === trigger.dataset.modifierId);
          if (index < 0) return;
          modifiers.splice(index, 1);
          refreshBoomMeshPreview(activeMesh);
          renderBoomSidebar();
          renderBoomViewportHud();
        } else if (action === "export-animation-js") {
          executeBoomTool("boom.animation.export_js", {});
        } else if (action === "export-animation-json") {
          executeBoomTool("boom.animation.export_json", {});
        } else if (action === "play-animation") {
          executeBoomTool("boom.animation.play", {});
        } else if (action === "pause-animation") {
          executeBoomTool("boom.animation.pause", {});
        }
      });
      boomSidebarRoot.addEventListener("input", (event) => {
        const target = event.target;
        if (!(target instanceof HTMLInputElement || target instanceof HTMLSelectElement)) return;
        if (target.matches('input[type="search"][data-action="filter"]')) {
          boomScene.filter = target.value || "";
          renderBoomSidebar();
          return;
        }
        const slicerField = target.dataset.slicerField;
        if (slicerField) {
          let nextValue;
          if (target instanceof HTMLInputElement && target.type === "checkbox") {
            nextValue = target.checked;
          } else if (target instanceof HTMLInputElement && target.type === "number") {
            nextValue = Number.parseFloat(target.value);
            if (!Number.isFinite(nextValue)) return;
            if (slicerField === "wallLoops") nextValue = Math.max(1, Math.min(8, Math.round(nextValue)));
            if (slicerField === "infillDensity") nextValue = Math.max(0, Math.min(100, Math.round(nextValue)));
            if (slicerField === "printSpeed") nextValue = Math.max(40, Math.min(400, Math.round(nextValue)));
            if (slicerField === "nozzleTemp") nextValue = Math.max(170, Math.min(320, Math.round(nextValue)));
            if (slicerField === "bedTemp") nextValue = Math.max(0, Math.min(130, Math.round(nextValue)));
            if (slicerField === "layerHeight") nextValue = Number(nextValue.toFixed(2));
          } else {
            nextValue = target.value;
          }
          boomScene.slicer[slicerField] = nextValue;
          rebuildBoomSlicerPreview();
          renderBoomViewportHud();
          renderBoomSidebar();
          return;
        }
        const modifierId = target.dataset.modifierId;
        const modifierField = target.dataset.modifierField;
        if (modifierId && modifierField) {
          const activeMesh = activeBoomMeshItem();
          const modifier = ensureBoomItemModifiers(activeMesh).find((entry) => entry.id === modifierId);
          if (!modifier) return;
          if (modifierField === "axis" && target instanceof HTMLSelectElement) {
            modifier.axis = target.value;
          } else {
            const value = Number.parseFloat(target.value);
            if (!Number.isFinite(value)) return;
            if (modifierField === "count") {
              modifier.count = Math.max(2, Math.min(6, Math.round(value)));
            } else if (modifierField === "offset") {
              modifier.offset = Number(value.toFixed(2));
            } else if (modifierField === "amount") {
              modifier.amount = Number(value.toFixed(2));
            } else if (modifierField === "width") {
              modifier.width = Number(Math.max(0.02, Math.min(0.42, value)).toFixed(2));
            } else if (modifierField === "levels") {
              modifier.levels = Math.max(1, Math.min(3, Math.round(value)));
            } else if (modifierField === "thickness") {
              modifier.thickness = Number(Math.max(0.02, Math.min(0.7, value)).toFixed(2));
            }
          }
          modifier.title = boomModifierTitle(modifier);
          refreshBoomMeshPreview(activeMesh);
          renderBoomSidebar();
          renderBoomViewportHud();
          return;
        }
        const active = boomItemById(boomScene.activeId);
        if (!active || active.id === "camera" || !active.transform) return;
        const field = target.dataset.field;
        if (!field) return;
        if (field === "mode" && target instanceof HTMLSelectElement) {
          active.transform.mode = target.value;
          return;
        }
        const axis = Number(target.dataset.axis);
        const nextValue = Number.parseFloat(target.value);
        if (!Number.isFinite(axis) || !Number.isFinite(nextValue)) return;
        if (!Array.isArray(active.transform[field])) return;
        const digits = field === "rotation" ? 1 : 3;
        active.transform[field][axis] = Number(nextValue.toFixed(digits));
      });
      boomSidebarBound = true;
    }
    renderBoomSidebar();
  }

  function flushBoomViewportHud() {
    if (!boomViewportHud) return false;
    const active = activeBoomItem();
    const activeMesh = activeBoomMeshItem();
    const activeMeshModifiers = activeMesh ? ensureBoomItemModifiers(activeMesh).filter((modifier) => modifier.enabled !== false) : [];
    const componentSummary = boomComponentSummary();
    const regionSummary = boomRegionSummary();
    const slicerChip = slicerPreviewEnabled() && slicerPreview
      ? `${slicerPreview.layerCount} layers preview`
      : (boomScene.propertyTab === "slicer" ? `${boomScene.slicer.workflow} mode` : "");
    const componentChip = componentSummary
      ? `${componentSummary.title} · ${componentSummary.subtitle}`
      : (boomScene.editMode === "object" ? "Object selection" : `Pick a ${boomScene.editMode}`);
    const html = `
      <div class="boom-modebar" role="toolbar" aria-label="BOOM edit mode">
        ${BOOM_EDIT_MODES.map((mode) => `
          <button class="boom-modebar-btn${boomScene.editMode === mode.id ? " is-active" : ""}" data-action="set-edit-mode" data-mode="${mode.id}" title="${mode.title} mode">
            ${mode.title}
          </button>
        `).join("")}
        <button class="boom-modebar-btn boom-modebar-btn-secondary${regionSummary ? " is-active" : ""}" data-action="select-volume-region" title="Build a KASM volume region from the active selection">
          Volume
        </button>
        <button class="boom-modebar-btn boom-modebar-btn-secondary" data-action="clear-region-selection" title="Clear the active KASM region"${regionSummary ? "" : " disabled"}>
          Clear
        </button>
      </div>
      <div class="boom-modebar-meta">
        <span class="boom-modebar-chip">${escapeBoomHtml(active?.name || "Selection")}</span>
        <span class="boom-modebar-chip${componentSummary ? " is-accent" : ""}">${escapeBoomHtml(componentChip)}</span>
        <span class="boom-modebar-chip${activeMeshModifiers.length ? " is-accent" : ""}">${activeMeshModifiers.length} modifier${activeMeshModifiers.length === 1 ? "" : "s"}</span>
        ${regionSummary ? `<span class="boom-modebar-chip is-accent">${escapeBoomHtml(`${regionSummary.title} | ${regionSummary.details[0][1]} cells`)}</span>` : ""}
        ${slicerChip ? `<span class="boom-modebar-chip is-accent">${escapeBoomHtml(slicerChip)}</span>` : ""}
      </div>
    `;
    const htmlHash = kasmHashString(`ui-hud|${html}`);
    if (boomViewportHudHtmlHash === htmlHash && boomViewportHud.dataset.boomHtmlHash === htmlHash) {
      boomUiRenderStats.hudSkips += 1;
      return false;
    }
    boomViewportHudHtmlHash = htmlHash;
    boomViewportHud.dataset.boomHtmlHash = htmlHash;
    boomViewportHud.innerHTML = html;
    boomUiRenderStats.hudFlushes += 1;
    return true;
  }

  function ensureBoomViewportHud() {
    if (!boomViewportHud || !boomViewportHud.isConnected) {
      boomViewportHud = document.createElement("div");
      boomViewportHud.className = "boom-viewport-hud";
      els.view.appendChild(boomViewportHud);
    }
    if (!boomViewportHudBound) {
      boomViewportHud.addEventListener("click", (event) => {
        const trigger = event.target.closest("[data-action]");
        if (!trigger) return;
        const action = trigger.dataset.action;
        if (action === "set-edit-mode") {
          executeBoomTool("boom.viewport.set_edit_mode", { mode: trigger.dataset.mode || "object" });
        } else if (action === "select-volume-region") {
          executeBoomTool("boom.query.volume_region_from_selection", { activate: true });
          rebuildBoomSlicerPreview();
        } else if (action === "clear-region-selection") {
          executeBoomTool("boom.region.clear", {});
          rebuildBoomSlicerPreview();
        }
      });
      boomViewportHudBound = true;
    }
    renderBoomViewportHud();
  }

  function meshRenderPasses(mesh) {
    const passModifiers = ensureBoomItemModifiers(findBoomItem("imported-mesh"))
      .filter((modifier) => modifier.enabled !== false && (modifier.type === "inflate" || modifier.type === "array" || modifier.type === "mirror"));
    const passKey = kasmHashString(`render-passes|${stableBoomStringify({
      transform: mesh?.transform || {},
      color: mesh?.color || [0.84, 0.85, 0.90],
      modifiers: passModifiers.map(boomModifierCachePayload),
    })}`);
    if (mesh?._renderPassesKey === passKey && Array.isArray(mesh._renderPasses)) {
      if (mesh._renderPassesHitLogKey !== passKey) {
        emitBoomAudit("render_passes", "HIT", passKey, 0, mesh._renderPasses.length, "passes");
        mesh._renderPassesHitLogKey = passKey;
      }
      return mesh._renderPasses;
    }
    const started = boomNowMs();
    const baseTransform = {
      location: [...(mesh?.transform?.location || [0, 0, 0])],
      rotation: [...(mesh?.transform?.rotation || [0, 0, 0])],
      scale: [...(mesh?.transform?.scale || [1, 1, 1])],
    };
    let passes = [{ transform: baseTransform, color: mesh?.color || [0.84, 0.85, 0.90] }];
    for (const modifier of passModifiers) {
      if (modifier.type === "inflate") {
        const amount = Number(modifier.amount || 1);
        passes = passes.map((pass) => ({
          ...pass,
          transform: {
            ...pass.transform,
            scale: pass.transform.scale.map((value) => Number((value * amount).toFixed(4))),
          },
        }));
      } else if (modifier.type === "array") {
        const axis = modifierAxisIndex(modifier.axis);
        const count = Math.max(2, Math.min(6, Math.round(Number(modifier.count || 2))));
        const offset = Number(modifier.offset || 0);
        const nextPasses = [];
        for (const pass of passes) {
          for (let i = 0; i < count; i += 1) {
            const transform = {
              ...pass.transform,
              location: [...pass.transform.location],
              rotation: [...pass.transform.rotation],
              scale: [...pass.transform.scale],
            };
            transform.location[axis] = Number((transform.location[axis] + offset * i).toFixed(4));
            nextPasses.push({
              transform,
              color: pass.color,
            });
          }
        }
        passes = nextPasses;
      } else if (modifier.type === "mirror") {
        const axis = modifierAxisIndex(modifier.axis);
        const nextPasses = [...passes];
        for (const pass of passes) {
          const transform = {
            ...pass.transform,
            location: [...pass.transform.location],
            rotation: [...pass.transform.rotation],
            scale: [...pass.transform.scale],
          };
          transform.scale[axis] = Number((transform.scale[axis] * -1).toFixed(4));
          nextPasses.push({
            transform,
            color: pass.color,
          });
        }
        passes = nextPasses;
      }
    }
    if (mesh) {
      for (const pass of passes) {
        pass.model = meshModelMatrix({ transform: pass.transform });
      }
      mesh._renderPassesKey = passKey;
      mesh._renderPasses = passes;
      mesh._renderPassesHitLogKey = "";
    }
    emitBoomAudit("render_passes", "MISS", passKey, boomNowMs() - started, passes.length, "passes");
    return passes;
  }

  function updateBoomMeshStats() {
    if (els.statVerts) els.statVerts.textContent = sceneMesh?.vertexCount ? String(sceneMesh.vertexCount) : "0";
    if (els.statFaces) els.statFaces.textContent = sceneMesh?.faceCount ? String(sceneMesh.faceCount) : "0";
  }

  function removeImportedSceneItem() {
    const idx = boomScene.items.findIndex((item) => item.id === "imported-mesh");
    if (idx >= 0) boomScene.items.splice(idx, 1);
    if (boomScene.activeId === "imported-mesh") {
      boomScene.activeId = "grid";
    }
  }

  function releaseSceneMesh() {
    const importedItem = findBoomItem("imported-mesh");
    releaseBoomSlicerPreview();
    releaseBoomDerivedMesh(sceneMesh);
    if (gl && sceneMesh?.vao) {
      try {
        for (const buffer of sceneMesh.buffers || []) gl.deleteBuffer(buffer);
        gl.deleteVertexArray(sceneMesh.vao);
      } catch (err) {
        console.warn("[banger] releaseSceneMesh error:", err);
      }
    }
    applyBoomKasmGraph(null, importedItem);
    clearBoomAnimationState();
    clearBoomComponentSelection();
    clearBoomPickHandle("scene-release");
    sceneMesh = null;
    removeImportedSceneItem();
    updateBoomMeshStats();
    renderBoomSidebar();
    renderBoomViewportHud();
  }

  function meshModelMatrix(mesh) {
    const transform = mesh?.transform || {};
    const location = transform.location || [0, 0, 0];
    const rotation = transform.rotation || [0, 0, 0];
    const scale = transform.scale || [1, 1, 1];
    const rx = rotation[0] * Math.PI / 180;
    const ry = rotation[1] * Math.PI / 180;
    const rz = rotation[2] * Math.PI / 180;
    const sx = scale[0] ?? 1;
    const sy = scale[1] ?? 1;
    const sz = scale[2] ?? 1;
    const cx = Math.cos(rx), sxr = Math.sin(rx);
    const cy = Math.cos(ry), syr = Math.sin(ry);
    const cz = Math.cos(rz), szr = Math.sin(rz);
    const m = new Float32Array(16);
    m[0]  = cz * cy * sx;
    m[1]  = szr * cy * sx;
    m[2]  = -syr * sx;
    m[3]  = 0;
    m[4]  = (cz * syr * sxr - szr * cx) * sy;
    m[5]  = (szr * syr * sxr + cz * cx) * sy;
    m[6]  = cy * sxr * sy;
    m[7]  = 0;
    m[8]  = (cz * syr * cx + szr * sxr) * sz;
    m[9]  = (szr * syr * cx - cz * sxr) * sz;
    m[10] = cy * cx * sz;
    m[11] = 0;
    m[12] = location[0] ?? 0;
    m[13] = location[1] ?? 0;
    m[14] = location[2] ?? 0;
    m[15] = 1;
    return m;
  }

  function ensureBoomDropOverlay() {
    if (boomDropOverlay && boomDropOverlay.isConnected) return boomDropOverlay;
    boomDropOverlay = document.createElement("div");
    boomDropOverlay.className = "banger-drop-overlay";
    boomDropOverlay.setAttribute("aria-hidden", "true");
    boomDropOverlay.innerHTML = `
      <div class="dropbox canvas-dropbox banger-dropbox" role="presentation">
        <span class="dropbox-corner tl" aria-hidden="true"></span>
        <span class="dropbox-corner tr" aria-hidden="true"></span>
        <span class="dropbox-corner bl" aria-hidden="true"></span>
        <span class="dropbox-corner br" aria-hidden="true"></span>
        <div class="dropbox-icon" aria-hidden="true">
          <svg viewBox="0 0 64 64" focusable="false">
            <path class="dropbox-icon-chevron" d="M32 6v20" />
            <path class="dropbox-icon-chevron" d="m24 18 8 8 8-8" />
            <path d="M8 34v16a4 4 0 0 0 4 4h40a4 4 0 0 0 4-4V34" />
            <path d="M8 34h14l4 6h12l4-6h14" />
          </svg>
        </div>
        <p class="dropbox-title">Drop 3D file in scene</p>
        <p class="dropbox-sub">OBJ, STL, PLY, OFF, glTF and GLB directly. FBX, DAE, 3DS, 3MF, USD and more can be normalized by the backend into BOOM preview.</p>
      </div>
    `;
    els.view.appendChild(boomDropOverlay);
    return boomDropOverlay;
  }

  function setBoomDropActive(active) {
    const overlay = ensureBoomDropOverlay();
    overlay.classList.toggle("is-active", !!active);
    overlay.setAttribute("aria-hidden", active ? "false" : "true");
    els.view.classList.toggle("is-drop-target", !!active);
  }

  function syncImportedSceneItem(fileName) {
    const existing = findBoomItem("imported-mesh");
    const label = boomImportedMeshLabel(fileName);
    const target = existing
      ? existing
      : {
          id: "imported-mesh",
          name: label,
          type: "mesh",
          visible: true,
          selectable: true,
          renderable: true,
          modifiers: [],
          meta: {
            imported: true,
            sourceName: label,
            vertexCount: 0,
            faceCount: 0,
          },
          transform: {
            location: [0, 0, 0],
            rotation: [0, 0, 0],
            scale: [1, 1, 1],
            mode: "XYZ Euler",
          },
        };
    target.name = label;
    target.type = "mesh";
    target.meta = {
      ...(target.meta || {}),
      imported: true,
      sourceName: label,
      animationSourceName: target.meta?.animationSourceName || "",
    };
    target.modifiers = ensureBoomItemModifiers(target);
    target.selectable = true;
    target.visible = target.visible !== false;
    target.renderable = target.renderable !== false;
    if (!boomScene.items.some((item) => item.id === "imported-mesh")) {
      const insertAt = Math.max(0, boomScene.items.findIndex((item) => item.id === "light"));
      boomScene.items.splice(insertAt, 0, target);
    }
    boomScene.collectionExpanded = true;
    boomScene.filter = "";
    boomScene.activeId = "imported-mesh";
    renderBoomSidebar();
    return target;
  }

  function createSceneMesh(meshData, fileName) {
    if (!meshData?.pos?.length || !meshData?.nrm?.length) return false;
    releaseSceneMesh();
    const item = syncImportedSceneItem(fileName);
    item.meta = {
      ...(item.meta || {}),
      imported: true,
      sourceName: item.name,
      vertexCount: meshData.count || 0,
      faceCount: meshData.faceCount || 0,
    };
    if (!gl) {
      sceneMesh = {
        ...meshData,
        vao: null,
        buffers: [],
        base: {
          pos: meshData.pos,
          nrm: meshData.nrm,
          count: meshData.count,
          faceCount: meshData.faceCount,
          source: meshData.source || null,
          normalizeView: meshData.normalizeView || null,
        },
        vertexCount: meshData.count,
        faceCount: meshData.faceCount,
        color: [0.84, 0.85, 0.90],
        visible: item.visible,
        transform: item.transform,
      };
      clearBoomComponentSelection();
      clearBoomRegionSelection();
      rebuildBoomDisplayMesh(sceneMesh);
      syncBoomKasmGraph(item, sceneMesh);
      rebuildBoomSlicerPreview();
      updateBoomMeshStats();
      renderBoomSidebar();
      renderBoomViewportHud();
      return true;
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const posBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, meshData.pos, gl.STATIC_DRAW);
    const aPosM = gl.getAttribLocation(meshProg, "aPos");
    gl.enableVertexAttribArray(aPosM);
    gl.vertexAttribPointer(aPosM, 3, gl.FLOAT, false, 0, 0);
    const nrmBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, nrmBuf);
    gl.bufferData(gl.ARRAY_BUFFER, meshData.nrm, gl.STATIC_DRAW);
    const aNormalM = gl.getAttribLocation(meshProg, "aNormal");
    gl.enableVertexAttribArray(aNormalM);
    gl.vertexAttribPointer(aNormalM, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    sceneMesh = {
      ...meshData,
      vao,
      buffers: [posBuf, nrmBuf],
      base: {
        pos: meshData.pos,
        nrm: meshData.nrm,
        count: meshData.count,
        faceCount: meshData.faceCount,
        source: meshData.source || null,
        normalizeView: meshData.normalizeView || null,
      },
      vertexCount: meshData.count,
      faceCount: meshData.faceCount,
      color: [0.84, 0.85, 0.90],
      visible: item.visible,
      transform: item.transform,
    };
    clearBoomComponentSelection();
    clearBoomRegionSelection();
    rebuildBoomDisplayMesh(sceneMesh);
    syncBoomKasmGraph(item, sceneMesh);
    rebuildBoomSlicerPreview();
    updateBoomMeshStats();
    renderBoomSidebar();
    renderBoomViewportHud();
    return true;
  }

  function applyBoomAnimationImport(animationSpec, fileName) {
    if (!animationSpec?.meshData) return false;
    if (!createSceneMesh(animationSpec.meshData, animationSpec.name || fileName)) return false;
    const item = findBoomItem("imported-mesh");
    if (item?.transform) {
      item.transform = cloneBoomTransform(animationSpec.transform || item.transform);
      item.meta = {
        ...(item.meta || {}),
        animationSourceName: fileName || "",
      };
      const modifiers = ensureBoomItemModifiers(item);
      modifiers.splice(0, modifiers.length, ...(animationSpec.modifiers || []).map((entry) => stableBoomValue(entry)));
      refreshBoomMeshPreview(item);
    }
    setBoomAnimationState(animationSpec.animation, fileName || "");
    renderBoomSidebar();
    renderBoomViewportHud();
    return true;
  }

  async function parseBoom3dFile(file, companionFiles = []) {
    const name = String(file?.name || "");
    if (!isBoom3dFileName(name)) return null;
    const lower = name.toLowerCase();
    if (lower.endsWith(".obj")) {
      const text = await file.text();
      return parseObjMesh(text, boomImportSourceMeta("obj", name, [boomTextSourcePart(text, name)]));
    }
    if (lower.endsWith(".ply")) {
      const text = await file.text();
      return parseAsciiPly(text, boomImportSourceMeta("ply-ascii", name, [boomTextSourcePart(text, name)]));
    }
    if (lower.endsWith(".off")) {
      const text = await file.text();
      return parseOffMesh(text, boomImportSourceMeta("off", name, [boomTextSourcePart(text, name)]));
    }
    if (lower.endsWith(".gltf")) {
      return parseGltfMesh(file, companionFiles);
    }
    if (lower.endsWith(".glb")) {
      return parseGlbMesh(file);
    }
    if (lower.endsWith(".stl")) {
      const buffer = await file.arrayBuffer();
      const sourcePart = boomBufferSourcePart(buffer, name);
      const header = new TextDecoder().decode(buffer.slice(0, Math.min(buffer.byteLength, 256)));
      if (/^\s*solid\b/i.test(header) && /facet\s+normal/i.test(header)) {
        const text = new TextDecoder().decode(buffer);
        return parseAsciiStl(text, boomImportSourceMeta("stl-ascii", name, [sourcePart]));
      }
      return parseBinaryStl(buffer, boomImportSourceMeta("stl-binary", name, [sourcePart]));
    }
    return null;
  }

  function arrayBufferToBase64(buffer) {
    const bytes = new Uint8Array(buffer);
    let binary = "";
    const chunk = 0x8000;
    for (let i = 0; i < bytes.length; i += chunk) {
      binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    return btoa(binary);
  }

  function base64ToArrayBuffer(base64) {
    const binary = atob(String(base64 || ""));
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
    return bytes.buffer;
  }

  async function normalizeBoomImportWithBackend(fileListLike) {
    if (!tauriInvoke) return null;
    const incoming = Array.from(fileListLike || []).filter(Boolean);
    if (!incoming.length || !incoming.some((file) => isBoom3dCandidateName(file.name))) return null;
    const files = [];
    for (const file of incoming) {
      const buffer = await file.arrayBuffer();
      files.push({
        name: String(file.name || "import.bin"),
        bytesB64: arrayBufferToBase64(buffer),
      });
    }
    const normalized = await backendInvoke("banger_normalize_import_files", {
      request: { files },
    });
    if (!normalized?.converted || !normalized.outputBytesB64) {
      if (Array.isArray(normalized?.warnings) && normalized.warnings.length) {
        console.warn("[banger] backend normalization warnings:", normalized.warnings.join(" | "));
      }
      return null;
    }
    if (Array.isArray(normalized.warnings) && normalized.warnings.length) {
      console.info("[banger] backend normalization:", normalized.warnings.join(" | "));
    }
    const glbFile = new File(
      [base64ToArrayBuffer(normalized.outputBytesB64)],
      normalized.outputName || "boom-normalized.glb",
      { type: "model/gltf-binary" }
    );
    const meshData = await parseGlbMesh(glbFile);
    if (!meshData) return null;
    if (!createSceneMesh(meshData, normalized.sourceName || normalized.outputName || glbFile.name)) return null;
    return normalized;
  }

  async function previewBoom3dFiles(fileListLike) {
    const incoming = Array.from(fileListLike || []).filter(Boolean);
    if (!incoming.length || !isViewVisible()) return { shown: 0, unsupported: [] };
    const animationFiles = incoming.filter((file) => isBoomAnimationFileName(file.name));
    for (const file of animationFiles) {
      try {
        const animationSpec = await parseBoomAnimationFile(file);
        if (animationSpec && applyBoomAnimationImport(animationSpec, file.name)) {
          return { shown: 1, unsupported: [] };
        }
      } catch (err) {
        console.warn("[banger] animation import failed:", file.name, err);
      }
    }
    const supported = incoming.filter((file) => isBoom3dFileName(file.name));
    const candidates = incoming.filter((file) => isBoom3dCandidateName(file.name));
    const unsupported = incoming.filter((file) => !isBoomSceneBridgeFileName(file.name)).map((file) => file.name);
    for (const file of supported) {
      try {
        const meshData = await parseBoom3dFile(file, incoming);
        if (meshData && createSceneMesh(meshData, file.name)) {
          return { shown: 1, unsupported };
        }
      } catch (err) {
        console.warn("[banger] 3d preview failed:", file.name, err);
      }
    }
    try {
      const normalized = await normalizeBoomImportWithBackend(candidates);
      if (normalized) {
        return { shown: 1, unsupported };
      }
    } catch (err) {
      console.warn("[banger] backend normalization failed:", err);
    }
    return { shown: 0, unsupported };
  }

  function initGL() {
    gl = els.canvas.getContext("webgl2", { antialias: true, alpha: true, premultipliedAlpha: false });
    if (!gl) {
      console.error("[banger] WebGL2 not available");
      return false;
    }
    const vsM = compile(gl, gl.VERTEX_SHADER, VS_MESH);
    const fsM = compile(gl, gl.FRAGMENT_SHADER, FS_MESH);
    const vsL = compile(gl, gl.VERTEX_SHADER, VS_LINE);
    const fsL = compile(gl, gl.FRAGMENT_SHADER, FS_LINE);
    const vsS = compile(gl, gl.VERTEX_SHADER, VS_SDF);
    const fsS = compile(gl, gl.FRAGMENT_SHADER, FS_SDF);
    if (!vsM || !fsM || !vsL || !fsL || !vsS || !fsS) return false;
    meshProg = link(gl, vsM, fsM);
    lineProg = link(gl, vsL, fsL);
    sdfProg  = link(gl, vsS, fsS);
    if (!meshProg || !lineProg || !sdfProg) return false;

    uMeshModel = gl.getUniformLocation(meshProg, "uModel");
    uMeshProj  = gl.getUniformLocation(meshProg, "uProj");
    uMeshView  = gl.getUniformLocation(meshProg, "uView");
    uMeshColor = gl.getUniformLocation(meshProg, "uColor");
    uMeshClipOffset = gl.getUniformLocation(meshProg, "uClipOffset");
    uLineProj  = gl.getUniformLocation(lineProg, "uProj");
    uLineView  = gl.getUniformLocation(lineProg, "uView");
    uLineFadeNear = gl.getUniformLocation(lineProg, "uFadeNear");
    uLineFadeFar  = gl.getUniformLocation(lineProg, "uFadeFar");
    uLineClipOffset = gl.getUniformLocation(lineProg, "uClipOffset");
    uSdfResolution  = gl.getUniformLocation(sdfProg, "uResolution");
    uSdfCameraPos   = gl.getUniformLocation(sdfProg, "uCameraPos");
    uSdfCameraFwd   = gl.getUniformLocation(sdfProg, "uCameraFwd");
    uSdfCameraRight = gl.getUniformLocation(sdfProg, "uCameraRight");
    uSdfCameraUp    = gl.getUniformLocation(sdfProg, "uCameraUp");
    uSdfTanHalfFovY = gl.getUniformLocation(sdfProg, "uTanHalfFovY");
    uSdfViewProj    = gl.getUniformLocation(sdfProg, "uViewProj");

    // cube VAO
    const cube = makeCube();
    cubeCount = cube.count;
    cubeVAO = gl.createVertexArray();
    gl.bindVertexArray(cubeVAO);
    const cubePosBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, cubePosBuf);
    gl.bufferData(gl.ARRAY_BUFFER, cube.pos, gl.STATIC_DRAW);
    const aPosM = gl.getAttribLocation(meshProg, "aPos");
    gl.enableVertexAttribArray(aPosM);
    gl.vertexAttribPointer(aPosM, 3, gl.FLOAT, false, 0, 0);
    const cubeNrmBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, cubeNrmBuf);
    gl.bufferData(gl.ARRAY_BUFFER, cube.nrm, gl.STATIC_DRAW);
    const aNormalM = gl.getAttribLocation(meshProg, "aNormal");
    gl.enableVertexAttribArray(aNormalM);
    gl.vertexAttribPointer(aNormalM, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    cubeBuffers = [cubePosBuf, cubeNrmBuf];

    // grid VAO
    const grid = makeGrid(320, 1);
    gridCount = grid.count;
    gridVAO = gl.createVertexArray();
    gl.bindVertexArray(gridVAO);
    const gridPosBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, gridPosBuf);
    gl.bufferData(gl.ARRAY_BUFFER, grid.pos, gl.STATIC_DRAW);
    const aPosL = gl.getAttribLocation(lineProg, "aPos");
    gl.enableVertexAttribArray(aPosL);
    gl.vertexAttribPointer(aPosL, 3, gl.FLOAT, false, 0, 0);
    const gridColBuf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, gridColBuf);
    gl.bufferData(gl.ARRAY_BUFFER, grid.col, gl.STATIC_DRAW);
    const aColorL = gl.getAttribLocation(lineProg, "aColor");
    gl.enableVertexAttribArray(aColorL);
    gl.vertexAttribPointer(aColorL, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    gridBuffers = [gridPosBuf, gridColBuf];

    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    if (sceneMesh && !sceneMesh.vao && sceneMesh.pos?.length && sceneMesh.nrm?.length) {
      createSceneMesh({
        pos: sceneMesh.base?.pos || sceneMesh.pos,
        nrm: sceneMesh.base?.nrm || sceneMesh.nrm,
        count: sceneMesh.base?.count || sceneMesh.count,
        faceCount: sceneMesh.base?.faceCount || sceneMesh.faceCount,
        bounds: sceneMesh.bounds,
      }, findBoomItem("imported-mesh")?.name || "Imported mesh");
    }
    rebuildBoomSlicerPreview();
    return true;
  }

  function stopRenderLoop() {
    if (raf) { cancelAnimationFrame(raf); raf = 0; }
  }

  // Release every GPU-side resource. We only do this on full shutdown.
  // On simple blur/visibility changes we keep the WebGL context alive,
  // which avoids black/white restores on some drivers when the app regains focus.
  function releaseGL() {
    stopRenderLoop();
    if (!gl) return;
    try {
      releaseBoomSlicerPreview();
      releaseBoomDerivedMesh(sceneMesh);
      clearBoomGpuResourceCache();
      if (sceneMesh?.vao) {
        for (const buffer of sceneMesh.buffers || []) gl.deleteBuffer(buffer);
        gl.deleteVertexArray(sceneMesh.vao);
        sceneMesh.vao = null;
        sceneMesh.buffers = [];
      }
      for (const b of cubeBuffers) gl.deleteBuffer(b);
      for (const b of gridBuffers) gl.deleteBuffer(b);
      if (cubeVAO) gl.deleteVertexArray(cubeVAO);
      if (gridVAO) gl.deleteVertexArray(gridVAO);
      if (meshProg) gl.deleteProgram(meshProg);
      if (lineProg) gl.deleteProgram(lineProg);
    } catch (err) {
      console.warn("[banger] releaseGL error:", err);
    }
    cubeBuffers = []; gridBuffers = [];
    cubeVAO = null; gridVAO = null;
    meshProg = null; lineProg = null; sdfProg = null;
    cubeCount = 0; gridCount = 0;
    gl = null;
  }

  function resize() {
    if (!gl) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const rect = els.canvas.getBoundingClientRect();
    const w = Math.max(2, Math.floor(rect.width * dpr));
    const h = Math.max(2, Math.floor(rect.height * dpr));
    if (els.canvas.width !== w || els.canvas.height !== h) {
      els.canvas.width = w;
      els.canvas.height = h;
    }
    gl.viewport(0, 0, w, h);
  }

  function cameraEye() {
    const ce = Math.cos(camera.elevation);
    const se = Math.sin(camera.elevation);
    const ca = Math.cos(camera.azimuth);
    const sa = Math.sin(camera.azimuth);
    return [
      camera.target[0] + camera.distance * ce * ca,
      camera.target[1] + camera.distance * ce * sa,
      camera.target[2] + camera.distance * se,
    ];
  }

  function cameraPanBasis() {
    const eye = cameraEye();
    let fx = camera.target[0] - eye[0];
    let fy = camera.target[1] - eye[1];
    let fz = camera.target[2] - eye[2];
    let fl = Math.hypot(fx, fy, fz) || 1;
    fx /= fl; fy /= fl; fz /= fl;

    let rx = fy;
    let ry = -fx;
    let rz = 0;
    let rl = Math.hypot(rx, ry, rz);
    if (rl < 1e-5) {
      rx = 1; ry = 0; rz = 0;
      rl = 1;
    }
    rx /= rl; ry /= rl; rz /= rl;

    let ux = ry * fz - rz * fy;
    let uy = rz * fx - rx * fz;
    let uz = rx * fy - ry * fx;
    let ul = Math.hypot(ux, uy, uz) || 1;
    ux /= ul; uy /= ul; uz /= ul;

    return {
      right: [rx, ry, rz],
      up: [ux, uy, uz],
    };
  }

  function cameraViewBasis() {
    const eye = cameraEye();
    let fx = camera.target[0] - eye[0];
    let fy = camera.target[1] - eye[1];
    let fz = camera.target[2] - eye[2];
    let fl = Math.hypot(fx, fy, fz) || 1;
    fx /= fl; fy /= fl; fz /= fl;

    const worldUp = [0, 0, 1];
    let rx = fy * worldUp[2] - fz * worldUp[1];
    let ry = fz * worldUp[0] - fx * worldUp[2];
    let rz = fx * worldUp[1] - fy * worldUp[0];
    let rl = Math.hypot(rx, ry, rz);
    if (rl < 1e-5) {
      rx = 1; ry = 0; rz = 0;
      rl = 1;
    }
    rx /= rl; ry /= rl; rz /= rl;

    let ux = ry * fz - rz * fy;
    let uy = rz * fx - rx * fz;
    let uz = rx * fy - ry * fx;
    let ul = Math.hypot(ux, uy, uz) || 1;
    ux /= ul; uy /= ul; uz /= ul;

    return {
      eye,
      forward: [fx, fy, fz],
      right: [rx, ry, rz],
      up: [ux, uy, uz],
    };
  }

  function screenRayFromClientPoint(clientX, clientY) {
    const rect = els.canvas.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const [clipOffsetX, clipOffsetY] = lastClipOffset || canvasCenterClipOffset();
    const localX = ((clientX - rect.left) / rect.width) * 2 - 1;
    const localY = 1 - ((clientY - rect.top) / rect.height) * 2;
    const ndcX = localX - clipOffsetX;
    const ndcY = localY - clipOffsetY;
    const aspect = rect.width / rect.height;
    const tanHalf = Math.tan((46 * Math.PI / 180) * 0.5);
    const basis = cameraViewBasis();
    const vx = ndcX * tanHalf * aspect;
    const vy = ndcY * tanHalf;
    const vz = -1;
    const wx = basis.right[0] * vx + basis.up[0] * vy + basis.forward[0] * (-vz);
    const wy = basis.right[1] * vx + basis.up[1] * vy + basis.forward[1] * (-vz);
    const wz = basis.right[2] * vx + basis.up[2] * vy + basis.forward[2] * (-vz);
    const len = Math.hypot(wx, wy, wz) || 1;
    return {
      origin: basis.eye,
      dir: [wx / len, wy / len, wz / len],
    };
  }

  function transformPointWithModel(model, x, y, z) {
    return [
      model[0] * x + model[4] * y + model[8] * z + model[12],
      model[1] * x + model[5] * y + model[9] * z + model[13],
      model[2] * x + model[6] * y + model[10] * z + model[14],
    ];
  }

  function projectWorldToCanvasPointFast(world, rect, proj = lastProj, view = lastView, clipOffset = lastClipOffset) {
    if (!proj || !view || !rect?.width || !rect?.height) return null;
    const viewPos = M4.transformVec4(view, world[0], world[1], world[2], 1);
    const clip = M4.transformVec4(proj, viewPos[0], viewPos[1], viewPos[2], viewPos[3]);
    if (Math.abs(clip[3]) < 1e-6) return null;
    clip[0] += clipOffset[0] * clip[3];
    clip[1] += clipOffset[1] * clip[3];
    const ndcX = clip[0] / clip[3];
    const ndcY = clip[1] / clip[3];
    return {
      x: ((ndcX + 1) * 0.5) * rect.width,
      y: ((1 - ndcY) * 0.5) * rect.height,
      depth: viewPos[2],
    };
  }

  function intersectRayTriangle(origin, dir, a, b, c) {
    const eps = 1e-6;
    const abx = b[0] - a[0], aby = b[1] - a[1], abz = b[2] - a[2];
    const acx = c[0] - a[0], acy = c[1] - a[1], acz = c[2] - a[2];
    const px = dir[1] * acz - dir[2] * acy;
    const py = dir[2] * acx - dir[0] * acz;
    const pz = dir[0] * acy - dir[1] * acx;
    const det = abx * px + aby * py + abz * pz;
    if (Math.abs(det) < eps) return null;
    const invDet = 1 / det;
    const tx = origin[0] - a[0], ty = origin[1] - a[1], tz = origin[2] - a[2];
    const u = (tx * px + ty * py + tz * pz) * invDet;
    if (u < 0 || u > 1) return null;
    const qx = ty * abz - tz * aby;
    const qy = tz * abx - tx * abz;
    const qz = tx * aby - ty * abx;
    const v = (dir[0] * qx + dir[1] * qy + dir[2] * qz) * invDet;
    if (v < 0 || u + v > 1) return null;
    const t = (acx * qx + acy * qy + acz * qz) * invDet;
    return t > eps ? t : null;
  }

  function buildProjectedFaceIndex(pos, passes, rect) {
    const faceCount = Math.floor((pos?.length || 0) / 9);
    const maxEntries = faceCount * Math.max(1, passes.length);
    const boxes = new Float32Array(maxEntries * 5);
    const refs = new Uint32Array(maxEntries * 2);
    let count = 0;
    for (let passIndex = 0; passIndex < passes.length; passIndex += 1) {
      const model = passes[passIndex].model || boomComponentTransform(passIndex);
      for (let faceIndex = 0; faceIndex < faceCount; faceIndex += 1) {
        const i = faceIndex * 9;
        const a = projectWorldToCanvasPointFast(transformPointWithModel(model, pos[i], pos[i + 1], pos[i + 2]), rect);
        const b = projectWorldToCanvasPointFast(transformPointWithModel(model, pos[i + 3], pos[i + 4], pos[i + 5]), rect);
        const c = projectWorldToCanvasPointFast(transformPointWithModel(model, pos[i + 6], pos[i + 7], pos[i + 8]), rect);
        if (!a || !b || !c) continue;
        const boxOffset = count * 5;
        boxes[boxOffset] = Math.min(a.x, b.x, c.x);
        boxes[boxOffset + 1] = Math.min(a.y, b.y, c.y);
        boxes[boxOffset + 2] = Math.max(a.x, b.x, c.x);
        boxes[boxOffset + 3] = Math.max(a.y, b.y, c.y);
        boxes[boxOffset + 4] = (a.depth + b.depth + c.depth) / 3;
        const refOffset = count * 2;
        refs[refOffset] = passIndex;
        refs[refOffset + 1] = faceIndex;
        count += 1;
      }
    }
    return {
      boxes: boxes.slice(0, count * 5),
      refs: refs.slice(0, count * 2),
      count,
      sourceFaces: faceCount,
    };
  }

  function buildProjectedVertexItems(passes, rect) {
    if (!boomKasmGraph?.vertices?.length) return [];
    const items = [];
    for (let passIndex = 0; passIndex < passes.length; passIndex += 1) {
      const model = passes[passIndex].model || boomComponentTransform(passIndex);
      for (let index = 0; index < boomKasmGraph.vertices.length; index += 1) {
        const vertex = boomKasmGraph.vertices[index];
        const world = transformPointWithModel(model, vertex.position[0], vertex.position[1], vertex.position[2]);
        const screen = projectWorldToCanvasPointFast(world, rect);
        if (!screen) continue;
        items.push({ nodeId: vertex.id, index, passIndex, x: screen.x, y: screen.y, depth: screen.depth });
      }
    }
    return items;
  }

  function buildProjectedEdgeItems(passes, rect) {
    if (!boomKasmGraph?.edges?.length) return [];
    const vertexMap = boomKasmVertexMap();
    const items = [];
    for (let passIndex = 0; passIndex < passes.length; passIndex += 1) {
      const model = passes[passIndex].model || boomComponentTransform(passIndex);
      for (let index = 0; index < boomKasmGraph.edges.length; index += 1) {
        const edge = boomKasmGraph.edges[index];
        const va = vertexMap.get(edge.vertices[0]);
        const vb = vertexMap.get(edge.vertices[1]);
        if (!va || !vb) continue;
        const aw = transformPointWithModel(model, va.position[0], va.position[1], va.position[2]);
        const bw = transformPointWithModel(model, vb.position[0], vb.position[1], vb.position[2]);
        const as = projectWorldToCanvasPointFast(aw, rect);
        const bs = projectWorldToCanvasPointFast(bw, rect);
        if (!as || !bs) continue;
        items.push({
          nodeId: edge.id,
          index,
          passIndex,
          ax: as.x,
          ay: as.y,
          bx: bs.x,
          by: bs.y,
          depth: (as.depth + bs.depth) * 0.5,
        });
      }
    }
    return items;
  }

  function boomPickHandleKey(rect, passes, meshGeometry, componentGeometry) {
    return kasmHashString(`pick-handle-v1|${Math.round(rect.width)}x${Math.round(rect.height)}|${boomGeometryHash(meshGeometry)}|${boomGeometryHash(componentGeometry)}|${boomKasmGraph?.object?.hash || "none"}|${sceneMesh?._renderPassesKey || passes.length}|${boomHashFloatArray(lastProj || [], "proj")}|${boomHashFloatArray(lastView || [], "view")}|${stableBoomStringify(lastClipOffset || [0, 0])}`);
  }

  function getBoomPickHandle() {
    if (!sceneMesh?.pos?.length || !lastProj || !lastView) return null;
    const rect = els.canvas.getBoundingClientRect();
    if (!rect.width || !rect.height) return null;
    const passes = meshRenderPasses(sceneMesh);
    const meshGeometry = sceneMesh.display?.pos?.length ? sceneMesh.display : sceneMesh;
    const componentGeometry = sceneMesh;
    const key = boomPickHandleKey(rect, passes, meshGeometry, componentGeometry);
    const started = boomNowMs();
    if (boomPickHandle?.key === key) {
      boomPickHandleStats.hits += 1;
      emitBoomAudit("pick_handle", "HIT", key, boomNowMs() - started, boomPickHandle.meshFaces?.count || 0, "screen_faces", {
        bytes: boomPickHandleStats.bytes,
      });
      return boomPickHandle;
    }
    const meshFaces = buildProjectedFaceIndex(meshGeometry.pos, passes, rect);
    const componentFaces = buildProjectedFaceIndex(componentGeometry.pos, passes, rect);
    const vertices = buildProjectedVertexItems(passes, rect);
    const edges = buildProjectedEdgeItems(passes, rect);
    const bytes = (meshFaces.boxes.byteLength || 0)
      + (meshFaces.refs.byteLength || 0)
      + (componentFaces.boxes.byteLength || 0)
      + (componentFaces.refs.byteLength || 0)
      + vertices.length * 48
      + edges.length * 56;
    boomPickHandle = { key, rect: { width: rect.width, height: rect.height }, passes, meshGeometry, componentGeometry, meshFaces, componentFaces, vertices, edges };
    boomPickHandleStats.misses += 1;
    boomPickHandleStats.bytes = bytes;
    boomPickHandleStats.lastBuildMs = Number((boomNowMs() - started).toFixed(3));
    boomPickHandleStats.lastKey = key;
    emitBoomAudit("pick_handle", "MISS", key, boomNowMs() - started, meshFaces.count + componentFaces.count + vertices.length + edges.length, "pick_items", {
      bytes,
      meshFaces: meshFaces.count,
      componentFaces: componentFaces.count,
      vertices: vertices.length,
      edges: edges.length,
    });
    return boomPickHandle;
  }

  function pickFaceCandidates(faceIndex, localX, localY, margin = 3) {
    const candidates = [];
    if (!faceIndex?.count) return candidates;
    const boxes = faceIndex.boxes;
    const refs = faceIndex.refs;
    for (let entry = 0; entry < faceIndex.count; entry += 1) {
      const boxOffset = entry * 5;
      if (
        localX < boxes[boxOffset] - margin
        || localY < boxes[boxOffset + 1] - margin
        || localX > boxes[boxOffset + 2] + margin
        || localY > boxes[boxOffset + 3] + margin
      ) {
        continue;
      }
      const refOffset = entry * 2;
      candidates.push({ passIndex: refs[refOffset], faceIndex: refs[refOffset + 1] });
    }
    return candidates;
  }

  function intersectIndexedFace(pos, passes, ray, passIndex, faceIndex) {
    const i = faceIndex * 9;
    const model = passes[passIndex]?.model || boomComponentTransform(passIndex);
    const a = transformPointWithModel(model, pos[i], pos[i + 1], pos[i + 2]);
    const b = transformPointWithModel(model, pos[i + 3], pos[i + 4], pos[i + 5]);
    const c = transformPointWithModel(model, pos[i + 6], pos[i + 7], pos[i + 8]);
    return intersectRayTriangle(ray.origin, ray.dir, a, b, c);
  }

  function pickSceneMesh(clientX, clientY) {
    if (!sceneMesh?.pos?.length) return null;
    const imported = findBoomItem("imported-mesh");
    if (!imported || imported.visible === false || imported.selectable === false || imported.renderable === false) return null;
    const ray = screenRayFromClientPoint(clientX, clientY);
    if (!ray) return null;
    const rect = els.canvas.getBoundingClientRect();
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    const handle = getBoomPickHandle();
    const pos = handle?.meshGeometry?.pos || (sceneMesh.display?.pos?.length ? sceneMesh.display.pos : sceneMesh.pos);
    const passes = handle?.passes || meshRenderPasses(sceneMesh);
    const candidates = handle ? pickFaceCandidates(handle.meshFaces, localX, localY, 2) : null;
    if (handle) {
      boomPickHandleStats.triangleTests += handle.meshFaces.count;
      boomPickHandleStats.candidateTests += candidates.length;
      boomPickHandleStats.faceTestsAvoided += Math.max(0, handle.meshFaces.count - candidates.length);
    }
    let bestT = Infinity;
    if (candidates) {
      for (const candidate of candidates) {
        const hit = intersectIndexedFace(pos, passes, ray, candidate.passIndex, candidate.faceIndex);
        if (hit != null && hit < bestT) bestT = hit;
      }
    } else {
      for (const pass of passes) {
        const model = pass.model || meshModelMatrix({ transform: pass.transform });
        for (let i = 0; i < pos.length; i += 9) {
          const a = transformPointWithModel(model, pos[i], pos[i + 1], pos[i + 2]);
          const b = transformPointWithModel(model, pos[i + 3], pos[i + 4], pos[i + 5]);
          const c = transformPointWithModel(model, pos[i + 6], pos[i + 7], pos[i + 8]);
          const hit = intersectRayTriangle(ray.origin, ray.dir, a, b, c);
          if (hit != null && hit < bestT) bestT = hit;
        }
      }
    }
    return Number.isFinite(bestT) ? { itemId: imported.id, distance: bestT } : null;
  }

  function projectWorldToCanvasPoint(world, proj = lastProj, view = lastView, clipOffset = lastClipOffset) {
    const rect = els.canvas.getBoundingClientRect();
    return projectWorldToCanvasPointFast(world, rect, proj, view, clipOffset);
  }

  function pointSegmentDistance2D(px, py, ax, ay, bx, by) {
    const abx = bx - ax;
    const aby = by - ay;
    const ab2 = abx * abx + aby * aby;
    if (ab2 < 1e-6) return Math.hypot(px - ax, py - ay);
    const t = Math.max(0, Math.min(1, ((px - ax) * abx + (py - ay) * aby) / ab2));
    const qx = ax + abx * t;
    const qy = ay + aby * t;
    return Math.hypot(px - qx, py - qy);
  }

  function boomComponentTransform(passIndex = 0) {
    const passes = meshRenderPasses(sceneMesh);
    const pass = passes[Math.max(0, Math.min(passes.length - 1, passIndex))];
    return pass?.model || meshModelMatrix({ transform: pass?.transform || sceneMesh?.transform });
  }

  function pickBoomVertex(clientX, clientY) {
    if (!boomKasmGraph?.vertices?.length) return null;
    const rect = els.canvas.getBoundingClientRect();
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    const threshold = 14;
    let best = null;
    const handle = getBoomPickHandle();
    const vertices = handle?.vertices || [];
    for (const vertex of vertices) {
      const dist = Math.hypot(localX - vertex.x, localY - vertex.y);
      if (dist > threshold) continue;
      const score = dist + Math.abs(vertex.depth) * 0.02;
      if (!best || score < best.score) {
        const linkCount = boomKasmQueries?.indexes?.vertexToEdges?.[vertex.nodeId]?.length || 0;
        best = { type: "vertex", nodeId: vertex.nodeId, index: vertex.index, passIndex: vertex.passIndex, distance: dist, score, linkCount };
      }
    }
    return best;
  }

  function pickBoomEdge(clientX, clientY) {
    if (!boomKasmGraph?.edges?.length) return null;
    const rect = els.canvas.getBoundingClientRect();
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    const threshold = 12;
    let best = null;
    const handle = getBoomPickHandle();
    const edges = handle?.edges || [];
    for (const edge of edges) {
      const dist = pointSegmentDistance2D(localX, localY, edge.ax, edge.ay, edge.bx, edge.by);
      if (dist > threshold) continue;
      const score = dist + Math.abs(edge.depth) * 0.02;
      if (!best || score < best.score) {
        best = { type: "edge", nodeId: edge.nodeId, index: edge.index, passIndex: edge.passIndex, distance: dist, score };
      }
    }
    return best;
  }

  function pickBoomFace(clientX, clientY) {
    if (!sceneMesh?.pos?.length || !boomKasmGraph?.faces?.length) return null;
    const ray = screenRayFromClientPoint(clientX, clientY);
    if (!ray) return null;
    const rect = els.canvas.getBoundingClientRect();
    const localX = clientX - rect.left;
    const localY = clientY - rect.top;
    const handle = getBoomPickHandle();
    const pos = handle?.componentGeometry?.pos || sceneMesh.pos;
    const passes = handle?.passes || meshRenderPasses(sceneMesh);
    const candidates = handle ? pickFaceCandidates(handle.componentFaces, localX, localY, 2) : null;
    if (handle) {
      boomPickHandleStats.triangleTests += handle.componentFaces.count;
      boomPickHandleStats.candidateTests += candidates.length;
      boomPickHandleStats.faceTestsAvoided += Math.max(0, handle.componentFaces.count - candidates.length);
    }
    let best = null;
    if (candidates) {
      for (const candidate of candidates) {
        const hit = intersectIndexedFace(pos, passes, ray, candidate.passIndex, candidate.faceIndex);
        if (hit == null) continue;
        if (!best || hit < best.distance) {
          const face = boomKasmGraph.faces[candidate.faceIndex];
          if (!face) continue;
          best = { type: "face", nodeId: face.id, index: candidate.faceIndex, passIndex: candidate.passIndex, distance: hit };
        }
      }
    } else {
      for (let passIndex = 0; passIndex < passes.length; passIndex += 1) {
        const model = passes[passIndex].model || boomComponentTransform(passIndex);
        for (let i = 0; i < pos.length; i += 9) {
          const a = transformPointWithModel(model, pos[i], pos[i + 1], pos[i + 2]);
          const b = transformPointWithModel(model, pos[i + 3], pos[i + 4], pos[i + 5]);
          const c = transformPointWithModel(model, pos[i + 6], pos[i + 7], pos[i + 8]);
          const hit = intersectRayTriangle(ray.origin, ray.dir, a, b, c);
          if (hit == null) continue;
          if (!best || hit < best.distance) {
            const index = i / 9;
            const face = boomKasmGraph.faces[index];
            if (!face) continue;
            best = { type: "face", nodeId: face.id, index, passIndex, distance: hit };
          }
        }
      }
    }
    return best;
  }

  function pickBoomComponent(clientX, clientY) {
    if (boomScene.editMode === "vertex") return pickBoomVertex(clientX, clientY);
    if (boomScene.editMode === "edge") return pickBoomEdge(clientX, clientY);
    if (boomScene.editMode === "face") return pickBoomFace(clientX, clientY);
    return pickSceneMesh(clientX, clientY);
  }

  function ensureBoomSelectionOverlay() {
    if (boomSelectionOverlay?.isConnected) return boomSelectionOverlay;
    boomSelectionOverlay = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    boomSelectionOverlay.setAttribute("class", "boom-selection-overlay");
    boomSelectionOverlay.setAttribute("aria-hidden", "true");
    els.view.appendChild(boomSelectionOverlay);
    return boomSelectionOverlay;
  }

  function drawBoomSelectionOverlay() {
    const overlay = ensureBoomSelectionOverlay();
    const rect = els.canvas.getBoundingClientRect();
    overlay.setAttribute("viewBox", `0 0 ${Math.max(1, rect.width)} ${Math.max(1, rect.height)}`);
    overlay.innerHTML = "";
    if (!boomKasmGraph) return;
    const selection = boomScene.componentSelection;
    const region = activeBoomRegionSelection();
    const vertexMap = boomKasmVertexMap();
    const model = boomComponentTransform(selection?.passIndex || 0);
    const makeCircle = (x, y, r, cls) => `<circle class="${cls}" cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="${r}" />`;
    const parts = [];
    if (region?.vertexIds?.length) {
      for (const vertexId of region.vertexIds.slice(0, 96)) {
        const vertex = vertexMap.get(vertexId);
        if (!vertex) continue;
        const world = transformPointWithModel(model, vertex.position[0], vertex.position[1], vertex.position[2]);
        const screen = projectWorldToCanvasPoint(world);
        if (!screen) continue;
        parts.push(makeCircle(screen.x, screen.y, 2.8, "boom-region-dot"));
      }
    }
    if (boomScene.editMode === "object" || !selection) {
      overlay.innerHTML = parts.join("");
      return;
    }
    if (selection.type === "vertex") {
      const vertex = boomKasmGraph.vertices.find((entry) => entry.id === selection.nodeId);
      if (!vertex) return;
      const world = transformPointWithModel(model, vertex.position[0], vertex.position[1], vertex.position[2]);
      const screen = projectWorldToCanvasPoint(world);
      if (!screen) return;
      parts.push(makeCircle(screen.x, screen.y, 10, "boom-selection-ring"));
      parts.push(makeCircle(screen.x, screen.y, 4.5, "boom-selection-dot"));
      overlay.innerHTML = parts.join("");
      return;
    }
    if (selection.type === "edge") {
      const edge = boomKasmGraph.edges.find((entry) => entry.id === selection.nodeId);
      const va = vertexMap.get(edge?.vertices?.[0]);
      const vb = vertexMap.get(edge?.vertices?.[1]);
      if (!edge || !va || !vb) return;
      const a = projectWorldToCanvasPoint(transformPointWithModel(model, va.position[0], va.position[1], va.position[2]));
      const b = projectWorldToCanvasPoint(transformPointWithModel(model, vb.position[0], vb.position[1], vb.position[2]));
      if (!a || !b) return;
      parts.push(`<line class="boom-selection-edge" x1="${a.x.toFixed(2)}" y1="${a.y.toFixed(2)}" x2="${b.x.toFixed(2)}" y2="${b.y.toFixed(2)}" />`);
      parts.push(makeCircle(a.x, a.y, 4, "boom-selection-dot"));
      parts.push(makeCircle(b.x, b.y, 4, "boom-selection-dot"));
      overlay.innerHTML = parts.join("");
      return;
    }
    if (selection.type === "face") {
      const face = boomKasmGraph.faces.find((entry) => entry.id === selection.nodeId);
      if (!face) return;
      const points = face.vertices
        .map((id) => vertexMap.get(id))
        .filter(Boolean)
        .map((vertex) => projectWorldToCanvasPoint(transformPointWithModel(model, vertex.position[0], vertex.position[1], vertex.position[2])))
        .filter(Boolean);
      if (points.length < 3) return;
      const poly = points.map((point) => `${point.x.toFixed(2)},${point.y.toFixed(2)}`).join(" ");
      parts.push(`<polygon class="boom-selection-face" points="${poly}" />`);
      overlay.innerHTML = parts.join("");
    }
  }

  function canvasCenterClipOffset() {
    if (!els.stage) return [0, 0];
    const canvasRect = els.canvas.getBoundingClientRect();
    const stageRect = els.stage.getBoundingClientRect();
    if (!canvasRect.width || !canvasRect.height || !stageRect.width || !stageRect.height) {
      return [0, 0];
    }
    const canvasCx = canvasRect.left + canvasRect.width * 0.5;
    const canvasCy = canvasRect.top + canvasRect.height * 0.5;
    const stageCx = stageRect.left + stageRect.width * 0.5;
    const stageCy = stageRect.top + stageRect.height * 0.5;
    const offsetX = (stageCx - canvasCx) / (canvasRect.width * 0.5);
    const offsetY = (canvasCy - stageCy) / (canvasRect.height * 0.5);
    return [offsetX, offsetY];
  }

  function syncBoomViewportAnchors() {
    if (!els.stage) return;
    const stageRect = els.stage.getBoundingClientRect();
    const viewportRightInset = Math.max(0, window.innerWidth - stageRect.right);
    if (boomViewportHud?.isConnected) {
      const chatEl = document.querySelector(".canvas-chat");
      const chatRect = chatEl && chatEl.offsetParent !== null ? chatEl.getBoundingClientRect() : null;
      const hudBottomInset = chatRect
        ? Math.max(window.innerHeight - stageRect.bottom + 18, window.innerHeight - chatRect.top + 14)
        : (window.innerHeight - stageRect.bottom + 18);
      boomViewportHud.style.top = "auto";
      boomViewportHud.style.left = `${Math.max(18, stageRect.left + 18)}px`;
      boomViewportHud.style.bottom = `${Math.max(18, hudBottomInset)}px`;
      boomViewportHud.style.maxWidth = `${Math.max(240, stageRect.width - 36)}px`;
    }
    if (els.gizmo) {
      els.gizmo.style.right = `${18 + viewportRightInset}px`;
    }
  }

  function render(ts) {
    raf = 0;
    if (gpuState !== "active" || !gl) return;
    const renderStarted = boomNowMs();
    const continuous = boomRenderContinuousActive(renderStarted);
    if (!boomRenderDirty && !continuous) {
      boomRenderStats.idleSkips += 1;
      return;
    }
    const frameReason = boomRenderReason || (continuous ? "continuous" : "dirty");
    boomRenderDirty = false;
    boomRenderReason = "";
    boomRenderStats.frames += 1;
    boomRenderStats.lastReason = frameReason;
    boomRenderStats.lastFrameAtMs = Number(renderStarted.toFixed(3));
    if (continuous) boomRenderStats.continuousFrames += 1;
    else boomRenderStats.dirtyFrames += 1;

    // FPS
    if (fpsTimer === 0) fpsTimer = ts;
    fpsFrames++;
    if (ts - fpsTimer >= 500) {
      lastFps = Math.round((fpsFrames * 1000) / (ts - fpsTimer));
      if (els.statFps) els.statFps.textContent = String(lastFps);
      fpsFrames = 0;
      fpsTimer = ts;
    }

    resize();
    syncBoomViewportAnchors();
    const w = els.canvas.width, h = els.canvas.height;
    applyBoomAnimationFrame(ts);

    gl.clearColor(0.0, 0.0, 0.0, 0.0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

    const proj = M4.perspective(46 * Math.PI / 180, w / h, 0.1, 2400);
    const eye = cameraEye();
    const [clipOffsetX, clipOffsetY] = canvasCenterClipOffset();
    // up = +Z (Blender-style)
    const view = M4.lookAt(eye, camera.target, [0, 0, 1]);

    // SDF raymarch (INGEN COMPUTE §19.4) — fullscreen triangle, depth
    // write enabled, fragment discards on miss so the grid/gizmo/mesh
    // stay visible. Sharing `eye`/`proj`/`view` with the other passes
    // keeps the SDF surface registered with the existing camera, the
    // selection picker and the slicer preview without any extra math.
    if (sdfProg) {
      const fx = camera.target[0] - eye[0];
      const fy = camera.target[1] - eye[1];
      const fz = camera.target[2] - eye[2];
      const flen = Math.hypot(fx, fy, fz) || 1;
      const fwd = [fx / flen, fy / flen, fz / flen];
      // worldUp = [0, 0, 1] (Z-up). right = fwd × worldUp.
      const rx = fwd[1] * 1 - fwd[2] * 0;
      const ry = fwd[2] * 0 - fwd[0] * 1;
      const rz = fwd[0] * 0 - fwd[1] * 0;
      const rlen = Math.hypot(rx, ry, rz) || 1;
      const right = [rx / rlen, ry / rlen, rz / rlen];
      // up = right × fwd (orthogonalised).
      const up = [
        right[1] * fwd[2] - right[2] * fwd[1],
        right[2] * fwd[0] - right[0] * fwd[2],
        right[0] * fwd[1] - right[1] * fwd[0],
      ];
      const viewProj = M4.multiply(proj, view);
      gl.useProgram(sdfProg);
      gl.uniform2f(uSdfResolution, w, h);
      gl.uniform3fv(uSdfCameraPos, new Float32Array(eye));
      gl.uniform3fv(uSdfCameraFwd, new Float32Array(fwd));
      gl.uniform3fv(uSdfCameraRight, new Float32Array(right));
      gl.uniform3fv(uSdfCameraUp, new Float32Array(up));
      gl.uniform1f(uSdfTanHalfFovY, Math.tan((46 * Math.PI / 180) * 0.5));
      gl.uniformMatrix4fv(uSdfViewProj, false, viewProj);
      gl.bindVertexArray(null);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    }

    // Grid (lines)
    gl.useProgram(lineProg);
    gl.uniformMatrix4fv(uLineProj, false, proj);
    gl.uniformMatrix4fv(uLineView, false, view);
    gl.uniform2f(uLineClipOffset, clipOffsetX, clipOffsetY);
    gl.uniform1f(uLineFadeNear, Math.max(34.0, camera.distance * 1.8));
    gl.uniform1f(uLineFadeFar,  Math.max(220.0, camera.distance * 13.0));
    gl.depthMask(false);
    gl.bindVertexArray(gridVAO);
    gl.drawArrays(gl.LINES, 0, gridCount);
    gl.depthMask(true);

    const drawableMesh = sceneMesh?.display?.vao ? sceneMesh.display : sceneMesh;
    if (drawableMesh?.vao && sceneMesh.visible !== false) {
      gl.useProgram(meshProg);
      gl.uniformMatrix4fv(uMeshProj, false, proj);
      gl.uniformMatrix4fv(uMeshView, false, view);
      gl.uniform2f(uMeshClipOffset, clipOffsetX, clipOffsetY);
      gl.bindVertexArray(drawableMesh.vao);
      const passes = meshRenderPasses(sceneMesh);
      for (const pass of passes) {
        gl.uniformMatrix4fv(uMeshModel, false, pass.model || meshModelMatrix({ transform: pass.transform }));
        const baseColor = pass.color || sceneMesh.color || new Float32Array([0.84, 0.85, 0.90]);
        const previewColor = slicerPreviewEnabled()
          ? new Float32Array([baseColor[0] * 0.48, baseColor[1] * 0.46, baseColor[2] * 0.44])
          : baseColor;
        gl.uniform3fv(uMeshColor, previewColor);
        gl.drawArrays(gl.TRIANGLES, 0, drawableMesh.count);
      }
    }

    if (slicerPreview?.vao && slicerPreviewEnabled()) {
      gl.useProgram(lineProg);
      gl.uniformMatrix4fv(uLineProj, false, proj);
      gl.uniformMatrix4fv(uLineView, false, view);
      gl.uniform2f(uLineClipOffset, clipOffsetX, clipOffsetY);
      gl.uniform1f(uLineFadeNear, Math.max(40.0, camera.distance * 1.2));
      gl.uniform1f(uLineFadeFar, Math.max(240.0, camera.distance * 9.0));
      gl.disable(gl.DEPTH_TEST);
      gl.depthMask(false);
      gl.bindVertexArray(slicerPreview.vao);
      gl.drawArrays(gl.LINES, 0, slicerPreview.count);
      gl.depthMask(true);
      gl.enable(gl.DEPTH_TEST);
    }

    gl.bindVertexArray(null);

    lastProj = proj;
    lastView = view;
    lastClipOffset = [clipOffsetX, clipOffsetY];
    drawGizmoAccurate(proj, view, lastClipOffset);
    drawBoomSelectionOverlay();
    if (boomRenderContinuousActive() && !raf) {
      raf = requestAnimationFrame(render);
    }
  }

  function drawGizmoAccurate(_proj = lastProj, view = lastView) {
    if (!els.gizmo || !view) return;
    const projectAxis = (vx, vy, vz) => {
      const viewVec = M4.transformVec4(view, vx, vy, vz, 0);
      return {
        x: viewVec[0],
        y: viewVec[1],
        depth: viewVec[2],
      };
    };
    const axes = [
      { v: [ 1, 0, 0], col: AXIS_HEX.x,    positive: true },
      { v: [-1, 0, 0], col: AXIS_HEX.xNeg, positive: false },
      { v: [ 0, 1, 0], col: AXIS_HEX.y,    positive: true },
      { v: [ 0,-1, 0], col: AXIS_HEX.yNeg, positive: false },
      { v: [ 0, 0, 1], col: AXIS_HEX.z,    positive: true },
      { v: [ 0, 0,-1], col: AXIS_HEX.zNeg, positive: false },
    ];
    const nodes = axes.map((axis) => {
      const projected = projectAxis(axis.v[0], axis.v[1], axis.v[2]);
      const len = Math.hypot(projected.x, projected.y) || 1;
      const radius = axis.positive ? 21 : 17.5;
      return {
        ...axis,
        px: (projected.x / len) * radius,
        py: (-projected.y / len) * radius,
        depth: projected.depth,
      };
    }).sort((a, b) => a.depth - b.depth);
    const parts = ['<circle cx="0" cy="0" r="2.4" fill="rgba(238,242,248,0.28)" />'];
    for (const axis of nodes) {
      const isNeg = !axis.positive;
      const lineW = isNeg ? 1.35 : 1.9;
      const dotR = isNeg ? 4.6 : 5.8;
      parts.push(`<line x1="0" y1="0" x2="${axis.px.toFixed(1)}" y2="${axis.py.toFixed(1)}" stroke="${axis.col}" stroke-opacity="${isNeg ? "0.54" : "0.92"}" stroke-width="${lineW}" stroke-linecap="round"/>`);
      parts.push(`<circle cx="${axis.px.toFixed(1)}" cy="${axis.py.toFixed(1)}" r="${dotR}" fill="${axis.col}" fill-opacity="${isNeg ? "0.56" : "0.94"}" />`);
    }
    els.gizmo.innerHTML = parts.join("");
  }

  function resetToDefaultNewSession() {
    try {
      if (typeof window.startAlphaNewSession === "function") {
        window.startAlphaNewSession();
      }
    } catch (err) {
      console.warn("[banger] unable to reset to new session:", err);
    }
  }

  function setLayoutActive(active) {
    els.content?.classList.toggle("is-banger-layout", active);
    document.body?.classList.toggle("banger-fullscreen-mode", active);
  }

  // ---------- input ----------
  function attachInput() {
    let dragging = false, lastX = 0, lastY = 0, mode = null;
    let downButton = 0;
    let downX = 0, downY = 0;
    let dragDistance = 0;
    els.canvas.addEventListener("contextmenu", (e) => {
      e.preventDefault();
    });
    els.canvas.addEventListener("mousedown", (e) => {
      dragging = true; lastX = e.clientX; lastY = e.clientY;
      downButton = e.button;
      downX = e.clientX;
      downY = e.clientY;
      dragDistance = 0;
      mode = (e.button === 1 || e.button === 2 || e.shiftKey) ? "pan" : "orbit";
      e.preventDefault();
      requestBoomRender("camera-input-start", 180);
    });
    window.addEventListener("mouseup", (e) => {
      const clickLike = dragging
        && downButton === 0
        && mode === "orbit"
        && dragDistance < 6;
      dragging = false;
      mode = null;
      if (!els.view.hidden) requestBoomRender(clickLike ? "component-pick" : "camera-input-end", 120);
      if (!clickLike || els.view.hidden) return;
      const hit = pickBoomComponent(e.clientX, e.clientY);
      if (!hit) {
        if (boomScene.editMode !== "object") {
          clearBoomComponentSelection();
          clearBoomRegionSelection();
          renderBoomSidebar();
          renderBoomViewportHud();
        }
        return;
      }
      if (boomScene.editMode === "object") {
        if (!hit?.itemId) return;
        clearBoomComponentSelection();
        clearBoomRegionSelection();
        boomScene.activeId = hit.itemId;
      } else {
        boomScene.activeId = "imported-mesh";
        setBoomComponentSelection(hit);
        clearBoomRegionSelection();
      }
      if (boomScene.propertyTab === "scene") boomScene.propertyTab = "object";
      renderBoomSidebar();
      renderBoomViewportHud();
      requestBoomRender("component-selection");
    });
    window.addEventListener("mousemove", (e) => {
      if (!dragging) return;
      const dx = e.clientX - lastX, dy = e.clientY - lastY;
      lastX = e.clientX; lastY = e.clientY;
      dragDistance = Math.max(dragDistance, Math.hypot(e.clientX - downX, e.clientY - downY));
      if (mode === "orbit") {
        camera.azimuth   -= dx * 0.008;
        camera.elevation += dy * 0.008;
        const lim = Math.PI / 2 - 0.05;
        if (camera.elevation >  lim) camera.elevation =  lim;
        if (camera.elevation < -lim) camera.elevation = -lim;
      } else if (mode === "pan") {
        const f = camera.distance * 0.0022;
        const { right, up } = cameraPanBasis();
        camera.target[0] += (-dx * right[0] + dy * up[0]) * f;
        camera.target[1] += (-dx * right[1] + dy * up[1]) * f;
        camera.target[2] += (-dx * right[2] + dy * up[2]) * f;
      }
      requestBoomRender(mode === "pan" ? "camera-pan" : "camera-orbit", 180);
    });
    els.canvas.addEventListener("wheel", (e) => {
      e.preventDefault();
      const k = Math.exp(e.deltaY * 0.001);
      camera.distance = Math.max(1.5, Math.min(80, camera.distance * k));
      requestBoomRender("camera-zoom", 180);
    }, { passive: false });
    // Numpad-style focus reset
    window.addEventListener("keydown", (e) => {
      if (els.view.hidden) return;
      if (e.target && /input|textarea/i.test(e.target.tagName)) return;
      if (e.key === "Home" || e.key === ".") {
        camera.target = [0,0,0]; camera.distance = 22;
        camera.azimuth = -Math.PI/4; camera.elevation = Math.PI/5;
        requestBoomRender("camera-reset");
      }
      if (e.key === "Escape") closeOverlay();
    });
  }

  function attachBoomFileDrop() {
    const stageFiles = async (fileListLike) => {
      const files = Array.from(fileListLike || []);
      if (!files.length) return;
      try {
        window.dispatchEvent(new CustomEvent("forge:banger-stage-files", { detail: { files } }));
      } catch (_) {}
      await previewBoom3dFiles(files);
    };
    const hasFiles = (event) => Array.from(event?.dataTransfer?.types || []).includes("Files");
    const onDragEnter = (event) => {
      if (!isViewVisible() || !hasFiles(event)) return;
      event.preventDefault();
      boomDragDepth += 1;
      setBoomDropActive(true);
    };
    const onDragOver = (event) => {
      if (!isViewVisible() || !hasFiles(event)) return;
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
      setBoomDropActive(true);
    };
    const onDragLeave = (event) => {
      if (!isViewVisible() || !hasFiles(event)) return;
      event.preventDefault();
      boomDragDepth = Math.max(0, boomDragDepth - 1);
      if (boomDragDepth === 0) setBoomDropActive(false);
    };
    const onDrop = async (event) => {
      if (!isViewVisible() || !hasFiles(event)) return;
      event.preventDefault();
      boomDragDepth = 0;
      setBoomDropActive(false);
      await stageFiles(event.dataTransfer?.files || []);
    };
    els.view.addEventListener("dragenter", onDragEnter);
    els.view.addEventListener("dragover", onDragOver);
    els.view.addEventListener("dragleave", onDragLeave);
    els.view.addEventListener("drop", onDrop);
  }

  // ---------- overlay control + lifecycle state machine ----------
  function setGpuStatus(label, tone) {
    if (!gpuStatusEl) return;
    gpuStatusEl.textContent = label;
    gpuStatusEl.dataset.tone = tone;
  }

  function ensureGpuStatusBadge() {
    if (gpuStatusEl) return;
    const stats = els.view.querySelector(".banger-stats");
    if (!stats) return;
    const sep = document.createElement("span");
    sep.className = "banger-stat-sep";
    sep.setAttribute("aria-hidden", "true");
    sep.textContent = "·";
    const pill = document.createElement("span");
    pill.className = "banger-gpu-pill";
    pill.dataset.tone = "active";
    pill.innerHTML = '<span class="banger-gpu-dot" aria-hidden="true"></span><span class="banger-gpu-label">GPU active</span>';
    stats.appendChild(sep);
    stats.appendChild(pill);
    gpuStatusEl = pill.querySelector(".banger-gpu-label");
    setGpuStatus("GPU active", "active");
  }

  function activate() {
    if (gpuState === "active") return;
    if (!gl) {
      if (!initGL()) {
        console.error("[banger] GL init failed");
        gpuState = "idle";
        return;
      }
    }
    if (!inputAttached) {
      attachInput();
      inputAttached = true;
    }
    if (!dropAttached) {
      attachBoomFileDrop();
      dropAttached = true;
    }
    fpsTimer = 0; fpsFrames = 0;
    gpuState = "active";
    setGpuStatus("GPU active", "active");
    resize();
    requestBoomRender("activate");
    requestAnimationFrame(() => {
      resize();
      requestBoomRender("activate-resize");
    });

    // Claim native BangerEngine (P1a). Fire-and-forget; the HUD reflects the
    // returned backend/adapter info as soon as it arrives.
    backendBusy = backendInvoke("banger_engine_start").then((status) => {
      if (gpuState === "active") applyBackendStatus(status);
    });
    backendInvoke("banger_runtime_status").then((status) => {
      if (gpuState === "active") applyRuntimeStatus(status);
    });
  }

  function suspend() {
    if (gpuState !== "active") return;
    gpuState = "suspended";
    stopRenderLoop();
    if (els.statFps) els.statFps.textContent = "—";
    setGpuStatus("GPU paused", "paused");
    backendBusy = backendInvoke("banger_engine_stop");
  }

  function shutdown() {
    if (gpuState === "idle") return;
    gpuState = "shutdown";
    releaseGL();
    if (els.statFps) els.statFps.textContent = "—";
    backendBusy = backendInvoke("banger_engine_stop");
    gpuState = "idle";
  }

  function isViewVisible() {
    return els.view && !els.view.hidden;
  }

  function setBoomActive(active) {
    els.boomBtn.classList.toggle("is-active", active);
    els.boomBtn.setAttribute("aria-pressed", active ? "true" : "false");
    if (!active) requestAnimationFrame(() => els.boomBtn?.blur?.());
    els.boomBtn.title = active ? "Close Banger" : "Banger — 3D matrix";
  }

  function openOverlay() {
    if (window.__forgeRealEstateModeActive) return;
    if (isViewVisible()) return; // already open
    try {
      if (window.__forgeTradingChatBridge?.isActive?.()) window.__forgeCloseTrading?.();
    } catch (_) {}
    try {
      if (window.__forgeWebExplorerIsActive?.()) window.__forgeCloseWebExplorer?.();
    } catch (_) {}
    resetToDefaultNewSession();
    els.view.hidden = false;
    els.view.setAttribute("aria-hidden", "false");
    if (els.stage) els.stage.classList.add("is-banger-mode");
    setLayoutActive(true);
    ensureBoomSidebar();
    ensureBoomViewportHud();
    ensureBoomSelectionOverlay();
    ensureBoomDropOverlay();
    setBoomActive(true);
    if (typeof window !== "undefined") {
      window.__forgeBoomIsActive = true;
      if (bangerController) {
        bangerController.publishActive(true);
      }
      window.__forgeBoomConsoleContext = buildBoomConsoleContext;
      window.__forgeBoomExecuteTool = executeBoomTool;
    }
    ensureGpuStatusBadge();
    requestAnimationFrame(() => syncBoomInteractionContract());
    if (boomScene.workspaceMode === "slicer") {
      void refreshBoomPrinterDiscovery();
    }
    if (document.visibilityState === "visible" && document.hasFocus()) {
      activate();
    } else {
      gpuState = "suspended";
      setGpuStatus("GPU paused", "paused");
    }
  }

  function closeOverlay() {
    if (!isViewVisible() && !window.__forgeBoomIsActive) return;
    shutdown();
    setBoomDropActive(false);
    releaseSceneMesh();
    if (els.stage) els.stage.classList.remove("is-banger-mode");
    els.view.hidden = true;
    els.view.setAttribute("aria-hidden", "true");
    setLayoutActive(false);
    setBoomActive(false);
    boomUiContract = null;
    if (typeof window !== "undefined") {
      window.__forgeBoomIsActive = false;
      if (bangerController) {
        bangerController.publishActive(false);
      }
      window.__forgeBoomConsoleContext = () => ({ active: false });
      window.__forgeBoomExecuteTool = () => ({ ok: false, tool: "boom.unavailable", detail: { error: "inactive" }, context: { active: false } });
      window.__forgeBoomUiContract = null;
      window.__forgeBoomCommandCatalog = [];
      window.__forgeBoomResolveControlHash = () => null;
    }
    resetToDefaultNewSession();
  }

  function toggleBanger() {
    if (bangerController) {
      bangerController.toggle();
      return;
    }
    if (window.__forgeRealEstateModeActive && !isViewVisible()) return;
    if (isViewVisible()) closeOverlay();
    else openOverlay();
  }

  // Lifecycle triggers — only meaningful while the view is open.
  function onVisibilityChange() {
    if (!isViewVisible()) return;
    if (document.visibilityState === "hidden") {
      suspend();
    } else if (document.hasFocus()) {
      activate();
    }
  }
  function onWindowBlur() {
    if (!isViewVisible()) return;
    suspend();
  }
  function onWindowFocus() {
    if (!isViewVisible()) return;
    if (document.visibilityState === "visible") activate();
  }
  document.addEventListener("visibilitychange", onVisibilityChange);
  window.addEventListener("blur", onWindowBlur);
  window.addEventListener("focus", onWindowFocus);
  window.addEventListener("beforeunload", shutdown);
  els.canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    stopRenderLoop();
    gl = null;
    cubeBuffers = [];
    gridBuffers = [];
    cubeVAO = null;
    gridVAO = null;
    meshProg = null;
    lineProg = null;
    sdfProg = null;
    cubeCount = 0;
    gridCount = 0;
    gpuState = isViewVisible() ? "suspended" : "idle";
    setGpuStatus("GPU paused", "paused");
  });
  els.canvas.addEventListener("webglcontextrestored", () => {
    if (!isViewVisible()) return;
    activate();
  });
  if (typeof window !== "undefined") {
    bangerController = window.ForgeBangerController?.create?.({
      runtime: window.ForgeShellRuntime,
      button: els.boomBtn,
      isVisible: isViewVisible,
      isBlocked: () => !!window.__forgeRealEstateModeActive,
      open: openOverlay,
      close: closeOverlay,
      syncButton: setBoomActive,
    }) || null;
    window.__forgeBoomIsActive = false;
    window.__forgeBoomConsoleContext = () => ({ active: false });
    window.__forgeBoomExecuteTool = () => ({ ok: false, tool: "boom.unavailable", detail: { error: "inactive" }, context: { active: false } });
    window.__forgeBoomPreview3dFiles = previewBoom3dFiles;
    window.__forgeOpenBoom = () => (bangerController ? bangerController.open() : openOverlay());
    window.__forgeCloseBoom = () => (bangerController ? bangerController.close() : closeOverlay());
    exposeBoomAuditState();
  }

  // ---------- wire up ----------
  // Direct listener (works in plain browsers); plus capture-phase delegation
  // because Tauri's titlebar drag-region absorbs bubble-phase clicks
  // from siblings — see the same pattern around #forgeSearchBtn in app.js.

  // ResizeObserver to follow stage size changes (panels collapsing, window resize).
  if ("ResizeObserver" in window) {
    const ro = new ResizeObserver(() => {
      resize();
      requestBoomRender("resize");
    });
    ro.observe(els.canvas);
  } else {
    window.addEventListener("resize", () => {
      resize();
      requestBoomRender("resize");
    });
  }
})();
