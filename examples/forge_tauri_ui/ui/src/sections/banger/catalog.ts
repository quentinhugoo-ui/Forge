// Static WebGL/math catalog for the Banger surface.
// Source of truth lives in TypeScript during the JS cutover.

type Vec3 = readonly [number, number, number];
type Vec4 = [number, number, number, number];

function at(values: Float32Array, index: number): number {
  return values[index] ?? 0;
}

// ---------- mat4 helpers (column-major, Float32Array) ----------
export const M4 = {
  identity(): Float32Array { const m=new Float32Array(16); m[0]=m[5]=m[10]=m[15]=1; return m; },
  perspective(fovY: number, aspect: number, near: number, far: number): Float32Array {
    const f = 1 / Math.tan(fovY / 2);
    const nf = 1 / (near - far);
    const m = new Float32Array(16);
    m[0]=f/aspect; m[5]=f; m[10]=(far+near)*nf; m[11]=-1; m[14]=2*far*near*nf;
    return m;
  },
  lookAt(eye: Vec3, target: Vec3, up: Vec3): Float32Array {
    const z0=eye[0]-target[0], z1=eye[1]-target[1], z2=eye[2]-target[2];
    let zl = Math.hypot(z0,z1,z2); zl = zl===0?1:1/zl;
    const zx=z0*zl, zy=z1*zl, zz=z2*zl;
    let xx=up[1]*zz-up[2]*zy, xy=up[2]*zx-up[0]*zz, xz=up[0]*zy-up[1]*zx;
    let xl = Math.hypot(xx,xy,xz); xl = xl===0?1:1/xl;
    xx*=xl; xy*=xl; xz*=xl;
    const yx=zy*xz-zz*xy, yy=zz*xx-zx*xz, yz=zx*xy-zy*xx;
    const m = new Float32Array(16);
    m[0]=xx; m[1]=yx; m[2]=zx; m[3]=0;
    m[4]=xy; m[5]=yy; m[6]=zy; m[7]=0;
    m[8]=xz; m[9]=yz; m[10]=zz; m[11]=0;
    m[12]=-(xx*eye[0]+xy*eye[1]+xz*eye[2]);
    m[13]=-(yx*eye[0]+yy*eye[1]+yz*eye[2]);
    m[14]=-(zx*eye[0]+zy*eye[1]+zz*eye[2]);
    m[15]=1;
    return m;
  },
  multiply(a: Float32Array, b: Float32Array): Float32Array {
    const out = new Float32Array(16);
    for (let i=0;i<4;i++) for (let j=0;j<4;j++) {
      out[i*4+j] = at(a,0*4+j)*at(b,i*4+0)+at(a,1*4+j)*at(b,i*4+1)+at(a,2*4+j)*at(b,i*4+2)+at(a,3*4+j)*at(b,i*4+3);
    }
    return out;
  },
  transformVec4(m: Float32Array, x: number, y: number, z: number, w = 1): Vec4 {
    return [
      at(m,0) * x + at(m,4) * y + at(m,8)  * z + at(m,12) * w,
      at(m,1) * x + at(m,5) * y + at(m,9)  * z + at(m,13) * w,
      at(m,2) * x + at(m,6) * y + at(m,10) * z + at(m,14) * w,
      at(m,3) * x + at(m,7) * y + at(m,11) * z + at(m,15) * w,
    ];
  },
};

export const AXIS_RGB = {
  x: [0.96, 0.43, 0.56],
  y: [0.23, 0.84, 0.68],
  z: [0.47, 0.58, 0.98],
};
export const AXIS_HEX = {
  x: "#f56d90",
  xNeg: "#9d4761",
  y: "#3bd6ad",
  yNeg: "#24836c",
  z: "#7894fa",
  zNeg: "#495caa",
};

export function makeCube(): { pos: Float32Array; nrm: Float32Array; count: number } {
  const faces: Array<[Vec3, Vec3, Vec3, Vec3, Vec3]> = [
    [[1, -1, -1], [1, 1, -1], [1, 1, 1], [1, -1, 1], [1, 0, 0]],
    [[-1, -1, 1], [-1, 1, 1], [-1, 1, -1], [-1, -1, -1], [-1, 0, 0]],
    [[-1, 1, -1], [-1, 1, 1], [1, 1, 1], [1, 1, -1], [0, 1, 0]],
    [[-1, -1, 1], [-1, -1, -1], [1, -1, -1], [1, -1, 1], [0, -1, 0]],
    [[-1, -1, 1], [1, -1, 1], [1, 1, 1], [-1, 1, 1], [0, 0, 1]],
    [[1, -1, -1], [-1, -1, -1], [-1, 1, -1], [1, 1, -1], [0, 0, -1]],
  ];
  const pos: number[] = [];
  const nrm: number[] = [];
  for (const face of faces) {
    const [a, b, c, d, n] = face;
    for (const v of [a, b, c, a, c, d]) {
      pos.push(...v);
      nrm.push(...n);
    }
  }
  return { pos: new Float32Array(pos), nrm: new Float32Array(nrm), count: pos.length / 3 };
}

export function makeGrid(half = 320, step = 1): { pos: Float32Array; col: Float32Array; count: number } {
  const pos: number[] = [];
  const col: number[] = [];
  const minor: Vec3 = [0.18, 0.185, 0.20];
  const major: Vec3 = [0.29, 0.295, 0.315];
  for (let i = -half; i <= half; i += step) {
    const c = i === 0 ? null : (i % 10 === 0 ? major : minor);
    if (!c) continue;
    pos.push(-half, i, 0, half, i, 0);
    col.push(...c, ...c);
    pos.push(i, -half, 0, i, half, 0);
    col.push(...c, ...c);
  }
  pos.push(-half, 0, 0, half, 0, 0);
  col.push(...AXIS_RGB.x, ...AXIS_RGB.x);
  pos.push(0, -half, 0, 0, half, 0);
  col.push(...AXIS_RGB.y, ...AXIS_RGB.y);
  return { pos: new Float32Array(pos), col: new Float32Array(col), count: pos.length / 3 };
}

export const VS_MESH = `#version 300 es
  precision highp float;
  in vec3 aPos;
  in vec3 aNormal;
  uniform mat4 uModel;
  uniform mat4 uProj;
  uniform mat4 uView;
  uniform vec2 uClipOffset;
  out vec3 vNormal;
  out vec3 vWorld;
  void main() {
    vec4 worldPos = uModel * vec4(aPos, 1.0);
    vNormal = normalize(mat3(uModel) * aNormal);
    vWorld  = worldPos.xyz;
    gl_Position = uProj * uView * worldPos;
    gl_Position.xy += uClipOffset * gl_Position.w;
  }
`;
export const FS_MESH = `#version 300 es
  precision highp float;
  in vec3 vNormal;
  in vec3 vWorld;
  out vec4 fragColor;
  uniform vec3 uColor;
  void main() {
    vec3 N = normalize(vNormal);
    vec3 L = normalize(vec3(0.6, 0.9, 0.7));
    float ndl = max(dot(N, L), 0.0);
    vec3 ambient = vec3(0.18, 0.18, 0.22);
    vec3 diffuse = uColor * (0.55 + 0.55 * ndl);
    vec3 col = ambient + diffuse;
    // soft rim
    float rim = pow(1.0 - max(dot(N, vec3(0.0,0.0,1.0)), 0.0), 2.0);
    col += rim * 0.08 * vec3(1.0, 0.7, 0.4);
    fragColor = vec4(col, 1.0);
  }
`;
export const VS_LINE = `#version 300 es
  precision highp float;
  in vec3 aPos;
  in vec3 aColor;
  uniform mat4 uProj;
  uniform mat4 uView;
  uniform vec2 uClipOffset;
  out vec3 vColor;
  out vec3 vViewPos;
  void main() {
    vColor = aColor;
    vec4 viewPos = uView * vec4(aPos, 1.0);
    vViewPos = viewPos.xyz;
    gl_Position = uProj * viewPos;
    gl_Position.xy += uClipOffset * gl_Position.w;
  }
`;
export const FS_LINE = `#version 300 es
  precision highp float;
  in vec3 vColor;
  in vec3 vViewPos;
  out vec4 fragColor;
  uniform float uFadeNear;
  uniform float uFadeFar;
  void main() {
    float dist = length(vViewPos);
    float fade = 1.0 - smoothstep(uFadeNear, uFadeFar, dist);
    fade = clamp(fade, 0.0, 1.0);
    fragColor = vec4(vColor * fade, fade);
  }
`;


// ---------- SDF raymarch (INGEN COMPUTE §19.4) ----------------------------
//
// Fragment shader raymarches a hardcoded signed-distance scene that the
// surface render loop draws as a single fullscreen triangle BEFORE the
// grid. Discards on miss so the grid + gizmo + imported mesh stay
// visible. Writes gl_FragDepth so depth interactions are correct.
//
// The SDF math mirrors `src/sdf.rs::SmoothUnion` (log-sum-exp softmin)
// — same primitives, same combinator, so a future KASM → SDF tree
// lowering can swap the hardcoded `scene()` body for a buffer-driven
// interpreter without touching the camera or the WebGL pipeline.

export const VS_SDF = `#version 300 es
  precision highp float;
  void main() {
    // Three-vertex fullscreen triangle (no vertex buffer needed).
    float x = float((gl_VertexID << 1) & 2) * 2.0 - 1.0;
    float y = float(gl_VertexID & 2) * 2.0 - 1.0;
    gl_Position = vec4(x, y, 0.0, 1.0);
  }
`;

// SDF scene as data (INGEN COMPUTE §19.3) :
//   uOps[i*2]   = vec4(op_code, p0, p1, p2)
//   uOps[i*2+1] = vec4(p3, p4, p5, k)
// Op codes mirror scenes.ts (OP_SPHERE=0, BOX=1, UNION=10, INTERSECT=11,
// DIFF=12, SMIN=13). The fragment walks `uOpCount` ops postfix-style
// through a tiny stack machine, so swapping scenes is a single
// uniform4fv upload — no shader recompile, no Rust round-trip.
export const FS_SDF = `#version 300 es
  precision highp float;
  uniform vec2  uResolution;
  uniform vec3  uCameraPos;
  uniform vec3  uCameraFwd;
  uniform vec3  uCameraRight;
  uniform vec3  uCameraUp;
  uniform float uTanHalfFovY;
  uniform mat4  uViewProj;
  uniform vec4  uOps[128];
  uniform int   uOpCount;
  // INGEN §13 verifier : 0 = normal render, 1 = Lipschitz heatmap
  // (green where |grad d| ≈ 1, red where it drifts — smin / domain ops
  // are expected violators, a proper sphere SDF stays solid green).
  uniform int   uDebugMode;
  // INGEN §19.5 compact splatting : exponential halo on raymarch misses
  // that come close to the surface. Acts as a proxy gaussien attached
  // to the SDF without a second render pass.
  uniform int   uGlow;
  // INGEN §11 mesh→SDF : 3D texture of signed distances voxelised from
  // an external mesh (banger_voxelize_mesh in Rust). Sampled via opcode
  // 20 (OP_SAMPLED_SDF). When uMeshLoaded=0 the sample returns a large
  // positive distance so it never affects the scene.
  uniform sampler3D uMeshSdf;
  uniform vec3      uMeshMin;
  uniform vec3      uMeshMax;
  uniform int       uMeshLoaded;
  // INGEN §19.5 real Gaussian Splatting. Each splat = 2 vec4 :
  //   slot[0] = (pos.xyz, world-scale σ)
  //   slot[1] = (color.rgb, opacity α)
  // Sampled from the SDF surface in scenes.ts::bakeGaussiansOnSurface.
  // Evaluated per-pixel inside this fragment : projection clip-space,
  // 2D screen-space falloff exp(-r²/2σ²), additive blend — no second
  // render pass, no rasterized billboards.
  uniform vec4  uGaussians[128];
  uniform int   uGaussianCount;
  // §world-building : sky background + distance fog (0 = old transparent
  // mode, 1 = atmospheric horizon). Toggle from the agent via
  // __forgeBangerSetSky / __forgeBangerSetFog.
  uniform int   uSky;
  uniform int   uFog;
  out vec4 fragColor;

  float sd_sphere(vec3 p, float r) { return length(p) - r; }

  float sd_box(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    return length(max(q, vec3(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
  }

  // Z-axis aligned torus : R = ring radius, r = tube radius.
  float sd_torus(vec3 p, float R, float r) {
    vec2 q = vec2(length(p.xy) - R, p.z);
    return length(q) - r;
  }

  // Capsule between endpoints a and b, radius r.
  float sd_capsule(vec3 p, vec3 a, vec3 b, float r) {
    vec3 pa = p - a;
    vec3 ba = b - a;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h) - r;
  }

  // Box of half-extents b minus a sphere of radius r per Inigo Quilez.
  float sd_rounded_box(vec3 p, vec3 b, float r) {
    vec3 q = abs(p) - b + vec3(r);
    return length(max(q, vec3(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
  }

  // Trilinear sample of the mesh-voxel 3D texture. Outside the voxel
  // grid returns a large positive distance — the bound stays a hard
  // shell, no spurious blends with the rest of the scene.
  float sd_sampled(vec3 p) {
    if (uMeshLoaded != 1) return 1e6;
    vec3 uvw = (p - uMeshMin) / max(uMeshMax - uMeshMin, vec3(1e-6));
    if (any(lessThan(uvw, vec3(0.0))) || any(greaterThan(uvw, vec3(1.0)))) {
      return 1e6;
    }
    return texture(uMeshSdf, uvw).r;
  }

  // Smooth union via log-sum-exp softmin — INGEN §20.1 (numerically
  // stable form, factored by min for finite-precision robustness).
  float smin(float a, float b, float k) {
    float m = min(a, b);
    return m - log(exp(-k * (a - m)) + exp(-k * (b - m))) / k;
  }

  // ---- World building : value noise + FBM (mirrors scenes.ts) ----
  float hash21(vec2 p) {
    p = fract(p * vec2(123.45, 678.91));
    float d = p.x * p.x + p.y * p.y + 45.32;
    p += vec2(d);
    return fract(p.x * p.y);
  }
  float vnoise2(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    vec2 s = f * f * (3.0 - 2.0 * f);
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    return mix(mix(a, b, s.x), mix(c, d, s.x), s.y);
  }
  float fbm2(vec2 p, int octaves) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 6; i++) {
      if (i >= octaves) break;
      v += a * vnoise2(p);
      p *= 2.0;
      a *= 0.5;
    }
    return v;
  }

  // Sky gradient : horizon-to-zenith (atmospheric, Z-up).
  vec3 skyColor(vec3 rd) {
    float t = clamp(rd.z * 0.5 + 0.5, 0.0, 1.0);
    return mix(vec3(0.78, 0.72, 0.65), vec3(0.30, 0.48, 0.72), t);
  }

  // SceneHit carries everything the lit path needs : distance + material.
  // Returned by sceneFull(p). Raymarch only reads .d ; the hit shader
  // reads the rest. cf. evaluateSceneFull in scenes.ts (mirror).
  struct SceneHit {
    float d;
    vec3  color;
    float roughness;
    float metallic;
  };

  SceneHit sceneFull(vec3 p) {
    float dStack[16];
    vec3  cStack[16];
    vec2  mStack[16]; // (roughness, metallic)
    int   sp = 0;
    int   n  = uOpCount;
    // cp = current p modulated by OP_REPEAT for infinite-grid instancing.
    vec3 cp = p;
    // curMat = state set by OP_MATERIAL, tags every following primitive.
    vec3  curColor = vec3(0.84, 0.85, 0.90);
    float curRough = 0.55;
    float curMetal = 0.0;
    for (int i = 0; i < 64; i++) {
      if (i >= n) break;
      vec4 a = uOps[i * 2];
      vec4 b = uOps[i * 2 + 1];
      int op = int(a.x);
      // ---- primitives push (dist, color, roughness, metallic) ----
      if (op == 0) {                                       // SPHERE
        dStack[sp] = sd_sphere(cp - a.yzw, b.x);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 1) {                                // BOX
        dStack[sp] = sd_box(cp - a.yzw, b.xyz);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 2) {                                // TORUS
        dStack[sp] = sd_torus(cp - a.yzw, b.x, b.y);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 3) {                                // CAPSULE
        dStack[sp] = sd_capsule(cp, a.yzw, b.xyz, b.w);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 4) {                                // ROUNDED_BOX
        dStack[sp] = sd_rounded_box(cp - a.yzw, b.xyz, b.w);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 5) {                                // TERRAIN (FBM)
        float h = fbm2(cp.xy * a.z, int(b.x));
        dStack[sp] = cp.z - a.y * h - a.w;
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      } else if (op == 20) {                               // SAMPLED_SDF
        dStack[sp] = sd_sampled(cp);
        cStack[sp] = curColor; mStack[sp] = vec2(curRough, curMetal);
        sp += 1;
      // ---- state ops (do not push) ----
      } else if (op == 30) {                               // REPEAT
        cp.x = a.y > 0.0 ? p.x - a.y * floor(p.x / a.y + 0.5) : p.x;
        cp.y = a.z > 0.0 ? p.y - a.z * floor(p.y / a.z + 0.5) : p.y;
        cp.z = a.w > 0.0 ? p.z - a.w * floor(p.z / a.w + 0.5) : p.z;
      } else if (op == 40) {                               // MATERIAL
        curColor = a.yzw;
        curRough = b.x;
        curMetal = b.y;
      // ---- combinators : propagate winning entry's material ----
      } else if (op == 10) {                               // UNION
        sp -= 1;
        if (dStack[sp] < dStack[sp - 1]) {
          dStack[sp - 1] = dStack[sp];
          cStack[sp - 1] = cStack[sp];
          mStack[sp - 1] = mStack[sp];
        }
      } else if (op == 11) {                               // INTERSECT
        sp -= 1;
        if (dStack[sp] > dStack[sp - 1]) {
          dStack[sp - 1] = dStack[sp];
          cStack[sp - 1] = cStack[sp];
          mStack[sp - 1] = mStack[sp];
        }
      } else if (op == 12) {                               // DIFF (a - b)
        sp -= 1;
        float negB = -dStack[sp];
        if (negB > dStack[sp - 1]) dStack[sp - 1] = negB;
        // material follows A — the carved-out face is still A's surface.
      } else if (op == 13) {                               // SMIN — blend mats by softmin weight
        // Decrement FIRST so dStack[sp] and dStack[sp - 1] are in
        // range (B = old top, A = one below). Reading before pop
        // would touch dStack[sp] out of the live stack range.
        sp -= 1;
        float ad = dStack[sp - 1];
        float bd = dStack[sp];
        float k  = b.x;
        float m  = min(ad, bd);
        float ea = exp(-k * (ad - m));
        float eb = exp(-k * (bd - m));
        float w  = ea / (ea + eb);
        dStack[sp - 1] = m - log(ea + eb) / k;
        cStack[sp - 1] = mix(cStack[sp], cStack[sp - 1], w);
        mStack[sp - 1] = mix(mStack[sp], mStack[sp - 1], w);
      }
    }
    SceneHit hit;
    if (sp > 0) {
      hit.d = dStack[0];
      hit.color = cStack[0];
      hit.roughness = mStack[0].x;
      hit.metallic  = mStack[0].y;
    } else {
      hit.d = 1e9;
      hit.color = vec3(0.8);
      hit.roughness = 0.5;
      hit.metallic = 0.0;
    }
    return hit;
  }

  // Distance-only wrapper for the raymarch loop and the gradient probe.
  // Safety : a NaN / Inf distance from a buggy opcode (or pathological
  // scene) used to crash the WebGL2 context — clamping to a large
  // positive value keeps the raymarch bounded and the UI responsive.
  float scene(vec3 p) {
    float d = sceneFull(p).d;
    return (isnan(d) || isinf(d)) ? 1e9 : d;
  }

  vec3 calc_normal(vec3 p) {
    vec2 e = vec2(0.0015, 0.0);
    return normalize(vec3(
      scene(p + e.xyy) - scene(p - e.xyy),
      scene(p + e.yxy) - scene(p - e.yxy),
      scene(p + e.yyx) - scene(p - e.yyx)
    ));
  }

  void main() {
    vec2 uv = (gl_FragCoord.xy * 2.0 - uResolution) / uResolution.y;
    vec3 rd = normalize(
      uCameraFwd
      + (uv.x * uTanHalfFovY) * uCameraRight
      + (uv.y * uTanHalfFovY) * uCameraUp
    );
    vec3 ro = uCameraPos;

    float t = 0.0;
    bool hit = false;
    float minDist = 1e9;
    // 160 steps + far plane 400 — terrains et villes répétées peuvent
    // s'étendre loin de la caméra.
    for (int i = 0; i < 160; i++) {
      vec3 p = ro + rd * t;
      float d = scene(p);
      if (d < minDist) minDist = d;
      if (d < 0.0015) { hit = true; break; }
      if (t > 400.0) { break; }
      t += d;
    }
    // §19.5 — Gaussian splats accumulate over both hit and miss
    // pixels. Computed once and added to the chosen base colour below.
    vec3 splatRgb = vec3(0.0);
    float splatA = 0.0;
    if (uGaussianCount > 0) {
      for (int gi = 0; gi < 64; gi++) {
        if (gi >= uGaussianCount) break;
        vec4 g0 = uGaussians[gi * 2];
        vec4 g1 = uGaussians[gi * 2 + 1];
        vec4 gclip = uViewProj * vec4(g0.xyz, 1.0);
        if (gclip.w < 0.001) continue;
        vec2 ndc = gclip.xy / gclip.w;
        vec2 gpx = (ndc * 0.5 + 0.5) * uResolution;
        // World-scale σ projected to screen pixels (perspective-aware).
        float scalePx = g0.w * (uResolution.y * 0.5) / max(gclip.w * uTanHalfFovY, 1e-3);
        vec2 dpx = gl_FragCoord.xy - gpx;
        float r2 = dot(dpx, dpx) / (scalePx * scalePx + 1.0);
        float a = exp(-r2 * 0.5) * g1.w;
        splatRgb += g1.rgb * a;
        splatA += a;
      }
    }

    if (!hit) {
      bool hasGlow  = (uGlow == 1 && minDist > 0.0 && minDist < 1.0);
      bool hasSplat = splatA > 0.01;
      bool hasSky   = (uSky == 1);
      if (!hasGlow && !hasSplat && !hasSky) discard;
      vec3 col = hasSky ? skyColor(rd) : vec3(0.0);
      float alpha = hasSky ? 1.0 : 0.0;
      if (hasGlow) {
        float halo = exp(-minDist * 5.0) * 0.6;
        col += vec3(0.35, 0.55, 0.85) * halo;
        alpha = max(alpha, halo);
      }
      col += splatRgb;
      alpha = max(alpha, min(1.0, splatA));
      fragColor = vec4(col, alpha);
      gl_FragDepth = 0.99999;
      return;
    }

    vec3 p = ro + rd * t;
    SceneHit hitMat = sceneFull(p);
    vec3 col;
    if (uDebugMode == 1) {
      // Raw gradient : |grad d| should stay ~ 1 for a Lipschitz-1 SDF.
      // smin / round_box deliberately drift — the heatmap shows where.
      vec2 e = vec2(0.0015, 0.0);
      vec3 grad = vec3(
        scene(p + e.xyy) - scene(p - e.xyy),
        scene(p + e.yxy) - scene(p - e.yxy),
        scene(p + e.yyx) - scene(p - e.yyx)
      ) / (2.0 * e.x);
      float dev = abs(length(grad) - 1.0);
      col = mix(vec3(0.18, 0.78, 0.32), vec3(0.92, 0.18, 0.18), clamp(dev * 2.0, 0.0, 1.0));
    } else {
      vec3 n = calc_normal(p);
      vec3 l = normalize(vec3(0.55, 0.85, 0.40));
      float lambert = max(dot(n, l), 0.0);
      // Blinn-Phong specular tuned by roughness.
      vec3  h = normalize(l - rd);
      float specExp = mix(96.0, 4.0, hitMat.roughness);
      float specPow = pow(max(dot(n, h), 0.0), specExp);
      // Dielectric : white highlight. Metal : colored highlight (albedo-tinted).
      vec3 specCol = mix(vec3(1.0), hitMat.color, hitMat.metallic);
      vec3 ambient = vec3(0.14, 0.16, 0.20) * hitMat.color;
      vec3 diffuse = hitMat.color * lambert * (1.0 - hitMat.metallic);
      vec3 specular = specCol * specPow * (1.0 - hitMat.roughness * 0.5);
      float rim = pow(1.0 - max(dot(n, -rd), 0.0), 2.0);
      col = ambient + diffuse + specular + hitMat.color * rim * 0.10;
    }
    // Splats additionnent leur contribution sur la surface aussi.
    col += splatRgb;
    // Distance fog vers la couleur du ciel — fait fondre les
    // primitives lointaines dans l'atmosphère (rend les mondes
    // crédibles, masque le pop-in à la limite de raymarch).
    if (uFog == 1) {
      float fogA = 1.0 - exp(-t * 0.025);
      vec3 atmosphere = uSky == 1 ? skyColor(rd) : vec3(0.05, 0.07, 0.10);
      col = mix(col, atmosphere, clamp(fogA, 0.0, 1.0));
    }
    fragColor = vec4(col, 1.0);

    // Write depth so the SDF correctly occludes / is occluded by the
    // rest of the scene (grid lines, imported mesh, slicer preview).
    vec4 clip = uViewProj * vec4(p, 1.0);
    gl_FragDepth = (clip.z / clip.w) * 0.5 + 0.5;
  }
`;

export const ForgeBangerCatalog = Object.freeze({
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
});

declare global {
  interface Window {
    ForgeBangerCatalog?: typeof ForgeBangerCatalog;
  }
}

window.ForgeBangerCatalog = ForgeBangerCatalog;
