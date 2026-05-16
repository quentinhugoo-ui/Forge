type Alpha3dWebglState = any;

const VS_SRC = `#version 300 es
in vec3 a_position;
in vec3 a_color;
in float a_size;
uniform mat4 u_proj;
uniform mat4 u_view;
uniform float u_pointSizeBase;
out vec3 v_color;
void main() {
  gl_Position = u_proj * u_view * vec4(a_position, 1.0);
  gl_PointSize = u_pointSizeBase * a_size;
  v_color = a_color;
}`;

const FS_SRC = `#version 300 es
precision highp float;
in vec3 v_color;
out vec4 fragColor;
void main() {
  fragColor = vec4(v_color, 1.0);
}`;

export function initAlpha3dWebgl(canvas: HTMLCanvasElement | null, state: Alpha3dWebglState): boolean {
  if (!canvas) return false;
  const gl = canvas.getContext("webgl2", { antialias: true, alpha: true, premultipliedAlpha: false });
  if (!gl) {
    console.error("[3d] WebGL2 not available");
    return false;
  }
  state.gl = gl;

  const program = makeAlpha3dProgram(gl, VS_SRC, FS_SRC);
  if (!program) return false;
  state.program = program;
  state.uniforms.proj = gl.getUniformLocation(program, "u_proj");
  state.uniforms.view = gl.getUniformLocation(program, "u_view");
  state.uniforms.pointSizeBase = gl.getUniformLocation(program, "u_pointSizeBase");

  state.buffers.position = gl.createBuffer();
  state.buffers.color = gl.createBuffer();
  state.buffers.size = gl.createBuffer();
  state.lineBuffers.position = gl.createBuffer();
  state.lineBuffers.color = gl.createBuffer();
  state.lineBuffers.size = gl.createBuffer();
  state.vao = gl.createVertexArray();
  state.lineVao = gl.createVertexArray();

  bindAlpha3dVao(gl, program, state.vao, state.buffers);
  bindAlpha3dVao(gl, program, state.lineVao, state.lineBuffers);
  gl.bindVertexArray(null);

  gl.clearColor(0, 0, 0, 0);
  gl.disable(gl.BLEND);
  gl.enable(gl.DEPTH_TEST);
  gl.depthFunc(gl.LEQUAL);
  return true;
}

export function resizeAlpha3dCanvasToDisplay(
  canvas: HTMLCanvasElement | null,
  state: Alpha3dWebglState,
  dpr: number,
): void {
  if (!canvas || !state.gl) return;
  const rect = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.floor(rect.width * dpr));
  const h = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.width !== w || canvas.height !== h) {
    canvas.width = w;
    canvas.height = h;
    state.gl.viewport(0, 0, w, h);
  }
}

export function uploadAlpha3dPayload(state: Alpha3dWebglState, payload: Alpha3dWebglState): void {
  const gl = state.gl;
  if (!gl) return;
  gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.position);
  gl.bufferData(gl.ARRAY_BUFFER, payload.positions, gl.STATIC_DRAW);
  gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.color);
  gl.bufferData(gl.ARRAY_BUFFER, payload.colors, gl.STATIC_DRAW);
  gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.size);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    payload.sizes || new Float32Array(state.pointCount).fill(1.0),
    gl.STATIC_DRAW,
  );
  gl.bindBuffer(gl.ARRAY_BUFFER, state.lineBuffers.position);
  gl.bufferData(gl.ARRAY_BUFFER, payload.linePositions || new Float32Array(0), gl.STATIC_DRAW);
  gl.bindBuffer(gl.ARRAY_BUFFER, state.lineBuffers.color);
  gl.bufferData(gl.ARRAY_BUFFER, payload.lineColors || new Float32Array(0), gl.STATIC_DRAW);
  gl.bindBuffer(gl.ARRAY_BUFFER, state.lineBuffers.size);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    payload.lineSizes || new Float32Array(state.linePointCount).fill(1.0),
    gl.STATIC_DRAW,
  );
}

export function renderAlpha3dWebglFrame(
  state: Alpha3dWebglState,
  matrices: { proj: Float32Array; view: Float32Array },
  dpr: number,
): boolean {
  const gl = state.gl;
  if (!gl) return false;
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  if (state.pointCount === 0) return false;

  gl.useProgram(state.program);
  gl.uniformMatrix4fv(state.uniforms.proj, false, matrices.proj);
  gl.uniformMatrix4fv(state.uniforms.view, false, matrices.view);
  gl.uniform1f(state.uniforms.pointSizeBase, (state.pointSize || 4.0) * dpr);
  gl.bindVertexArray(state.vao);
  let drawType = gl.POINTS;
  if (state.drawMode === "lines") drawType = gl.LINE_STRIP;
  if (state.drawMode === "linepairs") drawType = gl.LINES;
  if (state.drawMode === "triangles") drawType = gl.TRIANGLES;
  gl.drawArrays(drawType, 0, state.pointCount);
  if (state.linePointCount > 0 && state.lineVao) {
    gl.bindVertexArray(state.lineVao);
    gl.lineWidth(1);
    let lineType = gl.LINES;
    if (state.lineDrawMode === "lines") lineType = gl.LINE_STRIP;
    gl.drawArrays(lineType, 0, state.linePointCount);
  }
  gl.bindVertexArray(null);
  return true;
}

function bindAlpha3dVao(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  vao: WebGLVertexArrayObject | null,
  buffers: { position: WebGLBuffer | null; color: WebGLBuffer | null; size: WebGLBuffer | null },
): void {
  gl.bindVertexArray(vao);
  const aPos = gl.getAttribLocation(program, "a_position");
  gl.bindBuffer(gl.ARRAY_BUFFER, buffers.position);
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 3, gl.FLOAT, false, 0, 0);
  const aCol = gl.getAttribLocation(program, "a_color");
  gl.bindBuffer(gl.ARRAY_BUFFER, buffers.color);
  gl.enableVertexAttribArray(aCol);
  gl.vertexAttribPointer(aCol, 3, gl.FLOAT, false, 0, 0);
  const aSize = gl.getAttribLocation(program, "a_size");
  if (aSize >= 0) {
    gl.bindBuffer(gl.ARRAY_BUFFER, buffers.size);
    gl.enableVertexAttribArray(aSize);
    gl.vertexAttribPointer(aSize, 1, gl.FLOAT, false, 0, 0);
  }
}

function makeAlpha3dProgram(
  gl: WebGL2RenderingContext,
  vsSrc: string,
  fsSrc: string,
): WebGLProgram | null {
  const compile = (type: number, src: string) => {
    const shader = gl.createShader(type);
    if (!shader) return null;
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.error("[3d] shader error:", gl.getShaderInfoLog(shader));
      return null;
    }
    return shader;
  };
  const vs = compile(gl.VERTEX_SHADER, vsSrc);
  const fs = compile(gl.FRAGMENT_SHADER, fsSrc);
  if (!vs || !fs) return null;
  const program = gl.createProgram();
  if (!program) return null;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.error("[3d] link error:", gl.getProgramInfoLog(program));
    return null;
  }
  return program;
}
