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

  // Smooth union via log-sum-exp softmin — INGEN §20.1 (numerically
  // stable form, factored by min for finite-precision robustness).
  float smin(float a, float b, float k) {
    float m = min(a, b);
    return m - log(exp(-k * (a - m)) + exp(-k * (b - m))) / k;
  }

  float scene(vec3 p) {
    float stack[16];
    int   sp = 0;
    int   n  = uOpCount;
    for (int i = 0; i < 64; i++) {
      if (i >= n) break;
      vec4 a = uOps[i * 2];
      vec4 b = uOps[i * 2 + 1];
      int op = int(a.x);
      if (op == 0) {                                       // SPHERE
        stack[sp] = sd_sphere(p - a.yzw, b.x);
        sp += 1;
      } else if (op == 1) {                                // BOX
        stack[sp] = sd_box(p - a.yzw, b.xyz);
        sp += 1;
      } else if (op == 2) {                                // TORUS
        stack[sp] = sd_torus(p - a.yzw, b.x, b.y);
        sp += 1;
      } else if (op == 3) {                                // CAPSULE
        stack[sp] = sd_capsule(p, a.yzw, b.xyz, b.w);
        sp += 1;
      } else if (op == 4) {                                // ROUNDED_BOX
        stack[sp] = sd_rounded_box(p - a.yzw, b.xyz, b.w);
        sp += 1;
      } else if (op == 10) {                               // UNION
        sp -= 1;
        stack[sp - 1] = min(stack[sp - 1], stack[sp]);
      } else if (op == 11) {                               // INTERSECT
        sp -= 1;
        stack[sp - 1] = max(stack[sp - 1], stack[sp]);
      } else if (op == 12) {                               // DIFF (a - b)
        sp -= 1;
        stack[sp - 1] = max(stack[sp - 1], -stack[sp]);
      } else if (op == 13) {                               // SMIN
        sp -= 1;
        stack[sp - 1] = smin(stack[sp - 1], stack[sp], b.x);
      }
    }
    return sp > 0 ? stack[0] : 1e9;
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
    for (int i = 0; i < 96; i++) {
      vec3 p = ro + rd * t;
      float d = scene(p);
      if (d < 0.0015) { hit = true; break; }
      if (t > 60.0) { break; }
      t += d;
    }
    if (!hit) discard;

    vec3 p = ro + rd * t;
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
      float rim = pow(1.0 - max(dot(n, -rd), 0.0), 2.0);
      col = vec3(0.12, 0.14, 0.18)
          + vec3(0.80, 0.74, 0.68) * lambert
          + vec3(0.35, 0.50, 0.75) * rim * 0.20;
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
