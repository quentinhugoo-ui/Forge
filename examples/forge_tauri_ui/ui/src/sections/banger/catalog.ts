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

// INGEN COMPUTE §19 — Phase 1 a supprimé makeCube + makeGrid + VS_SDF/FS_SDF.
// Phase 4 supprime les derniers shaders WebGL2 (VS_MESH/FS_MESH/VS_LINE/FS_LINE)
// et l'init `getContext("webgl2")`. Tout le rendu vit dans INGEN Render
// (ingen-render.ts, WebGPU compute). Ce catalog ne publie plus que les
// helpers maths utilisés par le gizmo SVG et la sélection 2D.
export const ForgeBangerCatalog = Object.freeze({
  M4,
  AXIS_RGB,
  AXIS_HEX,
});

declare global {
  interface Window {
    ForgeBangerCatalog?: typeof ForgeBangerCatalog;
  }
}

window.ForgeBangerCatalog = ForgeBangerCatalog;
