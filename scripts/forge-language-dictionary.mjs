#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = path.join(root, "src", "kasm.rs");
const docPath = path.join(root, "FORGE_NATIVE_BYTECODE.md");
const startMarker = "<!-- forge-language-dictionary:generated:start -->";
const endMarker = "<!-- forge-language-dictionary:generated:end -->";

const args = new Set(process.argv.slice(2));
const checkOnly = args.has("--check");

const source = fs.readFileSync(sourcePath, "utf8");
const doc = fs.readFileSync(docPath, "utf8");

function findBraceBody(text, openBraceIndex) {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = openBraceIndex; index < text.length; index += 1) {
    const ch = text[index];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (ch === "\\") {
        escaped = true;
      } else if (ch === "\"") {
        inString = false;
      }
      continue;
    }
    if (ch === "\"") {
      inString = true;
      continue;
    }
    if (ch === "{") depth += 1;
    if (ch === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(openBraceIndex + 1, index);
    }
  }
  throw new Error("unclosed Rust body");
}

function functionBody(name, fromIndex = 0) {
  return functionBodyFrom(source, name, fromIndex);
}

function functionBodyFrom(text, name, fromIndex = 0) {
  const fnIndex = text.indexOf(`fn ${name}`, fromIndex);
  if (fnIndex < 0) throw new Error(`missing function ${name}`);
  const open = text.indexOf("{", fnIndex);
  return findBraceBody(text, open);
}

function implFunctionBody(implName, functionName) {
  const implIndex = source.indexOf(`impl ${implName}`);
  if (implIndex < 0) throw new Error(`missing impl ${implName}`);
  const fnIndex = source.indexOf(`pub fn ${functionName}`, implIndex);
  if (fnIndex < 0) throw new Error(`missing ${implName}::${functionName}`);
  const open = source.indexOf("{", fnIndex);
  return findBraceBody(source, open);
}

function stringWords(text) {
  const out = new Set();
  for (const match of text.matchAll(/"([A-Za-z_][A-Za-z0-9_]*)"/g)) {
    out.add(match[1]);
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function union(...lists) {
  return [...new Set(lists.flat())].sort((a, b) => a.localeCompare(b));
}

function enumBody(name) {
  return enumBodyFrom(source, name);
}

function enumBodyFrom(text, name) {
  const enumIndex = text.indexOf(`pub enum ${name}`);
  if (enumIndex < 0) throw new Error(`missing enum ${name}`);
  const open = text.indexOf("{", enumIndex);
  return findBraceBody(text, open);
}

function enumVariants(name) {
  return enumVariantsFrom(source, name);
}

function enumVariantsFrom(text, name) {
  return [...enumBodyFrom(text, name).matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\b/gm)]
    .map((match) => match[1]);
}

function enumVariantsWithValues(name) {
  return enumVariantsWithValuesFrom(source, name);
}

function enumVariantsWithValuesFrom(text, name) {
  return [...enumBodyFrom(text, name).matchAll(/^\s*([A-Z][A-Za-z0-9_]*)\s*=\s*([0-9]+)/gm)]
    .map((match) => ({ name: match[1], value: Number(match[2]) }));
}

function structFields(name) {
  return structFieldsFrom(source, name);
}

function structFieldsFrom(text, name) {
  const structIndex = text.indexOf(`pub struct ${name}`);
  if (structIndex < 0) throw new Error(`missing struct ${name}`);
  const open = text.indexOf("{", structIndex);
  const body = findBraceBody(text, open);
  return [...body.matchAll(/^\s*pub\s+([a-z_][A-Za-z0-9_]*)\s*:/gm)]
    .map((match) => match[1]);
}

function namedConstValues(names, text = source) {
  return names.map((name) => {
    const pattern = new RegExp(`(?:pub(?:\\(super\\))?\\s+)?const\\s+${name}\\s*:[^=]+=\\s*([^;]+);`);
    const match = text.match(pattern);
    if (!match) throw new Error(`missing const ${name}`);
    return `${name}=${match[1].trim()}`;
  });
}

function publicNames(kind) {
  const pattern = new RegExp(`^pub\\s+${kind}\\s+([A-Za-z_][A-Za-z0-9_]*)`, "gm");
  return [...source.matchAll(pattern)].map((match) => match[1]).sort((a, b) => a.localeCompare(b));
}

function publicConstNames() {
  const out = new Set();
  for (const match of source.matchAll(/^pub(?:\([^)]+\))?\s+const\s+([A-Z][A-Z0-9_]*):/gm)) {
    out.add(match[1]);
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function associatedConstNames(typeName) {
  const pattern = new RegExp(`pub\\s+const\\s+([A-Z][A-Z0-9_]*):\\s+${typeName}\\b`, "g");
  return [...source.matchAll(pattern)].map((match) => match[1]).sort((a, b) => a.localeCompare(b));
}

function publicFunctionNames() {
  const out = new Set();
  for (const match of source.matchAll(/^\s*pub\s+fn\s+([a-z_][A-Za-z0-9_]*)/gm)) {
    out.add(match[1]);
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function computePublicEnumVariantInventory(enumNames) {
  const out = [];
  for (const name of enumNames) {
    for (const variant of enumVariants(name)) {
      out.push(`${name}::${variant}`);
    }
  }
  return out.sort((a, b) => a.localeCompare(b));
}

function computePublicStructFieldInventory(structNames) {
  const out = [];
  for (const name of structNames) {
    const fields = structFields(name);
    if (fields.length === 0) {
      out.push(`${name}::<tuple_or_marker>`);
    } else {
      for (const field of fields) {
        out.push(`${name}.${field}`);
      }
    }
  }
  return out.sort((a, b) => a.localeCompare(b));
}

function associatedConstInventory() {
  const out = new Set();
  const implRe = /impl(?:<[^>{}]+>)?\s+([A-Za-z_][A-Za-z0-9_]*)(?:<[^>{}]+>)?[^{]*\{/g;
  for (const match of source.matchAll(implRe)) {
    const typeName = match[1];
    const open = match.index + match[0].length - 1;
    const body = findBraceBody(source, open);
    for (const constMatch of body.matchAll(/pub\s+const\s+([A-Z][A-Z0-9_]*):/g)) {
      out.add(`${typeName}::${constMatch[1]}`);
    }
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function codeList(values) {
  return values.map((value) => `\`${value}\``).join(", ");
}

function wrappedCodeList(values, indent = "") {
  const words = values.map((value, index) => {
    const suffix = index + 1 === values.length ? "" : ",";
    return `\`${value}\`${suffix}`;
  });
  const lines = [];
  let current = indent;
  for (const word of words) {
    const next = current.trim().length === 0 ? `${indent}${word}` : `${current} ${word}`;
    if (next.length > 88 && current.trim().length > 0) {
      lines.push(current);
      current = `${indent}${word}`;
    } else {
      current = next;
    }
  }
  if (current.trim().length > 0) lines.push(current);
  return lines.join("\n");
}

function bulletWrappedCodeList(values) {
  const lines = wrappedCodeList(values, "").split("\n");
  return lines.map((line, index) => `${index === 0 ? "- " : "  "}${line}`).join("\n");
}

function wrappedEnumValueList(values, indent = "") {
  return wrappedCodeList(values.map(({ name, value }) => `${name}=${value}`), indent);
}

function tablegenMnemonics() {
  const out = new Set();
  for (const match of source.matchAll(/def\s+Kasm_[A-Za-z0-9_]+\s*:\s*Kasm_[A-Za-z0-9_]*<"([^"]+)"/g)) {
    out.add(match[1]);
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function tablegenTargetSpellings() {
  const out = new Set();
  for (const match of source.matchAll(/I32EnumAttrCase<"[^"]+",\s*[0-9]+,\s*"([^"]+)"/g)) {
    out.add(match[1]);
  }
  return [...out].sort((a, b) => a.localeCompare(b));
}

function extractStartsWithPrefixes(functionName) {
  const body = functionBody(functionName);
  return [...body.matchAll(/starts_with\("([^"]+)"\)/g)].map((match) => match[1]).sort();
}

const sectionNames = stringWords(functionBody("is_forge_section_name"));
const numericLiteralSuffixes = stringWords(functionBody("split_numeric_suffix"));
const scalarAliases = stringWords(implFunctionBody("ForgeScalarTy", "parse"));
const typeBody = implFunctionBody("ForgeType", "parse");
const namedTypeLiterals = stringWords(typeBody);
const typeFamilies = [
  "array<T,N>",
  "column<T>",
  "complex<T>",
  "field<T,R>",
  "graph<N,E>",
  "mat<T,C,R>",
  "table<name:type;...>",
  "tensor<T,shape>",
  "vec<T,N>",
];
const typeWords = union(
  scalarAliases,
  namedTypeLiterals.filter((word) => !["x"].includes(word)),
  typeFamilies,
);
const unitWords = stringWords(functionBody("named"));
const handoffKinds = stringWords(implFunctionBody("ForgeOutputKind", "parse"));
const precisionValues = stringWords(implFunctionBody("ForgePrecision", "parse"));
const moduleHashFields = structFields("ForgeModuleHashes");
const forbiddenPrefixes = extractStartsWithPrefixes("is_forbidden_forge_effect_name");
const forbiddenEffects = stringWords(functionBody("is_forbidden_forge_effect_name"))
  .filter((word) => !forbiddenPrefixes.includes(word));
const unboundedControl = stringWords(functionBody("is_unbounded_forge_control_name"));
const reservedControl = stringWords(functionBody("is_reserved_forge_control_ident"));

const callTyBody = functionBody("infer_forge_call_ty");
const builtinTyBody = functionBody("infer_builtin_forge_call_ty");
const dataParallelBody = functionBody("infer_data_parallel_function_call_ty");
const allCallWords = union(stringWords(callTyBody), stringWords(builtinTyBody), stringWords(dataParallelBody));

const numericBuiltins = [
  "add", "sub", "mul", "div", "mod", "rem", "neg", "finite", "abs", "min",
  "max", "clamp", "saturate", "floor", "ceil", "round", "trunc", "fract",
  "sign", "copysign", "fma", "lerp", "mix", "sqrt", "rsqrt", "cbrt", "pow",
  "exp", "exp2", "ln", "log", "log2", "log10", "sin", "cos", "tan", "asin",
  "acos", "atan", "atan2", "sinh", "cosh", "tanh", "erf", "erfc", "gamma",
  "lgamma", "beta",
].filter((word) => allCallWords.includes(word));
const booleanBuiltins = [
  "eq", "ne", "lt", "le", "gt", "ge", "and", "or", "xor", "not", "any",
  "all", "where", "select",
].filter((word) => allCallWords.includes(word));
const bitIntegerBuiltins = [
  "shl", "shr", "rotl", "rotr", "bit_and", "bit_or", "bit_xor", "bit_not",
  "popcount", "clz", "ctz", "byte_swap", "bit_reverse", "hash32", "hash64",
].filter((word) => allCallWords.includes(word));
const vectorBuiltins = [
  "vec2", "vec3", "vec4", "mat2", "mat3", "mat4", "complex", "dot", "length",
  "distance", "normalize", "cross", "outer", "matmul", "transpose",
  "determinant", "inverse", "trace", "eigen_small", "svd_small", "qr_small",
  "cholesky_small",
].filter((word) => allCallWords.includes(word));
const collectionBuiltins = [
  "len", "shape", "rank", "size", "reshape", "flatten", "squeeze",
  "unsqueeze", "slice", "concat", "split", "tile", "repeat", "broadcast",
  "transpose_axes", "permute", "index", "sum", "rows", "cols", "node_count",
  "edge_count", "sample",
].filter((word) => allCallWords.includes(word));
const dataParallelBuiltins = [
  "map", "zip", "zip_with", "reduce", "fold", "scan", "prefix_sum", "filter",
  "compact", "partition", "sort", "argsort", "unique", "histogram",
  "bin_count", "gather", "take", "scatter", "scatter_add", "scatter_min",
  "scatter_max", "masked_load", "masked_store", "atomic_add", "atomic_min",
  "atomic_max",
].filter((word) => allCallWords.includes(word));
const queryBuiltins = [
  "window", "rolling", "groupby", "join", "asof_join",
].filter((word) => allCallWords.includes(word));
const samplingBuiltins = [
  "rng_seed", "uniform", "normal", "lognormal", "poisson", "bernoulli",
  "sobol", "halton", "latin_hypercube", "stratified_sample", "monte_carlo",
  "importance_sample", "resample",
].filter((word) => allCallWords.includes(word));
const statisticsBuiltins = [
  "mean", "variance", "std", "covariance", "correlation", "quantile", "median",
  "minmax", "zscore", "normalize_stats", "linear_regression", "robust_loss",
].filter((word) => allCallWords.includes(word));
const selectionBuiltins = [
  "argmin", "argmax", "pareto", "pareto_front", "rank", "top_k",
  "diversity_select",
].filter((word) => allCallWords.includes(word));
const optimizationBuiltins = [
  "gradient_descent_step", "adam_step", "newton_step", "bfgs_step",
  "line_search", "project_bounds", "constraint_penalty",
].filter((word) => allCallWords.includes(word));
const autodiffBuiltins = [
  "grad", "jacobian", "hessian_diag", "jvp", "vjp", "finite_diff_check",
  "sensitivity_forward", "sensitivity_adjoint",
].filter((word) => allCallWords.includes(word));
const solverBuiltins = [
  "root_find", "bisection", "newton_root", "fixed_point", "linear_solve",
  "sparse_solve", "least_squares", "ode_step_euler", "ode_step_rk4",
  "ode_solve", "pde_stencil_step", "relaxation_step",
].filter((word) => allCallWords.includes(word));
const signalBuiltins = [
  "fft", "ifft", "rfft", "convolution", "fir_filter", "iir_filter",
  "window_hann", "window_blackman", "spectrogram", "wavelet_step",
].filter((word) => allCallWords.includes(word));
const sparseGraphBuiltins = [
  "csr_matvec", "coo_to_csr", "sparse_reduce", "graph_neighbors",
  "graph_degree", "bfs_step", "shortest_path_step", "pagerank_step",
  "connected_components_step",
].filter((word) => allCallWords.includes(word));
const geometryBuiltins = [
  "transform_point", "transform_normal", "sdf_sphere", "sdf_box",
  "sdf_capsule", "sdf_torus", "sdf_union", "sdf_intersection", "sdf_subtract",
  "sdf_smooth_union", "gradient_field", "normal_from_sdf", "raymarch_step",
  "marching_cubes_cell", "voxel_sample", "surfel_accumulate",
].filter((word) => allCallWords.includes(word));
const physicsBuiltins = [
  "integrate_force", "integrate_velocity", "inertia_tensor",
  "stress_tensor_basic", "strain_basic", "thermal_flux_step",
  "fluid_advect_step", "pressure_projection_step", "collision_distance",
  "constraint_project",
].filter((word) => allCallWords.includes(word));
const contractBuiltins = [
  "unit_cast", "dimensional_check", "bounds_check", "finite_check",
  "nan_guard", "invariant", "assert", "approx_equal", "hash_value",
  "hash_buffer", "proof_emit",
].filter((word) => allCallWords.includes(word));
const memoryBuiltins = [
  "byte_load", "byte_store", "u32_load", "f32_load", "span", "slice_view",
  "page_id", "pointer_tag", "dom_node_record", "graph_edge_record",
  "memory_region_hash",
].filter((word) => allCallWords.includes(word));
const cryptoBuiltins = [
  "sha256_block", "blake3_chunk", "merkle_pair", "hmac_block", "xor_stream",
  "random_oracle_probe",
].filter((word) => allCallWords.includes(word));
const boundedControlBuiltins = ["fori", "while_fuel"].filter((word) => allCallWords.includes(word));
const bytecodeTargets = enumVariantsWithValues("Target");
const bytecodeTypes = enumVariantsWithValues("Ty");
const bytecodeOps = enumVariantsWithValues("Op");
const f64SubOps = enumVariants("F64SubOp");
const f64ImmediateConstants = namedConstValues([
  "F64_ADD", "F64_SUB", "F64_MUL", "F64_DIV", "F64_MIN", "F64_MAX",
  "F64_SQRT", "F64_ABS", "F64_NEG", "F64_FROM_I64", "F64_TO_I64",
  "F64_EXP", "F64_LN", "F64_OP_MAX",
]);
const bytecodeWireConstants = namedConstValues([
  "HEADER_LEN", "NODE_LEN", "FOOTER_LEN", "MAX_NODES", "MAX_SLOTS",
  "MAGIC", "VERSION",
]);
const bytecodeNodeFields = structFields("Node");
const kasmErrorVariants = enumVariants("KasmError");
const tensorWireConstants = namedConstValues([
  "TENSOR_MAGIC", "TENSOR_VERSION", "TENSOR_HEADER_LEN", "TENSOR_FOOTER_LEN",
  "TENSOR_NODE_LEN", "TENSOR_MAX_NODES", "TENSOR_MAX_DIMS", "TENSOR_MAX_SLOTS",
  "TENSOR_MAX_DIM_EXTENT",
]);
const tensorTypes = enumVariantsWithValues("TensorTy");
const tensorOps = enumVariantsWithValues("TensorOp");
const tensorNodeFields = structFields("TensorNode");
const tensorErrorVariants = enumVariants("TensorError");
const reductionTrees = enumVariantsWithValues("ReductionTree");
const kernelFamilies = enumVariantsWithValues("KernelFamily");
const roundModes = enumVariantsWithValues("RoundMode");
const numericContractFields = structFields("NumericContract");
const fbcWireConstants = namedConstValues(["FBC_VERSION", "FBC_VERIFIER_VERSION"]);
const fbcProgramFields = structFields("ForgeBytecodeProgram");
const fbcOpcodes = enumVariants("ForgeOpcode");
const fbcCapabilityKinds = enumVariants("ForgeCapabilityKind");
const fbcCapabilityFields = structFields("ForgeCapability");
const fbcHostcalls = enumVariants("ForgeHostCall");
const fbcVmStatus = enumVariants("ForgeVmStatus");
const fbcVmErrors = enumVariants("ForgeVmError");
const fbcVerifierFields = structFields("ForgeVerifierReport");
const fbcRunProofFields = structFields("ForgeRunProof");
const fbcVmConfigFields = structFields("ForgeVmConfig");
const fbcTextDirectives = ["name=", "schema=", "deterministic=", "hostcall=", "cap=", "op="];
const fbcTextOpcodes = stringWords(functionBody("opcode_name"));
const fbcHostcallNames = stringWords(functionBody("hostcall_name"));
const fbcCapabilityNames = stringWords(functionBody("cap_kind_name"));
const embeddedDialectConstants = ["FORGE_EMBEDDED_KASM_TABLEGEN_DIALECT"];
const embeddedDialectTargets = tablegenTargetSpellings();
const embeddedDialectMnemonics = tablegenMnemonics();
const publicEnumInventory = publicNames("enum");
const publicStructInventory = publicNames("struct");
const publicConstInventory = publicConstNames();
const kasmErrnoConstants = associatedConstNames("KasmErrno");
const publicFunctionInventory = publicFunctionNames();
const publicEnumVariantInventory = computePublicEnumVariantInventory(publicEnumInventory);
const publicStructFieldInventory = computePublicStructFieldInventory(publicStructInventory);
const publicAssociatedConstInventory = associatedConstInventory();

const declarationWords = [
  "module", "version", "none", "import", "hash", "sha256", "const", "fn",
  "return", "let", "emit", "param", "output", "unit", "bounds", "nominal",
  "handoff", "case", "seed", "given", "expect", "tolerance", "max_steps",
  "max_memory_mb", "precision", "parallelism", "proof_hash", "output_hash",
  "compact_result",
];
const expressionTokens = [
  "true", "false", "-x", "!flag", "+", "-", "*", "/", "^", "==", "!=", "<",
  "<=", ">", ">=", "&&", "||", "(...)", "<...>", "[...]", "{...}", ",", ";",
  ":", "=", "->",
];

function generatedDictionary() {
  return [
    "### Forge Language Dictionary",
    "",
    startMarker,
    "",
    "This block is generated from `src/kasm.rs` by",
    "`node scripts/forge-language-dictionary.mjs`. Update it with that script;",
    "use `--check` to fail when the document drifts from the parser.",
    "",
    "User identifiers may still introduce module, function, parameter, constant,",
    "let and output names, but they must not collide with reserved control names",
    "or forbidden effect names.",
    "",
    "Sections:",
    "",
    bulletWrappedCodeList(sectionNames),
    "",
    "Module and declaration words:",
    "",
    bulletWrappedCodeList(declarationWords),
    "",
    "Types parsed by `ForgeType::parse` and `ForgeScalarTy::parse`:",
    "",
    bulletWrappedCodeList(typeWords),
    "",
    "Units parsed by `ForgeUnitDim::named` plus composed unit expressions:",
    "",
    bulletWrappedCodeList(unitWords),
    "- composed forms may use `*`, `/` and `^`, for example `kg*m/s^2` or `m^2`.",
    "",
    "Output handoff kinds parsed by `ForgeOutputKind::parse`:",
    "",
    bulletWrappedCodeList(handoffKinds),
    "",
    "Precision values parsed by `ForgePrecision::parse`:",
    "",
    bulletWrappedCodeList(precisionValues),
    "",
    "Module hash/proof fields emitted by `ForgeModuleHashes`:",
    "",
    bulletWrappedCodeList(moduleHashFields),
    "",
    "Expression syntax:",
    "",
    bulletWrappedCodeList(expressionTokens),
    "- identifiers start with an ASCII letter or `_`, then continue with ASCII",
    "  letters, digits or `_`.",
    `- numeric literal suffixes: ${codeList(numericLiteralSuffixes)}; underscores are ignored;`,
    "  unsuffixed integers parse as `i64`, unsuffixed decimal/exponent numbers parse as `f64`.",
    "",
    "Constraint and proof words:",
    "",
    "- `assert`, `finite`, `bounds`, `approx`, `proof_hash`, `output_hash`,",
    "  `compact_result`.",
    "",
    "Current builtin calls:",
    "",
    `- numeric: ${codeList(numericBuiltins)};`,
    `- boolean/comparison: ${codeList(booleanBuiltins)};`,
    `- bit/integer/hash: ${codeList(bitIntegerBuiltins)};`,
    `- vector/matrix/complex: ${codeList(vectorBuiltins)};`,
    `- collection/shape: ${codeList(collectionBuiltins)};`,
    `- data-parallel: ${codeList(dataParallelBuiltins)};`,
    `- query/time: ${codeList(queryBuiltins)};`,
    `- sampling: ${codeList(samplingBuiltins)};`,
    `- statistics: ${codeList(statisticsBuiltins)};`,
    `- selection: ${codeList(selectionBuiltins)};`,
    `- optimization: ${codeList(optimizationBuiltins)};`,
    `- autodiff: ${codeList(autodiffBuiltins)};`,
    `- solvers: ${codeList(solverBuiltins)};`,
    `- signal/FFT: ${codeList(signalBuiltins)};`,
    `- sparse/graph: ${codeList(sparseGraphBuiltins)};`,
    `- geometry/SDF/3D: ${codeList(geometryBuiltins)};`,
    `- physics/engineering: ${codeList(physicsBuiltins)};`,
    `- units/contracts/proof: ${codeList(contractBuiltins)};`,
    `- memory/DOM/RAM future: ${codeList(memoryBuiltins)};`,
    `- crypto/hash: ${codeList(cryptoBuiltins)};`,
    `- bounded control: ${codeList(boundedControlBuiltins)}.`,
    "",
    "Reserved or rejected names:",
    "",
    `- unbounded control calls are rejected: ${codeList(unboundedControl)};`,
    `- reserved control identifiers cannot be used as function names: ${codeList(reservedControl)};`,
    "- forbidden effect calls are rejected:",
    wrappedCodeList(forbiddenEffects, "  "),
    `- forbidden effect prefixes are rejected: ${codeList(forbiddenPrefixes)}.`,
    "",
    "Bytecode runtime dictionary:",
    "",
    "- bytecode wire constants:",
    wrappedCodeList(bytecodeWireConstants, "  "),
    "- bytecode targets from `Target`:",
    wrappedEnumValueList(bytecodeTargets, "  "),
    "- bytecode types from `Ty`:",
    wrappedEnumValueList(bytecodeTypes, "  "),
    "- bytecode opcodes from `Op`:",
    wrappedEnumValueList(bytecodeOps, "  "),
    "- bytecode node fields from `Node`:",
    wrappedCodeList(bytecodeNodeFields, "  "),
    "- bytecode runtime errors from `KasmError`:",
    wrappedCodeList(kasmErrorVariants, "  "),
    "- `F64Op` immediate sub-ops from `F64SubOp`:",
    wrappedCodeList(f64SubOps, "  "),
    "- `F64Op` immediate constants:",
    wrappedCodeList(f64ImmediateConstants, "  "),
    "",
    "Tensor runtime dictionary:",
    "",
    "- tensor wire constants:",
    wrappedCodeList(tensorWireConstants, "  "),
    "- tensor dtypes from `TensorTy`:",
    wrappedEnumValueList(tensorTypes, "  "),
    "- tensor opcodes from `TensorOp`:",
    wrappedEnumValueList(tensorOps, "  "),
    "- tensor node fields from `TensorNode`:",
    wrappedCodeList(tensorNodeFields, "  "),
    "- tensor runtime errors from `TensorError`:",
    wrappedCodeList(tensorErrorVariants, "  "),
    "- numeric contract reduction trees from `ReductionTree`:",
    wrappedEnumValueList(reductionTrees, "  "),
    "- numeric contract kernel families from `KernelFamily`:",
    wrappedEnumValueList(kernelFamilies, "  "),
    "- numeric contract round modes from `RoundMode`:",
    wrappedEnumValueList(roundModes, "  "),
    "- numeric contract fields from `NumericContract`:",
    wrappedCodeList(numericContractFields, "  "),
    "",
    "FBC v0 dictionary:",
    "",
    "- FBC wire constants:",
    wrappedCodeList(fbcWireConstants, "  "),
    "- FBC text directives parsed by `parse_program_v0`:",
    wrappedCodeList(fbcTextDirectives, "  "),
    "- FBC program fields from `ForgeBytecodeProgram`:",
    wrappedCodeList(fbcProgramFields, "  "),
    "- FBC op enum variants from `ForgeOpcode`:",
    wrappedCodeList(fbcOpcodes, "  "),
    "- FBC text opcode names from `opcode_name`:",
    wrappedCodeList(fbcTextOpcodes, "  "),
    "- FBC capability kinds from `ForgeCapabilityKind`:",
    wrappedCodeList(fbcCapabilityKinds, "  "),
    "- FBC capability text names from `cap_kind_name`:",
    wrappedCodeList(fbcCapabilityNames, "  "),
    "- FBC capability fields from `ForgeCapability`:",
    wrappedCodeList(fbcCapabilityFields, "  "),
    "- FBC hostcalls from `ForgeHostCall`:",
    wrappedCodeList(fbcHostcalls, "  "),
    "- FBC hostcall text names from `hostcall_name`:",
    wrappedCodeList(fbcHostcallNames, "  "),
    "- FBC VM statuses from `ForgeVmStatus`:",
    wrappedCodeList(fbcVmStatus, "  "),
    "- FBC VM errors from `ForgeVmError`:",
    wrappedCodeList(fbcVmErrors, "  "),
    "- FBC verifier report fields from `ForgeVerifierReport`:",
    wrappedCodeList(fbcVerifierFields, "  "),
    "- FBC run proof fields from `ForgeRunProof`:",
    wrappedCodeList(fbcRunProofFields, "  "),
    "- FBC VM config fields from `ForgeVmConfig`:",
    wrappedCodeList(fbcVmConfigFields, "  "),
    "",
    "Embedded KASM/Forge dialect reference:",
    "",
    "- embedded dialect constants:",
    wrappedCodeList(embeddedDialectConstants, "  "),
    "- embedded dialect target spellings:",
    wrappedCodeList(embeddedDialectTargets, "  "),
    "- embedded dialect operation mnemonics:",
    wrappedCodeList(embeddedDialectMnemonics, "  "),
    "",
    "Complete public inventory from `src/kasm.rs`:",
    "",
    "- public enums:",
    wrappedCodeList(publicEnumInventory, "  "),
    "- public structs:",
    wrappedCodeList(publicStructInventory, "  "),
    "- public constants:",
    wrappedCodeList(publicConstInventory, "  "),
    "- public `KasmErrno` constants:",
    wrappedCodeList(kasmErrnoConstants, "  "),
    "- public functions:",
    wrappedCodeList(publicFunctionInventory, "  "),
    "- public enum variants:",
    wrappedCodeList(publicEnumVariantInventory, "  "),
    "- public struct fields:",
    wrappedCodeList(publicStructFieldInventory, "  "),
    "- public associated constants:",
    wrappedCodeList(publicAssociatedConstInventory, "  "),
    "",
    endMarker,
  ].join("\n");
}

function replaceDictionaryBlock(currentDoc, block) {
  const markerStart = currentDoc.indexOf(startMarker);
  const markerEnd = currentDoc.indexOf(endMarker);
  if (markerStart >= 0 && markerEnd >= markerStart) {
    const headerStart = currentDoc.lastIndexOf("### Forge Language Dictionary", markerStart);
    const afterEnd = markerEnd + endMarker.length;
    return `${currentDoc.slice(0, headerStart)}${block}${currentDoc.slice(afterEnd)}`;
  }
  const header = "### Forge Language Dictionary";
  const next = "\n### Example";
  const headerIndex = currentDoc.indexOf(header);
  const nextIndex = currentDoc.indexOf(next, headerIndex);
  if (headerIndex < 0 || nextIndex < 0) {
    throw new Error("cannot locate Forge Language Dictionary section");
  }
  return `${currentDoc.slice(0, headerIndex)}${block}\n${currentDoc.slice(nextIndex)}`;
}

const nextDoc = replaceDictionaryBlock(doc, generatedDictionary());

if (checkOnly) {
  if (nextDoc !== doc) {
    console.error("FORGE_NATIVE_BYTECODE.md dictionary is out of sync with src/kasm.rs.");
    console.error("Run: node scripts/forge-language-dictionary.mjs");
    process.exit(1);
  }
  console.log("Forge language dictionary is in sync.");
} else {
  fs.writeFileSync(docPath, nextDoc);
  console.log("Updated FORGE_NATIVE_BYTECODE.md Forge Language Dictionary.");
}
