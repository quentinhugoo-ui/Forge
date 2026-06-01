// Banger world-building library — composite SDF primitives that the
// agent / ForgeSlash can compose into landscapes, cities, vehicles,
// etc. Pure TS sugar on top of scenes.ts opcodes — no shader changes,
// no new state. Each function returns an SdfOp[] postfix subprogram
// that can be concatenated and ended with the right number of unions.
//
// Doctrine compact : these helpers are stateless and additive ; deleting
// any one of them costs nothing (no caller outside the agent layer).
// Adding a new biome / building type = ~10 lines here.

import { terrainSurfaceZ, type SdfOp, type Vec3 } from "./scenes.js";

const v3 = (x: number, y: number, z: number): Vec3 => [x, y, z];

export interface TerrainSpec {
  readonly amplitude: number;
  readonly frequency: number;
  readonly groundZ: number;
  readonly octaves: number;
  readonly caveStrength: number;
  readonly overhangStrength: number;
  readonly erosionStrength: number;
  readonly erosion?: TerrainErosionControls;
}

export interface TerrainErosionControls {
  readonly river: number;
  readonly rainfall: number;
  readonly slope: number;
  readonly sediment: number;
  readonly pathWear: number;
}

export interface TerrainErosionSample {
  readonly riverMask: number;
  readonly rainWash: number;
  readonly slopeWear: number;
  readonly sedimentDeposit: number;
  readonly pathWear: number;
  readonly effectiveErosionStrength: number;
  readonly erosionHash: string;
}

export const DEFAULT_TERRAIN_SPEC: TerrainSpec = {
  amplitude: 1.5,
  frequency: 0.08,
  groundZ: -0.4,
  octaves: 4,
  caveStrength: 0.18,
  overhangStrength: 0.16,
  erosionStrength: 0.34,
  erosion: {
    river: 0.46,
    rainfall: 0.64,
    slope: 0.52,
    sediment: 0.28,
    pathWear: 0.58,
  },
};

export const DEFAULT_VEGETATION_SPEC: VegetationSpecies = {
  seed: 7319,
  height: 2.2,
  trunkRadius: 0.105,
  canopyRadius: 0.82,
  branchCount: 6,
  leafCount: 10,
  leafScale: 1.0,
  canopyDensity: 0.74,
  windStrength: 0.38,
  season: 0.32,
};

export const DEFAULT_WEATHER_SPEC: WeatherSpec = {
  timeOfDay: 0.62,
  turbidity: 0.34,
  fogDensity: 0.42,
  fogHeight: 0.62,
  cloudiness: 0.36,
  rain: 0.18,
  snow: 0.0,
  wind: 0.32,
  humidity: 0.58,
  waterLevel: 0,
};

export const DEFAULT_DEFORMATION_RIG: DeformationRigSpec = {
  objectId: "sdf_actor",
  seed: 90217,
  time: 0,
  correctiveStrength: 0.42,
  joints: [
    {
      id: "root_sway",
      partId: "root",
      anchor: v3(0, 0, 0),
      axis: v3(0, 0, 1),
      radius: 3.2,
      bend: 0.18,
      twist: 0.12,
      muscle: 0.05,
      phase: 0.0,
      frequency: 1.0,
    },
    {
      id: "upper_counter",
      partId: "upper",
      anchor: v3(0, 0, 1.4),
      axis: v3(0, 1, 0.35),
      radius: 2.4,
      bend: 0.12,
      twist: -0.18,
      muscle: 0.08,
      phase: 1.7,
      frequency: 1.35,
    },
  ],
};

export const DEFAULT_CHARACTER_SPEC: CharacterSpec = {
  seed: 44191,
  height: 1.72,
  build: 0.48,
  skinTone: v3(0.70, 0.46, 0.34),
  skinFlush: 0.22,
  skinWetness: 0.14,
  poreScale: 0.62,
  hairLength: 0.34,
  hairDensity: 0.68,
  hairMelanin: 0.72,
  clothCoverage: 0.64,
  clothWetness: 0.10,
  clothDirt: 0.18,
  expressionSmile: 0.18,
  expressionBlink: 0.0,
  speechOpen: 0.08,
};

export const DEFAULT_WORLD_STREAM_SPEC: WorldStreamSpec = {
  seed: 59183,
  cellSize: 8,
  radiusCells: 2,
  maxLod: 5,
  visualBudgetMb: 64,
  collisionBudgetMb: 18,
  ioBudgetMbPerSec: 96,
  frameBudgetMs: 2.4,
  targetScreenErrorPx: 1.25,
  prefetchSeconds: 1.2,
  hysteresisCells: 1,
};

export const DEFAULT_RENDER_GRAPH_SPEC: RenderGraphSpec = {
  frameBudgetMs: 16.67,
  vramBudgetMb: 96,
  bandwidthBudgetMb: 420,
  logicalBindlessSlots: 512,
  targetHz: 60,
  viewportHeightPx: 720,
  fovYDeg: 62,
  viewForward: v3(1, 0, -0.18),
  hzbBaseMipPx: 8,
  maxCandidatePages: 4096,
  maxVisiblePages: 2048,
};

export interface TerrainStreamTile {
  readonly tileX: number;
  readonly tileY: number;
  readonly lod: number;
  readonly center: Vec3;
  readonly boundsMin: Vec3;
  readonly boundsMax: Vec3;
  readonly screenErrorPx: number;
  readonly resident: boolean;
  readonly erosionHash: string;
  readonly erosion: TerrainErosionSample;
  readonly terrainCellHash: string;
  readonly visualCacheHash: string;
  readonly collisionCacheHash: string;
  readonly visualResolution: number;
  readonly collisionResolution: number;
}

export interface TerrainStreamManifest {
  readonly schema: "forge.banger.terrain_stream.v1";
  readonly sourceHash: string;
  readonly manifestHash: string;
  readonly streamingSource: Vec3;
  readonly tileSize: number;
  readonly targetScreenErrorPx: number;
  readonly visualResidentCount: number;
  readonly collisionResidentCount: number;
  readonly tiles: readonly TerrainStreamTile[];
}

export interface VegetationSpecies {
  readonly seed: number;
  readonly height: number;
  readonly trunkRadius: number;
  readonly canopyRadius: number;
  readonly branchCount: number;
  readonly leafCount: number;
  readonly leafScale: number;
  readonly canopyDensity: number;
  readonly windStrength: number;
  readonly season: number;
}

export interface VegetationCanopySample {
  readonly position: Vec3;
  readonly radius: number;
  readonly density: number;
  readonly sourceRef: string;
  readonly sampleHash: string;
}

export interface VegetationSurfel {
  readonly position: Vec3;
  readonly normal: Vec3;
  readonly radius: number;
  readonly albedo: Vec3;
  readonly roughness: number;
  readonly sourceRef: string;
  readonly surfelHash: string;
}

export interface VegetationStreamCluster {
  readonly cellX: number;
  readonly cellY: number;
  readonly lod: "near-sdf" | "canopy-density" | "surfel-cloud";
  readonly center: Vec3;
  readonly resident: boolean;
  readonly clusterHash: string;
  readonly cacheHash: string;
}

export interface VegetationClusterManifest {
  readonly schema: "forge.banger.vegetation_cluster.v1";
  readonly speciesHash: string;
  readonly clusterHash: string;
  readonly sourceHash: string;
  readonly canopyDensityHash: string;
  readonly surfelCloudHash: string;
  readonly windFieldHash: string;
  readonly streamHash: string;
  readonly origin: Vec3;
  readonly boundsMin: Vec3;
  readonly boundsMax: Vec3;
  readonly nearOpBudget: number;
  readonly canopySamples: readonly VegetationCanopySample[];
  readonly surfels: readonly VegetationSurfel[];
  readonly streamClusters: readonly VegetationStreamCluster[];
}

export interface WeatherSpec {
  readonly timeOfDay: number;
  readonly turbidity: number;
  readonly fogDensity: number;
  readonly fogHeight: number;
  readonly cloudiness: number;
  readonly rain: number;
  readonly snow: number;
  readonly wind: number;
  readonly humidity: number;
  readonly waterLevel: number | null;
}

export interface WeatherManifest {
  readonly schema: "forge.banger.weather.v1";
  readonly weatherHash: string;
  readonly skyHash: string;
  readonly froxelFogHash: string;
  readonly waterHash: string;
  readonly wetnessHash: string;
  readonly particleHash: string;
  readonly skyEnabled: boolean;
  readonly fogEnabled: boolean;
  readonly waterLevel: number | null;
  readonly materialWetness: number;
  readonly valleyFogDensity: number;
  readonly precipitation: "clear" | "rain" | "snow" | "mixed";
}

export interface NewObjectBounds {
  readonly min: Vec3;
  readonly max: Vec3;
}

export interface NewObjectWebResearchStep {
  readonly schema: "forge.banger.web_research.v1";
  readonly command: "/web_";
  readonly id: string;
  readonly partId: string;
  readonly target: string;
  readonly sources: readonly string[];
  readonly concepts: readonly string[];
  readonly researchHash: string;
}

export interface NewObjectComputeStep {
  readonly schema: "forge.banger.newcompute.prereq.v1";
  readonly command: "/newcompute_";
  readonly id: string;
  readonly partId: string;
  readonly round: "initial" | "refined";
  readonly challenge: string;
  readonly mathModel: string;
  readonly equations: readonly string[];
  readonly researchRefs: readonly string[];
  readonly priorComputeRefs: readonly string[];
  readonly inputs: Record<string, string | number | boolean | null>;
  readonly outputs: Record<string, string | number | boolean | null>;
  readonly validation: readonly string[];
  readonly sources: readonly string[];
  readonly template: string;
  readonly computeHash: string;
  readonly rawResultHash: string;
  readonly proofHash: string;
  readonly llmReview: "usable_for_llm_curation";
  readonly promotion: "promoted_for_llm_curation";
}

export interface NewObjectMathCuration {
  readonly schema: "forge.banger.llm_math_curation.v1";
  readonly partId: string;
  readonly curationOf: readonly string[];
  readonly rejectedDirectInjection: true;
  readonly selectedModel: string;
  readonly curatedParameters: Record<string, string | number | boolean | null>;
  readonly authoringDecision: string;
  readonly validationSummary: readonly string[];
  readonly curationHash: string;
  readonly proofHash: string;
}

export interface NewObjectPart {
  readonly id: string;
  readonly name: string;
  readonly role: string;
  readonly independent: boolean;
  readonly material_region: string;
  readonly sdf_refs: readonly string[];
  readonly curationRefs: readonly string[];
  readonly primitiveRange: { readonly start: number; readonly count: number };
  readonly bounds: NewObjectBounds;
  readonly interfaces: readonly string[];
  readonly selection_handle: string;
  readonly specifics: Record<string, string | number | boolean | null>;
  readonly partHash: string;
}

export interface NewObjectRenderControls {
  readonly skyEnabled: boolean;
  readonly fogEnabled: boolean;
  readonly waterLevel: number | null;
  readonly camera: {
    readonly target: Vec3;
    readonly distance: number;
    readonly azimuth: number;
    readonly elevation: number;
  };
}

export interface DefaultOceanSunsetNewObject {
  readonly schema: "forge.banger.newobject.preview.v1";
  readonly command: "/newobject_";
  readonly objectIntent: "default_ocean_sunset";
  readonly ops: readonly SdfOp[];
  readonly webResearch: readonly NewObjectWebResearchStep[];
  readonly computePlan: readonly NewObjectComputeStep[];
  readonly computePlanHash: string;
  readonly llmCuration: readonly NewObjectMathCuration[];
  readonly mathCurationHash: string;
  readonly objectParts: readonly NewObjectPart[];
  readonly renderControls: NewObjectRenderControls;
  readonly previewHandoff: {
    readonly sourceHash: string;
    readonly mathCurationHash: string;
    readonly evidenceComputePlanHash: string;
    readonly objectPartsHash: string;
    readonly renderHash: string;
  };
  readonly proofHash: string;
}

export interface TemporalAdversarialCase {
  readonly id: string;
  readonly scene: string;
  readonly stressors: readonly string[];
  readonly target: readonly string[];
  readonly caseHash: string;
}

export interface TemporalAdversarialManifest {
  readonly schema: "forge.banger.temporal_adversarial.v1";
  readonly manifestHash: string;
  readonly cases: readonly TemporalAdversarialCase[];
}

export interface DeformationJointSpec {
  readonly id: string;
  readonly partId: string;
  readonly anchor: Vec3;
  readonly axis: Vec3;
  readonly radius: number;
  readonly bend: number;
  readonly twist: number;
  readonly muscle: number;
  readonly phase: number;
  readonly frequency: number;
}

export interface DeformationRigSpec {
  readonly objectId: string;
  readonly seed: number;
  readonly time: number;
  readonly correctiveStrength: number;
  readonly joints: readonly DeformationJointSpec[];
}

export interface DeformationSourceRef {
  readonly opIndex: number;
  readonly objectId: string;
  readonly partId: string;
  readonly sdfNodeId: string;
  readonly parameterPath: string;
  readonly sourceHash: string;
  readonly deformedHash: string;
  readonly maxOffset: number;
}

export interface DeformationManifest {
  readonly schema: "forge.banger.deformation_field.v1";
  readonly rigHash: string;
  readonly sourceHash: string;
  readonly deformedHash: string;
  readonly correctiveHash: string;
  readonly time: number;
  readonly maxOffset: number;
  readonly sourceRefs: readonly DeformationSourceRef[];
}

export interface DeformedSceneResult {
  readonly ops: readonly SdfOp[];
  readonly manifest: DeformationManifest;
}

export interface CharacterSpec {
  readonly seed: number;
  readonly height: number;
  readonly build: number;
  readonly skinTone: Vec3;
  readonly skinFlush: number;
  readonly skinWetness: number;
  readonly poreScale: number;
  readonly hairLength: number;
  readonly hairDensity: number;
  readonly hairMelanin: number;
  readonly clothCoverage: number;
  readonly clothWetness: number;
  readonly clothDirt: number;
  readonly expressionSmile: number;
  readonly expressionBlink: number;
  readonly speechOpen: number;
}

export interface CharacterSkinProfile {
  readonly albedo: Vec3;
  readonly subsurfaceCm: number;
  readonly poreScale: number;
  readonly microNormalStrength: number;
  readonly flush: number;
  readonly wetness: number;
  readonly profileHash: string;
}

export interface CharacterPartRef {
  readonly partId: string;
  readonly sdfNodeId: string;
  readonly parameterPath: string;
  readonly sourceHash: string;
  readonly materialHash: string;
}

export interface HairStrandSample {
  readonly root: Vec3;
  readonly tip: Vec3;
  readonly radius: number;
  readonly sourceRef: string;
  readonly strandHash: string;
}

export interface ClothPatchSample {
  readonly center: Vec3;
  readonly normal: Vec3;
  readonly radius: number;
  readonly wrinkle: number;
  readonly wetness: number;
  readonly dirt: number;
  readonly sourceRef: string;
  readonly patchHash: string;
}

export interface CharacterManifest {
  readonly schema: "forge.banger.character_sdf.v1";
  readonly characterHash: string;
  readonly specHash: string;
  readonly sourceHash: string;
  readonly skinHash: string;
  readonly hairCacheHash: string;
  readonly clothCacheHash: string;
  readonly facialHash: string;
  readonly deformationHash: string;
  readonly correctiveHash: string;
  readonly origin: Vec3;
  readonly boundsMin: Vec3;
  readonly boundsMax: Vec3;
  readonly skin: CharacterSkinProfile;
  readonly partRefs: readonly CharacterPartRef[];
  readonly hairStrands: readonly HairStrandSample[];
  readonly clothPatches: readonly ClothPatchSample[];
  readonly opBudget: number;
}

export interface WorldStreamSpec {
  readonly seed: number;
  readonly cellSize: number;
  readonly radiusCells: number;
  readonly maxLod: number;
  readonly visualBudgetMb: number;
  readonly collisionBudgetMb: number;
  readonly ioBudgetMbPerSec: number;
  readonly frameBudgetMs: number;
  readonly targetScreenErrorPx: number;
  readonly prefetchSeconds: number;
  readonly hysteresisCells: number;
}

export interface WorldStreamBudget {
  readonly visualBudgetMb: number;
  readonly collisionBudgetMb: number;
  readonly ioBudgetMbPerSec: number;
  readonly frameBudgetMs: number;
  readonly visualUsedMb: number;
  readonly collisionUsedMb: number;
  readonly ioUsedMb: number;
  readonly frameUsedMs: number;
  readonly budgetOk: boolean;
}

export interface WorldStreamCell {
  readonly cellX: number;
  readonly cellY: number;
  readonly lod: number;
  readonly quality: "near-sdf" | "fieldlet-cache" | "surfel-far" | "culled";
  readonly center: Vec3;
  readonly predictedCenterDistance: number;
  readonly screenErrorPx: number;
  readonly resident: boolean;
  readonly prefetch: boolean;
  readonly priority: number;
  readonly visualMb: number;
  readonly collisionMb: number;
  readonly ioMb: number;
  readonly frameMs: number;
  readonly cellHash: string;
  readonly sourceHash: string;
  readonly terrainCellHash: string;
  readonly visualCacheHash: string;
  readonly collisionCacheHash: string;
  readonly vegetationCacheHash: string;
}

export interface WorldStreamReplayEvent {
  readonly step: number;
  readonly action: "load" | "prefetch" | "keep" | "evict";
  readonly cellHash: string;
  readonly reason: string;
  readonly eventHash: string;
}

export interface WorldStreamManifest {
  readonly schema: "forge.banger.world_stream.v1";
  readonly streamHash: string;
  readonly sourceHash: string;
  readonly replayHash: string;
  readonly camera: Vec3;
  readonly velocity: Vec3;
  readonly predictedSource: Vec3;
  readonly cellSize: number;
  readonly targetScreenErrorPx: number;
  readonly maxScreenErrorPx: number;
  readonly popRiskPx: number;
  readonly residentCount: number;
  readonly prefetchCount: number;
  readonly evictCount: number;
  readonly budget: WorldStreamBudget;
  readonly cells: readonly WorldStreamCell[];
  readonly replay: readonly WorldStreamReplayEvent[];
}

export interface RenderGraphSpec {
  readonly frameBudgetMs: number;
  readonly vramBudgetMb: number;
  readonly bandwidthBudgetMb: number;
  readonly logicalBindlessSlots: number;
  readonly targetHz: number;
  readonly viewportHeightPx: number;
  readonly fovYDeg: number;
  readonly viewForward: Vec3;
  readonly hzbBaseMipPx: number;
  readonly maxCandidatePages: number;
  readonly maxVisiblePages: number;
}

export interface RenderGraphRuntimePass {
  readonly id?: string;
  readonly cpuMs?: number;
  readonly dispatches?: number;
  readonly workgroups?: number;
  readonly active?: boolean;
  readonly cacheHash?: string;
}

export interface RenderGraphRuntimeStats {
  readonly frameHash?: string;
  readonly sourceHash?: string;
  readonly cpuFrameMs?: number;
  readonly frameCacheHitRatio?: number;
  readonly width?: number;
  readonly height?: number;
  readonly bindGroupBindings?: number;
  readonly approxVramBytes?: number;
  readonly shadowPages?: number;
  readonly splats?: number;
  readonly hasHistory?: boolean;
  readonly passes?: readonly RenderGraphRuntimePass[];
}

export interface RenderGraphResourceEntry {
  readonly slot: number;
  readonly role: "sdf-source" | "page-cache" | "material-table" | "shadow-pages" | "radiance-probes" | "history" | "output";
  readonly cellHash: string;
  readonly sourceHash: string;
  readonly cacheHash: string;
  readonly bytes: number;
  readonly transient: boolean;
  readonly lifetime: readonly string[];
  readonly entryHash: string;
}

export interface RenderGraphResourceTable {
  readonly schema: "forge.banger.logical_resource_table.v1";
  readonly mode: "webgpu-storage-table-now_native-bindless-later";
  readonly slotBudget: number;
  readonly usedSlots: number;
  readonly tableBudgetOk: boolean;
  readonly tableHash: string;
  readonly entries: readonly RenderGraphResourceEntry[];
}

export interface RenderGraphCullingPage {
  readonly pageIndex: number;
  readonly cellHash: string;
  readonly sourceHash: string;
  readonly pageHash: string;
  readonly quality: WorldStreamCell["quality"];
  readonly boundsMin: Vec3;
  readonly boundsMax: Vec3;
  readonly screenErrorPx: number;
  readonly hzbMip: number;
  readonly frustumVisible: boolean;
  readonly occlusionVisible: boolean;
  readonly visible: boolean;
  readonly compactedIndex: number;
  readonly indirectGroup: number;
  readonly reason: "visible" | "stream-culled" | "frustum" | "hzb-occluded" | "candidate-overflow" | "visible-overflow";
  readonly recordHash: string;
}

export interface RenderGraphIndirectArgs {
  readonly schema: "forge.banger.indirect_args.v1";
  readonly dispatchWorkgroups: Vec3;
  readonly drawIndexedIndirectCount: number;
  readonly candidateBytes: number;
  readonly visibleBytes: number;
  readonly argsHash: string;
}

export interface RenderGraphCullingPlan {
  readonly schema: "forge.banger.gpu_culling_plan.v1";
  readonly mode: "page-cluster-compaction";
  readonly candidateCells: number;
  readonly residentCells: number;
  readonly culledCells: number;
  readonly candidatePageCount: number;
  readonly streamCulledPageCount: number;
  readonly frustumCulledPageCount: number;
  readonly hzbCulledPageCount: number;
  readonly overflowCulledPageCount: number;
  readonly visiblePageCount: number;
  readonly indirectDispatches: number;
  readonly indirectArgs: RenderGraphIndirectArgs;
  readonly occlusionPyramidHash: string;
  readonly visiblePageListHash: string;
  readonly compactionHash: string;
  readonly pages: readonly RenderGraphCullingPage[];
  readonly cullingHash: string;
}

export interface RenderGraphCompressionPlan {
  readonly schema: "forge.banger.render_compression.v1";
  readonly rawBytes: number;
  readonly compressedBytes: number;
  readonly compressionRatio: number;
  readonly sdfPageBytes: number;
  readonly surfelBytes: number;
  readonly radianceBytes: number;
  readonly materialBytes: number;
  readonly compressionHash: string;
}

export interface RenderGraphPass {
  readonly id: string;
  readonly kind: "stream" | "cull" | "cache" | "shadow" | "gi" | "main" | "post";
  readonly reads: readonly string[];
  readonly writes: readonly string[];
  readonly dispatch: "cpu-manifest" | "gpu-compute" | "gpu-indirect-plan" | "copy-present";
  readonly workgroups: number;
  readonly estimatedMs: number;
  readonly measuredMs: number;
  readonly passHash: string;
}

export interface RenderGraphProfile {
  readonly schema: "forge.banger.render_graph_profile.v1";
  readonly frameBudgetMs: number;
  readonly estimatedFrameMs: number;
  readonly measuredFrameMs: number;
  readonly vramBudgetMb: number;
  readonly vramUsedMb: number;
  readonly bandwidthBudgetMb: number;
  readonly bandwidthUsedMb: number;
  readonly frameCacheHitRatio: number;
  readonly passCount: number;
  readonly budgetOk: boolean;
  readonly profileHash: string;
}

export interface RenderGraphManifest {
  readonly schema: "forge.banger.render_graph.v1";
  readonly graphHash: string;
  readonly streamHash: string;
  readonly sourceHash: string;
  readonly rendererHash: string;
  readonly passesHash: string;
  readonly resourcesHash: string;
  readonly cullingHash: string;
  readonly compressionHash: string;
  readonly profileHash: string;
  readonly stream: WorldStreamManifest;
  readonly passes: readonly RenderGraphPass[];
  readonly resources: RenderGraphResourceTable;
  readonly culling: RenderGraphCullingPlan;
  readonly compression: RenderGraphCompressionPlan;
  readonly profile: RenderGraphProfile;
  readonly doctrine: "SDF-authoritative render graph; GPU resources are transient hashable caches";
}

/** Mulberry32 — tiny deterministic RNG so worlds are reproducible
 *  from a single seed (the agent can store / re-emit the seed). */
export function seededRandom(seed: number): () => number {
  let s = (seed | 0) || 1;
  return () => {
    s = (s + 0x6D2B79F5) | 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function stableHash(parts: readonly (string | number | boolean)[], namespace = "terrain"): string {
  let h = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const text = parts.map((part) => typeof part === "number" ? part.toFixed(5) : String(part)).join("|");
  for (let i = 0; i < text.length; i += 1) {
    h ^= BigInt(text.charCodeAt(i));
    h = (h * prime) & 0xffffffffffffffffn;
  }
  return `kasm://${namespace}/${h.toString(16).padStart(16, "0")}`;
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function finiteNumber(value: unknown, fallback: number): number {
  const n = Number(value);
  return Number.isFinite(n) ? n : fallback;
}

function erosionControls(spec: TerrainSpec): TerrainErosionControls {
  return {
    river: clamp01(spec.erosion?.river ?? 0),
    rainfall: clamp01(spec.erosion?.rainfall ?? 0),
    slope: clamp01(spec.erosion?.slope ?? 0),
    sediment: clamp01(spec.erosion?.sediment ?? 0),
    pathWear: clamp01(spec.erosion?.pathWear ?? 0),
  };
}

export function terrainErosionStrength(spec: TerrainSpec = DEFAULT_TERRAIN_SPEC): number {
  const e = erosionControls(spec);
  return clamp01(
    spec.erosionStrength
    + e.river * 0.18
    + e.rainfall * 0.12
    + e.slope * 0.10
    + e.pathWear * 0.08
    - e.sediment * 0.09,
  );
}

function terrainSpecHash(spec: TerrainSpec): string {
  const e = erosionControls(spec);
  return stableHash([
    "terrain_spec",
    spec.amplitude,
    spec.frequency,
    spec.groundZ,
    spec.octaves,
    spec.caveStrength,
    spec.overhangStrength,
    spec.erosionStrength,
    e.river,
    e.rainfall,
    e.slope,
    e.sediment,
    e.pathWear,
    terrainErosionStrength(spec),
  ]);
}

function vegetationSpec(spec: Partial<VegetationSpecies> = {}): VegetationSpecies {
  return {
    seed: Math.max(1, Number(spec.seed ?? DEFAULT_VEGETATION_SPEC.seed) | 0),
    height: Math.max(0.35, Math.min(12, Number(spec.height ?? DEFAULT_VEGETATION_SPEC.height))),
    trunkRadius: Math.max(0.025, Math.min(0.6, Number(spec.trunkRadius ?? DEFAULT_VEGETATION_SPEC.trunkRadius))),
    canopyRadius: Math.max(0.12, Math.min(6, Number(spec.canopyRadius ?? DEFAULT_VEGETATION_SPEC.canopyRadius))),
    branchCount: Math.max(0, Math.min(14, Number(spec.branchCount ?? DEFAULT_VEGETATION_SPEC.branchCount) | 0)),
    leafCount: Math.max(0, Math.min(32, Number(spec.leafCount ?? DEFAULT_VEGETATION_SPEC.leafCount) | 0)),
    leafScale: Math.max(0.25, Math.min(3, Number(spec.leafScale ?? DEFAULT_VEGETATION_SPEC.leafScale))),
    canopyDensity: clamp01(Number(spec.canopyDensity ?? DEFAULT_VEGETATION_SPEC.canopyDensity)),
    windStrength: clamp01(Number(spec.windStrength ?? DEFAULT_VEGETATION_SPEC.windStrength)),
    season: clamp01(Number(spec.season ?? DEFAULT_VEGETATION_SPEC.season)),
  };
}

export function vegetationSpecHash(spec: Partial<VegetationSpecies> = {}): string {
  const s = vegetationSpec(spec);
  return stableHash([
    "vegetation_species",
    s.seed,
    s.height,
    s.trunkRadius,
    s.canopyRadius,
    s.branchCount,
    s.leafCount,
    s.leafScale,
    s.canopyDensity,
    s.windStrength,
    s.season,
  ], "vegetation");
}

function weatherSpec(spec: Partial<WeatherSpec> = {}): WeatherSpec {
  const water = spec.waterLevel === null
    ? null
    : Number.isFinite(Number(spec.waterLevel ?? DEFAULT_WEATHER_SPEC.waterLevel))
      ? Number(spec.waterLevel ?? DEFAULT_WEATHER_SPEC.waterLevel)
      : null;
  return {
    timeOfDay: clamp01(Number(spec.timeOfDay ?? DEFAULT_WEATHER_SPEC.timeOfDay)),
    turbidity: clamp01(Number(spec.turbidity ?? DEFAULT_WEATHER_SPEC.turbidity)),
    fogDensity: clamp01(Number(spec.fogDensity ?? DEFAULT_WEATHER_SPEC.fogDensity)),
    fogHeight: clamp01(Number(spec.fogHeight ?? DEFAULT_WEATHER_SPEC.fogHeight)),
    cloudiness: clamp01(Number(spec.cloudiness ?? DEFAULT_WEATHER_SPEC.cloudiness)),
    rain: clamp01(Number(spec.rain ?? DEFAULT_WEATHER_SPEC.rain)),
    snow: clamp01(Number(spec.snow ?? DEFAULT_WEATHER_SPEC.snow)),
    wind: clamp01(Number(spec.wind ?? DEFAULT_WEATHER_SPEC.wind)),
    humidity: clamp01(Number(spec.humidity ?? DEFAULT_WEATHER_SPEC.humidity)),
    waterLevel: water,
  };
}

export function weatherManifest(spec: Partial<WeatherSpec> = {}): WeatherManifest {
  const w = weatherSpec(spec);
  const precipitation: WeatherManifest["precipitation"] = w.rain > 0.05 && w.snow > 0.05
    ? "mixed"
    : w.rain > 0.05
      ? "rain"
      : w.snow > 0.05
        ? "snow"
        : "clear";
  const materialWetness = clamp01(w.humidity * 0.45 + w.rain * 0.55 + (w.waterLevel === null ? 0 : 0.12));
  const valleyFogDensity = clamp01(w.fogDensity * (0.55 + w.humidity * 0.45));
  const skyHash = stableHash(["sky", w.timeOfDay, w.turbidity, w.cloudiness, w.humidity], "weather");
  const froxelFogHash = stableHash(["froxel_fog", w.fogDensity, w.fogHeight, valleyFogDensity, w.wind], "weather");
  const waterHash = stableHash(["water", w.waterLevel === null ? -9999 : w.waterLevel, w.wind, w.rain, w.humidity], "weather");
  const wetnessHash = stableHash(["wetness", materialWetness, w.rain, w.snow, w.humidity], "weather");
  const particleHash = stableHash(["particles", precipitation, w.rain, w.snow, w.wind, w.cloudiness], "weather");
  const weatherHash = stableHash(["weather", skyHash, froxelFogHash, waterHash, wetnessHash, particleHash], "weather");
  return {
    schema: "forge.banger.weather.v1",
    weatherHash,
    skyHash,
    froxelFogHash,
    waterHash,
    wetnessHash,
    particleHash,
    skyEnabled: true,
    fogEnabled: w.fogDensity > 0.01 || w.humidity > 0.4,
    waterLevel: w.waterLevel,
    materialWetness,
    valleyFogDensity,
    precipitation,
  };
}

function newObjectPart(
  id: string,
  name: string,
  role: string,
  bounds: NewObjectBounds,
  specifics: Record<string, string | number | boolean | null>,
  curationRefs: readonly string[] = [],
): NewObjectPart {
  const partHash = stableHash([
    "newobject_part",
    id,
    name,
    role,
    bounds.min[0],
    bounds.min[1],
    bounds.min[2],
    bounds.max[0],
    bounds.max[1],
    bounds.max[2],
    ...curationRefs,
    ...Object.entries(specifics).flatMap(([key, value]) => [key, String(value)]),
  ], "newobject");
  return {
    id,
    name,
    role,
    independent: true,
    material_region: id,
    sdf_refs: [`newobject.default_ocean_sunset.${id}`],
    curationRefs,
    primitiveRange: { start: 0, count: 0 },
    bounds,
    interfaces: ["render-cache-only", "sdf-authoritative-source-part", "llm-curated-math"],
    selection_handle: `${id}_aabb`,
    specifics,
    partHash,
  };
}

function newObjectWebResearchStep(
  id: string,
  partId: string,
  target: string,
  sources: readonly string[],
  concepts: readonly string[],
): NewObjectWebResearchStep {
  const researchHash = stableHash(["web_research", id, partId, target, ...sources, ...concepts], "web");
  return {
    schema: "forge.banger.web_research.v1",
    command: "/web_",
    id,
    partId,
    target,
    sources,
    concepts,
    researchHash,
  };
}

function newObjectComputeStep(
  id: string,
  partId: string,
  challenge: string,
  mathModel: string,
  equations: readonly string[],
  inputs: Record<string, string | number | boolean | null>,
  outputs: Record<string, string | number | boolean | null>,
  validation: readonly string[],
  sources: readonly string[],
  researchRefs: readonly string[] = [],
  priorComputeRefs: readonly string[] = [],
  round: "initial" | "refined" = "initial",
): NewObjectComputeStep {
  const payload = [
    "newobject_default_ocean_sunset",
    id,
    partId,
    round,
    challenge,
    mathModel,
    ...equations,
    ...researchRefs,
    ...priorComputeRefs,
    ...Object.entries(inputs).flatMap(([key, value]) => [`input.${key}`, String(value)]),
    ...Object.entries(outputs).flatMap(([key, value]) => [`output.${key}`, String(value)]),
    ...validation,
    ...sources,
  ];
  const computeHash = stableHash(["newcompute", ...payload], "newcompute");
  const rawResultHash = stableHash(["newcompute_raw_math_data", computeHash, ...Object.entries(outputs).flatMap(([key, value]) => [key, String(value)])], "newcompute");
  const proofHash = stableHash(["newcompute_proof", computeHash, rawResultHash, partId, ...validation], "newcompute");
  const template = [
    "/newcompute_",
    "version=3",
    "mode=llm_filled_lab_grade_template",
    `part=${partId}`,
    `round=${round}`,
    `challenge=${challenge}`,
    `math_model=${mathModel}`,
    `research_refs=${researchRefs.join(",")}`,
    `prior_compute_refs=${priorComputeRefs.join(",")}`,
    `equations=${equations.join(" | ")}`,
    `oracles=${validation.join(" | ")}`,
    "result_contract=valuable_math_decision_data_not_sdf_code",
    `output_contract=${Object.entries(outputs).map(([key, value]) => `${key}:${value}`).join(",")}`,
    `proof_hash=${proofHash}`,
  ].join("\n");
  return {
    schema: "forge.banger.newcompute.prereq.v1",
    command: "/newcompute_",
    id,
    partId,
    round,
    challenge,
    mathModel,
    equations,
    researchRefs,
    priorComputeRefs,
    inputs,
    outputs,
    validation,
    sources,
    template,
    computeHash,
    rawResultHash,
    proofHash,
    llmReview: "usable_for_llm_curation",
    promotion: "promoted_for_llm_curation",
  };
}

function newObjectMathCuration(
  partId: string,
  computes: readonly NewObjectComputeStep[],
  selectedModel: string,
  curatedParameters: Record<string, string | number | boolean | null>,
  authoringDecision: string,
  validationSummary: readonly string[],
): NewObjectMathCuration {
  const curationOf = computes.map((step) => step.proofHash);
  const curationHash = stableHash([
    "llm_math_curation",
    partId,
    selectedModel,
    ...curationOf,
    ...Object.entries(curatedParameters).flatMap(([key, value]) => [key, String(value)]),
    authoringDecision,
    ...validationSummary,
  ], "newobject");
  const proofHash = stableHash(["llm_math_curation_proof", curationHash, partId, ...curationOf], "newobject");
  return {
    schema: "forge.banger.llm_math_curation.v1",
    partId,
    curationOf,
    rejectedDirectInjection: true,
    selectedModel,
    curatedParameters,
    authoringDecision,
    validationSummary,
    curationHash,
    proofHash,
  };
}

export function defaultOceanSunsetNewObject(): DefaultOceanSunsetNewObject {
  const webResearch = [
    newObjectWebResearchStep(
      "web_ciel_scattering_sota",
      "ciel",
      "current production sky atmosphere math for blue sky, aerial perspective and sunset",
      [
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
        "https://ebruneton.github.io/precomputed_atmospheric_scattering/",
        "https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint.pdf",
      ],
      ["Rayleigh phase", "Mie phase", "spectral-to-RGB fit", "aerial perspective", "sunset turbidity"],
    ),
    newObjectWebResearchStep(
      "web_ocean_wave_sota",
      "ocean",
      "low-cost real-time ocean motion with physically bounded wave spectra",
      [
        "https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models",
        "https://dev.epicgames.com/documentation/unreal-engine/simulating-waves-using-the-water-waves-asset-in-unreal-engine",
        "https://jtessen.people.clemson.edu/reports/papers_files/coursenotes2004.pdf",
      ],
      ["Gerstner steepness bound", "dispersion relation", "JONSWAP spectrum", "current advection", "normal-map chop"],
    ),
    newObjectWebResearchStep(
      "web_soleil_radiance_sota",
      "soleil",
      "sun disk radiance and sunset color coupled to sky and ocean highlights",
      [
        "https://pubmed.ncbi.nlm.nih.gov/24807990/",
        "https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint.pdf",
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
      ],
      ["solar radiance fit", "blackbody CCT approximation", "finite disk mask", "Fresnel glint", "shared sun direction"],
    ),
    newObjectWebResearchStep(
      "web_brume_volume_sota",
      "brume",
      "horizon mist and height fog with stable real-time extinction",
      [
        "https://dev.epicgames.com/documentation/unreal-engine/volumetric-fog-in-unreal-engine",
        "https://www.ea.com/frostbite/news/physically-based-unified-volumetric-rendering-in-frostbite",
        "https://publications.ri.cmu.edu/a-practical-analytic-single-scattering-model-for-real-time-rendering",
      ],
      ["Beer-Lambert extinction", "exponential height density", "Henyey-Greenstein phase", "froxel integration", "aerial perspective"],
    ),
    newObjectWebResearchStep(
      "web_ile_terrain_sota",
      "ile",
      "distant island silhouette: mountainous relief and forest cover that read at horizon distance",
      [
        "https://iquilezles.org/articles/fbm/",
        "https://iquilezles.org/articles/morenoise/",
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
      ],
      ["azimuth ridge profile", "rotated value-noise relief", "altitude vegetation banding", "aerial perspective desaturation", "analytic distant terrain"],
    ),
  ] as const;
  const researchRefs = (partId: string) => webResearch
    .filter((step) => step.partId === partId)
    .map((step) => step.researchHash);
  const computePlan = [
    newObjectComputeStep(
      "newcompute_ciel_scattering",
      "ciel",
      "maximize believable blue-sky-to-sunset gradient with constant-time shader evaluation",
      "Rayleigh/Mie single-scattering fit with Hosek-Wilkie sunset fallback",
      [
        "beta_R(lambda) proportional lambda^-4",
        "P_R(mu)=3/(16*pi)*(1+mu^2)",
        "P_M(mu,g)=3/(8*pi)*((1-g^2)*(1+mu^2))/((2+g^2)*(1+g^2-2*g*mu)^(3/2))",
        "L_sky approx integral(T_view * (beta_R*P_R + beta_M*P_M) * E_sun ds)",
      ],
      { timeOfDay: 0.78, turbidity: 0.34, humidity: 0.58, shaderBudget: "single_env_lookup" },
      { rayleighScaleKm: 8.0, mieScaleKm: 1.2, mieG: 0.76, zenithBlue: 0.72, horizonBlue: 0.90 },
      [
        "dimensional: scattering coefficients remain inverse-length",
        "metamorphic: higher turbidity warms and desaturates the horizon",
        "limit: sun below horizon removes disk but preserves aerial perspective",
      ],
      [
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
        "https://ebruneton.github.io/precomputed_atmospheric_scattering/",
        "https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint.pdf",
      ],
      researchRefs("ciel"),
    ),
    newObjectComputeStep(
      "newcompute_ocean_spectrum",
      "ocean",
      "produce natural low-cost ocean motion with stable current, reflection and no mesh authority",
      "directional Gerstner wave stack seeded from compact JONSWAP/dispersion constraints",
      [
        "omega_i=sqrt(g*k_i*tanh(k_i*h))",
        "eta(x,t)=sum_i A_i*sin(dot(k_i,x)-omega_i*t+phi_i)",
        "Q_i<=1/(k_i*A_i*N) to prevent Gerstner loop self-intersection",
        "current advection x'=x-v_current*t",
      ],
      { gravity: 9.80665, waterDepthM: 80, swellMeters: 0.55, currentX: 0.6, currentY: 0.35 },
      { waveCount: 3, dominantWavelengthM: 9.5, secondaryWavelengthM: 4.7, chopAmplitudeM: 0.11, maxSteepness: 0.72 },
      [
        "dimensional: dispersion output omega is radians per second",
        "stability: steepness bound forbids overturned crests",
        "metamorphic: increasing current translates phase without changing wave energy",
      ],
      [
        "https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models",
        "https://dev.epicgames.com/documentation/unreal-engine/simulating-waves-using-the-water-waves-asset-in-unreal-engine",
        "https://jtessen.people.clemson.edu/reports/papers_files/coursenotes2004.pdf",
      ],
      researchRefs("ocean"),
    ),
    newObjectComputeStep(
      "newcompute_soleil_radiance",
      "soleil",
      "place a low horizon solar disk that matches sky scattering and ocean glints",
      "Hosek-Wilkie solar radiance with blackbody CCT approximation for sunset warmth",
      [
        "sunDir=normalize(vec3(0.82,0.18,0.16))",
        "L_lambda(T)=c1/(lambda^5*(exp(c2/(lambda*T))-1))",
        "diskMask=pow(max(dot(rd,sunDir),0),900)",
        "specularGlint proportional FresnelSchlick(dot(view,half),F0)",
      ],
      { colorTemperatureK: 3400, angularRadiusDeg: 0.266, horizonElevation: 0.16 },
      { diskIntensity: 7.0, mieHaloIntensity: 0.42, warmR: 1.0, warmG: 0.55, warmB: 0.28 },
      [
        "color: lower CCT warms the disk without changing source direction",
        "coupling: sunDir is shared by sky, shadow and probe passes",
        "limit: angular disk stays finite and does not become a geometry source",
      ],
      [
        "https://pubmed.ncbi.nlm.nih.gov/24807990/",
        "https://cgg.mff.cuni.cz/projects/SkylightModelling/HosekWilkie_SkylightModel_SIGGRAPH2012_Preprint.pdf",
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
      ],
      researchRefs("soleil"),
    ),
    newObjectComputeStep(
      "newcompute_brume_extinction",
      "brume",
      "add light horizon mist that supports aerial perspective without hiding the SDF source",
      "Beer-Lambert exponential height fog with Henyey-Greenstein forward scattering",
      [
        "rho(z)=rho0*exp(-(z-z0)/H)",
        "T(s)=exp(-integral_0^s sigma_t*rho(x(t)) dt)",
        "P_HG(mu,g)=(1-g^2)/(4*pi*(1+g^2-2*g*mu)^(3/2))",
        "L_fog=integral(T_view*sigma_s*rho*P_HG*L_sun ds)",
      ],
      { density: 0.26, heightFalloff: 0.22, humidity: 0.58, horizonBlend: 0.58 },
      { extinction: 0.026, scatteringAlbedo: 0.82, anisotropyG: 0.22, froxelReady: true },
      [
        "energy: scatteringAlbedo remains in [0,1]",
        "metamorphic: density zero gives transparent air",
        "stability: exponential height integral is monotone with path length",
      ],
      [
        "https://dev.epicgames.com/documentation/unreal-engine/volumetric-fog-in-unreal-engine",
        "https://www.ea.com/frostbite/news/physically-based-unified-volumetric-rendering-in-frostbite",
        "https://publications.ri.cmu.edu/a-practical-analytic-single-scattering-model-for-real-time-rendering",
      ],
      researchRefs("brume"),
    ),
    newObjectComputeStep(
      "newcompute_ile_terrain",
      "ile",
      "place a distant mountainous island under the sun with believable relief and vegetation at near-zero render cost",
      "azimuth ridge profile from value-noise fbm with altitude-banded vegetation and aerial perspective",
      [
        "ridge(theta)=env(theta)*sum_k a_k*(0.5+0.5*sin(f_k*theta+phi_k))",
        "env(theta)=1-smoothstep(w0,w1,|theta-theta_sun|)",
        "veg(h)=mix(rock,forest,smoothstep(h0,h1,h))*fbm(theta,h)",
        "C_view=mix(C_island,C_haze,aerial), aerial in [0,1]",
      ],
      { azimuthHalfWidthRad: 0.32, peakElevationRad: 0.069, treeLineFrac: 0.62, aerial: 0.5 },
      { peaks: 3, vegetationAlbedoG: 0.18, rockAlbedo: 0.25, hazeBlend: 0.5 },
      [
        "limit: island stays a background silhouette, never a marched SDF source in the hot path",
        "metamorphic: widening azimuthHalfWidth grows the island without moving its centre",
        "coupling: centre azimuth is locked to the shared sun direction",
      ],
      [
        "https://iquilezles.org/articles/fbm/",
        "https://iquilezles.org/articles/morenoise/",
        "https://dev.epicgames.com/documentation/en-us/unreal-engine/sky-atmosphere",
      ],
      researchRefs("ile"),
    ),
  ] as const;
  const computeEvidence = (partId: string) => computePlan.filter((step) => step.partId === partId);
  const llmCuration = [
    newObjectMathCuration(
      "ciel",
      computeEvidence("ciel"),
      "Rayleigh/Mie sky fit curated from compute evidence, not copied as SDF code",
      { timeOfDay: 0.78, turbidity: 0.34, cloudiness: 0.16, rayleighScaleKm: 8.0, mieScaleKm: 1.2 },
      "Author /newobject_ part 'ciel' as an atmospheric source part; renderer may derive a transient sky shader cache from these curated parameters.",
      ["accepted scattering ranges", "rejected direct LUT injection", "kept source SDF-authoritative and cache-only"],
    ),
    newObjectMathCuration(
      "ocean",
      computeEvidence("ocean"),
      "Bounded Gerstner/JONSWAP wave decision data curated for a compact ocean source part",
      { waterLevel: 0, swellMeters: 0.55, currentX: 0.6, currentY: 0.35, waveCount: 3, maxSteepness: 0.72 },
      "Author /newobject_ part 'ocean' as analytic water intent; motion remains a transient shader cache and not authored mesh geometry.",
      ["accepted steepness bound", "rejected raw wave samples as geometry", "selected low-current sunset sea state"],
    ),
    newObjectMathCuration(
      "soleil",
      computeEvidence("soleil"),
      "Solar disk and glint parameters curated from radiance evidence",
      { directionX: 0.82, directionY: 0.18, directionZ: 0.16, colorTemperatureK: 3400, diskIntensity: 7.0 },
      "Author /newobject_ part 'soleil' as a selectable horizon light source; shader passes share the curated direction.",
      ["accepted low horizon direction", "rejected disk mesh authority", "kept radiance as lighting decision data"],
    ),
    newObjectMathCuration(
      "brume",
      computeEvidence("brume"),
      "Beer-Lambert height fog parameters curated for light horizon mist",
      { density: 0.26, heightFalloff: 0.22, extinction: 0.026, scatteringAlbedo: 0.82, anisotropyG: 0.22 },
      "Author /newobject_ part 'brume' as a volumetric source part; fog/froxel data are reconstructible preview caches.",
      ["accepted monotone extinction", "rejected opaque fog wall", "kept density as curated authoring parameter"],
    ),
    newObjectMathCuration(
      "ile",
      computeEvidence("ile"),
      "Azimuth ridge profile with altitude vegetation banding curated for a distant island under the sun",
      { azimuthHalfWidthRad: 0.32, peakElevationRad: 0.069, treeLineFrac: 0.62, peaks: 3, hazeBlend: 0.5 },
      "Author /newobject_ part 'ile' as a distant background silhouette source; relief and vegetation stay a render cache, not marched geometry.",
      ["accepted horizon-only silhouette", "rejected full terrain SDF in the march hot path", "kept centre azimuth locked to the sun"],
    ),
  ] as const;
  const curationRefs = (partId: string) => llmCuration
    .filter((step) => step.partId === partId)
    .map((step) => step.proofHash);
  const objectParts = [
    newObjectPart(
      "ciel",
      "Ciel bleu",
      "atmospheric sky dome source object; shader cache derives Rayleigh/Mie-like scattering from this part",
      { min: v3(-220, -220, -2), max: v3(220, 220, 160) },
      { timeOfDay: 0.78, turbidity: 0.34, cloudiness: 0.16, skyEnabled: true },
      curationRefs("ciel"),
    ),
    newObjectPart(
      "ocean",
      "Ocean anime",
      "infinite analytic water surface with natural current; rendered as a transient wave/reflection cache",
      { min: v3(-220, -220, -0.18), max: v3(220, 220, 0.42) },
      { waterLevel: 0, swellMeters: 0.55, currentX: 0.6, currentY: 0.35, foam: 0.0 },
      curationRefs("ocean"),
    ),
    newObjectPart(
      "soleil",
      "Soleil couchant",
      "low horizon sun disk and warm glint source for ocean highlights",
      { min: v3(96, 18, 10), max: v3(138, 48, 38) },
      { directionX: 0.82, directionY: 0.18, directionZ: 0.16, colorTemperatureK: 3400, horizonDisk: true },
      curationRefs("soleil"),
    ),
    newObjectPart(
      "brume",
      "Brume legere",
      "height fog and aerial perspective over the water horizon",
      { min: v3(-180, -180, -1), max: v3(180, 180, 38) },
      { density: 0.26, heightFalloff: 0.22, horizonBlend: 0.58, fogEnabled: true },
      curationRefs("brume"),
    ),
    newObjectPart(
      "ile",
      "Ile lointaine",
      "distant mountainous island under the sun: forested relief silhouette with aerial perspective, rendered as a background cache",
      { min: v3(360, 40, 0), max: v3(470, 150, 26) },
      { azimuthHalfWidthRad: 0.32, peakElevationRad: 0.069, treeLineFrac: 0.62, peaks: 3, hazeBlend: 0.5, vegetation: true },
      curationRefs("ile"),
    ),
  ] as const;
  const renderControls: NewObjectRenderControls = {
    skyEnabled: true,
    fogEnabled: true,
    waterLevel: 0,
    camera: {
      target: v3(18, 4, 0.2),
      distance: 30,
      azimuth: -2.78,
      elevation: 0.08,
    },
  };
  const computePlanHash = stableHash(["default_ocean_sunset_compute_plan", ...computePlan.map((step) => step.proofHash)], "newcompute");
  const mathCurationHash = stableHash(["default_ocean_sunset_math_curation", ...llmCuration.map((step) => step.proofHash)], "newobject");
  const sourceHash = stableHash(["default_ocean_sunset_source", mathCurationHash, ...objectParts.map((part) => part.partHash)], "newobject");
  const objectPartsHash = stableHash(["default_ocean_sunset_parts", ...objectParts.map((part) => part.partHash)], "newobject");
  const renderHash = stableHash([
    "default_ocean_sunset_render",
    renderControls.skyEnabled,
    renderControls.fogEnabled,
    renderControls.waterLevel ?? -9999,
    renderControls.camera.target[0],
    renderControls.camera.target[1],
    renderControls.camera.target[2],
    renderControls.camera.distance,
    renderControls.camera.azimuth,
    renderControls.camera.elevation,
  ], "newobject");
  const proofHash = stableHash(["default_ocean_sunset_newobject", sourceHash, mathCurationHash, objectPartsHash, renderHash], "newobject");
  return {
    schema: "forge.banger.newobject.preview.v1",
    command: "/newobject_",
    objectIntent: "default_ocean_sunset",
    ops: [],
    webResearch,
    computePlan,
    computePlanHash,
    llmCuration,
    mathCurationHash,
    objectParts,
    renderControls,
    previewHandoff: {
      sourceHash,
      mathCurationHash,
      evidenceComputePlanHash: computePlanHash,
      objectPartsHash,
      renderHash,
    },
    proofHash,
  };
}

export function temporalAdversarialManifest(): TemporalAdversarialManifest {
  const specs = [
    {
      id: "forest_rain_dusk",
      scene: "dense SDF foliage + rain wetness + dusk fog",
      stressors: ["alpha_foliage_crawl", "history_rejection", "specular_wetness", "camera_pan"],
      target: ["motion_vectors", "reactive_mask", "history_clamp"],
    },
    {
      id: "thin_leaf_orbit",
      scene: "thin implicit leaves against bright sky",
      stressors: ["thin_geometry", "subpixel_jitter", "disocclusion"],
      target: ["sdf_depth_reprojection", "clamped_history"],
    },
    {
      id: "water_reflection_walk",
      scene: "SDF terrain + reflective water plane + moving camera",
      stressors: ["reflection_instability", "reactive_surface", "aerial_fog"],
      target: ["reactive_mask", "luma_clip", "depth_rejection"],
    },
    {
      id: "chrome_splats_crossing",
      scene: "metallic SDF + Gaussian splats + fast parallax",
      stressors: ["specular_shimmer", "splat_depth_mismatch", "parallax"],
      target: ["source_id_stability", "motion_divergence", "history_hash"],
    },
  ];
  const cases = specs.map((spec) => ({
    ...spec,
    caseHash: stableHash(["temporal_case", spec.id, spec.scene, ...spec.stressors, ...spec.target], "temporal"),
  }));
  return {
    schema: "forge.banger.temporal_adversarial.v1",
    manifestHash: stableHash(["temporal_adversarial", ...cases.map((c) => c.caseHash)], "temporal"),
    cases,
  };
}

function vecDot(a: Vec3, b: Vec3): number {
  return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
}

function vecAdd(a: Vec3, b: Vec3): Vec3 {
  return v3(a[0] + b[0], a[1] + b[1], a[2] + b[2]);
}

function vecSub(a: Vec3, b: Vec3): Vec3 {
  return v3(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

function vecScale(a: Vec3, s: number): Vec3 {
  return v3(a[0] * s, a[1] * s, a[2] * s);
}

function vecCross(a: Vec3, b: Vec3): Vec3 {
  return v3(
    a[1] * b[2] - a[2] * b[1],
    a[2] * b[0] - a[0] * b[2],
    a[0] * b[1] - a[1] * b[0],
  );
}

function rotateAroundAxis(v: Vec3, axis: Vec3, angle: number): Vec3 {
  const a = vecNorm(axis);
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  return vecAdd(
    vecAdd(vecScale(v, c), vecScale(vecCross(a, v), s)),
    vecScale(a, vecDot(a, v) * (1 - c)),
  );
}

function deformationRigSpec(spec: Partial<DeformationRigSpec> = {}): DeformationRigSpec {
  const joints = Array.isArray(spec.joints) && spec.joints.length
    ? spec.joints
    : DEFAULT_DEFORMATION_RIG.joints;
  return {
    objectId: String(spec.objectId || DEFAULT_DEFORMATION_RIG.objectId),
    seed: Math.max(1, Number(spec.seed ?? DEFAULT_DEFORMATION_RIG.seed) | 0),
    time: Number.isFinite(Number(spec.time)) ? Number(spec.time) : DEFAULT_DEFORMATION_RIG.time,
    correctiveStrength: clamp01(Number(spec.correctiveStrength ?? DEFAULT_DEFORMATION_RIG.correctiveStrength)),
    joints: joints.map((joint, i) => ({
      id: String(joint.id || `joint_${i}`),
      partId: String(joint.partId || joint.id || `part_${i}`),
      anchor: v3(Number(joint.anchor?.[0]) || 0, Number(joint.anchor?.[1]) || 0, Number(joint.anchor?.[2]) || 0),
      axis: vecNorm(v3(Number(joint.axis?.[0]) || 0, Number(joint.axis?.[1]) || 0, Number(joint.axis?.[2]) || 1)),
      radius: Math.max(0.05, Math.min(20, Number(joint.radius) || 1)),
      bend: Math.max(-4, Math.min(4, Number(joint.bend) || 0)),
      twist: Math.max(-Math.PI, Math.min(Math.PI, Number(joint.twist) || 0)),
      muscle: Math.max(-2, Math.min(2, Number(joint.muscle) || 0)),
      phase: Number(joint.phase) || 0,
      frequency: Math.max(0, Math.min(16, Number(joint.frequency) || 1)),
    })),
  };
}

export function deformationRigHash(spec: Partial<DeformationRigSpec> = {}): string {
  const rig = deformationRigSpec(spec);
  return stableHash([
    "deformation_rig",
    rig.objectId,
    rig.seed,
    rig.time,
    rig.correctiveStrength,
    ...rig.joints.flatMap((joint) => [
      joint.id,
      joint.partId,
      joint.anchor[0],
      joint.anchor[1],
      joint.anchor[2],
      joint.axis[0],
      joint.axis[1],
      joint.axis[2],
      joint.radius,
      joint.bend,
      joint.twist,
      joint.muscle,
      joint.phase,
      joint.frequency,
    ]),
  ], "deformation");
}

function opHash(op: SdfOp, index: number, namespace: string): string {
  return stableHash(["op", index, JSON.stringify(op)], namespace);
}

function deformPoint(point: Vec3, rig: DeformationRigSpec): { point: Vec3; maxOffset: number; partId: string } {
  let out = point;
  let maxOffset = 0;
  let partId = rig.joints[0]?.partId || "root";
  for (const joint of rig.joints) {
    const rel = vecSub(out, joint.anchor);
    const axial = vecDot(rel, joint.axis);
    const radial = vecSub(rel, vecScale(joint.axis, axial));
    const dist = Math.hypot(radial[0], radial[1], radial[2]);
    const w = Math.exp(-(dist * dist) / Math.max(joint.radius * joint.radius, 0.0001))
      * clamp01(1 - Math.abs(axial) / Math.max(joint.radius * 1.8, 0.0001));
    if (w <= 0.0001) continue;
    const wave = Math.sin(rig.time * joint.frequency + joint.phase + rig.seed * 0.00017);
    const tangent = vecNorm(Math.abs(joint.axis[2]) < 0.92 ? vecCross(joint.axis, v3(0, 0, 1)) : vecCross(joint.axis, v3(1, 0, 0)));
    const rotated = vecAdd(joint.anchor, rotateAroundAxis(rel, joint.axis, joint.twist * wave * w));
    const radialDir = dist > 0.0001 ? vecScale(radial, 1 / dist) : tangent;
    const corrective = rig.correctiveStrength * joint.muscle * w * (1 - clamp01(Math.abs(axial) / Math.max(joint.radius, 0.0001)));
    const bent = vecAdd(
      rotated,
      vecAdd(vecScale(tangent, joint.bend * wave * w), vecScale(radialDir, corrective)),
    );
    const offset = Math.hypot(bent[0] - out[0], bent[1] - out[1], bent[2] - out[2]);
    if (offset > maxOffset) {
      maxOffset = offset;
      partId = joint.partId;
    }
    out = bent;
  }
  return { point: out, maxOffset, partId };
}

function deformOp(op: SdfOp, rig: DeformationRigSpec): { op: SdfOp; maxOffset: number; partId: string } {
  if (op.op === "sphere" || op.op === "box" || op.op === "torus" || op.op === "roundedBox") {
    const d = deformPoint(op.center, rig);
    return { op: { ...op, center: d.point } as SdfOp, maxOffset: d.maxOffset, partId: d.partId };
  }
  if (op.op === "capsule") {
    const a = deformPoint(op.a, rig);
    const b = deformPoint(op.b, rig);
    return {
      op: { ...op, a: a.point, b: b.point },
      maxOffset: Math.max(a.maxOffset, b.maxOffset),
      partId: a.maxOffset >= b.maxOffset ? a.partId : b.partId,
    };
  }
  return { op, maxOffset: 0, partId: "static" };
}

/** First-stage SDF animation: semantic rig -> continuous deformation field.
 *  The output is still plain SdfOp[] so the existing renderer stays single-path. */
export function deformSceneWithField(
  ops: readonly SdfOp[] = [],
  rigSpec: Partial<DeformationRigSpec> = {},
  time?: number,
): DeformedSceneResult {
  const resolvedTime = Number.isFinite(Number(time))
    ? Number(time)
    : Number.isFinite(Number(rigSpec.time))
      ? Number(rigSpec.time)
      : DEFAULT_DEFORMATION_RIG.time;
  const rig = deformationRigSpec({ ...rigSpec, time: resolvedTime });
  const sourceHash = stableHash(["deformation_source", ...ops.map((op, i) => opHash(op, i, "deformation"))], "deformation");
  const deformed: SdfOp[] = [];
  const sourceRefs: DeformationSourceRef[] = [];
  let maxOffset = 0;
  for (let i = 0; i < ops.length; i += 1) {
    const src = ops[i]!;
    const next = deformOp(src, rig);
    deformed.push(next.op);
    if (next.maxOffset > 0.0001) {
      maxOffset = Math.max(maxOffset, next.maxOffset);
      sourceRefs.push({
        opIndex: i,
        objectId: rig.objectId,
        partId: next.partId,
        sdfNodeId: `op_${i}`,
        parameterPath: `${src.op}.position`,
        sourceHash: opHash(src, i, "deformation"),
        deformedHash: opHash(next.op, i, "deformation"),
        maxOffset: next.maxOffset,
      });
    }
  }
  const rigHash = deformationRigHash(rig);
  const deformedHash = stableHash(["deformation_output", rigHash, ...deformed.map((op, i) => opHash(op, i, "deformation"))], "deformation");
  const correctiveHash = stableHash(["corrective", rigHash, rig.correctiveStrength, maxOffset, sourceRefs.length], "deformation");
  return {
    ops: deformed,
    manifest: {
      schema: "forge.banger.deformation_field.v1",
      rigHash,
      sourceHash,
      deformedHash,
      correctiveHash,
      time: rig.time,
      maxOffset,
      sourceRefs,
    },
  };
}

export function terrainErosionSample(x: number, y: number, spec: TerrainSpec = DEFAULT_TERRAIN_SPEC): TerrainErosionSample {
  const e = erosionControls(spec);
  const stream = Math.sin(x * spec.frequency * 1.7 + y * spec.frequency * 0.43);
  const basin = Math.cos(y * spec.frequency * 1.3 - x * spec.frequency * 0.31);
  const riverMask = clamp01((1 - Math.abs(stream * 0.72 + basin * 0.28)) * 1.35 - 0.22) * e.river;
  const rainWash = clamp01((Math.sin((x + y) * spec.frequency * 2.1) * 0.5 + 0.5) * e.rainfall);
  const slopeWear = clamp01((Math.cos(x * spec.frequency * 0.9) * Math.sin(y * spec.frequency * 1.1) * 0.5 + 0.5) * e.slope);
  const sedimentDeposit = clamp01((1 - riverMask) * (0.45 + 0.55 * rainWash) * e.sediment);
  const pathWear = clamp01(Math.exp(-Math.abs(y + 1.2) * 0.55) * e.pathWear);
  const effectiveErosionStrength = terrainErosionStrength(spec);
  const erosionHash = stableHash([
    "erosion",
    terrainSpecHash(spec),
    x,
    y,
    riverMask,
    rainWash,
    slopeWear,
    sedimentDeposit,
    pathWear,
    effectiveErosionStrength,
  ]);
  return { riverMask, rainWash, slopeWear, sedimentDeposit, pathWear, effectiveErosionStrength, erosionHash };
}

// Shared material palette — tweak here and every helper picks it up.
export const MATERIALS = {
  grass:    { color: v3(0.26, 0.46, 0.20), roughness: 0.85, metallic: 0.0, normalDetailStrength: 0.40, decalWeight: 0.12 },
  bark:     { color: v3(0.36, 0.22, 0.13), roughness: 0.95, metallic: 0.0, normalDetailStrength: 0.72, decalWeight: 0.20 },
  foliage:  { color: v3(0.20, 0.55, 0.24), roughness: 0.65, metallic: 0.0, normalDetailStrength: 0.30, decalWeight: 0.08 },
  stone:    { color: v3(0.70, 0.68, 0.62), roughness: 0.80, metallic: 0.0, normalDetailStrength: 0.66, decalWeight: 0.32 },
  brick:    { color: v3(0.65, 0.30, 0.22), roughness: 0.88, metallic: 0.0, normalDetailStrength: 0.52, decalWeight: 0.24 },
  roof:     { color: v3(0.50, 0.18, 0.12), roughness: 0.70, metallic: 0.0, normalDetailStrength: 0.38, decalWeight: 0.18 },
  glass:    { color: v3(0.55, 0.70, 0.80), roughness: 0.10, metallic: 0.0, normalDetailStrength: 0.04, decalWeight: 0.02 },
  metal:    { color: v3(0.85, 0.85, 0.88), roughness: 0.30, metallic: 0.95, normalDetailStrength: 0.18, decalWeight: 0.10 },
  rubber:   { color: v3(0.08, 0.08, 0.09), roughness: 0.95, metallic: 0.0, normalDetailStrength: 0.44, decalWeight: 0.16 },
  chrome:   { color: v3(0.92, 0.93, 0.95), roughness: 0.05, metallic: 1.0, normalDetailStrength: 0.03, decalWeight: 0.00 },
  paint_red:{ color: v3(0.72, 0.12, 0.10), roughness: 0.35, metallic: 0.20, normalDetailStrength: 0.22, decalWeight: 0.12 },
} as const;

function mat(spec: { color: Vec3; roughness: number; metallic: number; normalDetailStrength?: number; decalWeight?: number }): SdfOp {
  const op: SdfOp = {
    op: "material",
    color: spec.color,
    roughness: spec.roughness,
    metallic: spec.metallic,
  };
  if (spec.normalDetailStrength != null) op.normalDetailStrength = spec.normalDetailStrength;
  if (spec.decalWeight != null) op.decalWeight = spec.decalWeight;
  return op;
}

function vecMix(a: Vec3, b: Vec3, t: number): Vec3 {
  const u = clamp01(t);
  return v3(
    a[0] * (1 - u) + b[0] * u,
    a[1] * (1 - u) + b[1] * u,
    a[2] * (1 - u) + b[2] * u,
  );
}

function characterSpec(spec: Partial<CharacterSpec> = {}): CharacterSpec {
  const tone = spec.skinTone || DEFAULT_CHARACTER_SPEC.skinTone;
  return {
    seed: Math.max(1, finiteNumber(spec.seed, DEFAULT_CHARACTER_SPEC.seed) | 0),
    height: Math.max(0.45, Math.min(2.4, finiteNumber(spec.height, DEFAULT_CHARACTER_SPEC.height))),
    build: clamp01(finiteNumber(spec.build, DEFAULT_CHARACTER_SPEC.build)),
    skinTone: v3(
      clamp01(finiteNumber(tone[0], DEFAULT_CHARACTER_SPEC.skinTone[0])),
      clamp01(finiteNumber(tone[1], DEFAULT_CHARACTER_SPEC.skinTone[1])),
      clamp01(finiteNumber(tone[2], DEFAULT_CHARACTER_SPEC.skinTone[2])),
    ),
    skinFlush: clamp01(finiteNumber(spec.skinFlush, DEFAULT_CHARACTER_SPEC.skinFlush)),
    skinWetness: clamp01(finiteNumber(spec.skinWetness, DEFAULT_CHARACTER_SPEC.skinWetness)),
    poreScale: clamp01(finiteNumber(spec.poreScale, DEFAULT_CHARACTER_SPEC.poreScale)),
    hairLength: clamp01(finiteNumber(spec.hairLength, DEFAULT_CHARACTER_SPEC.hairLength)),
    hairDensity: clamp01(finiteNumber(spec.hairDensity, DEFAULT_CHARACTER_SPEC.hairDensity)),
    hairMelanin: clamp01(finiteNumber(spec.hairMelanin, DEFAULT_CHARACTER_SPEC.hairMelanin)),
    clothCoverage: clamp01(finiteNumber(spec.clothCoverage, DEFAULT_CHARACTER_SPEC.clothCoverage)),
    clothWetness: clamp01(finiteNumber(spec.clothWetness, DEFAULT_CHARACTER_SPEC.clothWetness)),
    clothDirt: clamp01(finiteNumber(spec.clothDirt, DEFAULT_CHARACTER_SPEC.clothDirt)),
    expressionSmile: clamp01(finiteNumber(spec.expressionSmile, DEFAULT_CHARACTER_SPEC.expressionSmile)),
    expressionBlink: clamp01(finiteNumber(spec.expressionBlink, DEFAULT_CHARACTER_SPEC.expressionBlink)),
    speechOpen: clamp01(finiteNumber(spec.speechOpen, DEFAULT_CHARACTER_SPEC.speechOpen)),
  };
}

export function characterSpecHash(spec: Partial<CharacterSpec> = {}): string {
  const s = characterSpec(spec);
  return stableHash([
    "character_spec",
    s.seed,
    s.height,
    s.build,
    s.skinTone[0],
    s.skinTone[1],
    s.skinTone[2],
    s.skinFlush,
    s.skinWetness,
    s.poreScale,
    s.hairLength,
    s.hairDensity,
    s.hairMelanin,
    s.clothCoverage,
    s.clothWetness,
    s.clothDirt,
    s.expressionSmile,
    s.expressionBlink,
    s.speechOpen,
  ], "character");
}

function characterSkinProfile(spec: CharacterSpec): CharacterSkinProfile {
  const flushColor = v3(0.95, 0.22, 0.18);
  const wetLift = v3(0.08, 0.06, 0.05);
  const flushed = vecMix(spec.skinTone, flushColor, spec.skinFlush * 0.34);
  const albedo = v3(
    clamp01(flushed[0] + wetLift[0] * spec.skinWetness),
    clamp01(flushed[1] + wetLift[1] * spec.skinWetness),
    clamp01(flushed[2] + wetLift[2] * spec.skinWetness),
  );
  const subsurfaceCm = 0.18 + (1 - (albedo[0] + albedo[1] + albedo[2]) / 3) * 0.34 + spec.skinFlush * 0.10;
  const microNormalStrength = 0.16 + spec.poreScale * 0.44;
  const profileHash = stableHash([
    "skin_profile",
    albedo[0],
    albedo[1],
    albedo[2],
    subsurfaceCm,
    spec.poreScale,
    microNormalStrength,
    spec.skinFlush,
    spec.skinWetness,
  ], "character");
  return {
    albedo,
    subsurfaceCm,
    poreScale: spec.poreScale,
    microNormalStrength,
    flush: spec.skinFlush,
    wetness: spec.skinWetness,
    profileHash,
  };
}

function characterSkinMaterial(skin: CharacterSkinProfile) {
  return {
    color: skin.albedo,
    roughness: 0.72 - skin.wetness * 0.28,
    metallic: 0.0,
    normalDetailStrength: skin.microNormalStrength,
    decalWeight: clamp01(skin.flush * 0.18 + skin.wetness * 0.34),
  };
}

function characterHairMaterial(spec: CharacterSpec) {
  const dark = v3(0.045, 0.032, 0.022);
  const light = v3(0.62, 0.46, 0.23);
  const color = vecMix(light, dark, spec.hairMelanin);
  return {
    color,
    roughness: 0.58 + spec.hairDensity * 0.18,
    metallic: 0.0,
    normalDetailStrength: 0.30 + spec.hairDensity * 0.20,
    decalWeight: 0.02,
  };
}

function characterClothMaterial(spec: CharacterSpec) {
  const clean = v3(0.16, 0.22, 0.50);
  const dirt = v3(0.22, 0.18, 0.12);
  return {
    color: vecMix(clean, dirt, spec.clothDirt),
    roughness: 0.82 - spec.clothWetness * 0.30,
    metallic: 0.0,
    normalDetailStrength: 0.36 + spec.clothCoverage * 0.18,
    decalWeight: clamp01(0.12 + spec.clothDirt * 0.42 + spec.clothWetness * 0.18),
  };
}

function characterHairStrands(origin: Vec3, spec: CharacterSpec, sampleCount = 12): HairStrandSample[] {
  const count = Math.max(0, Math.min(24, Math.round(sampleCount * spec.hairDensity)));
  const strands: HairStrandSample[] = [];
  if (count <= 0 || spec.hairLength <= 0.01) return strands;
  const rng = seededRandom(spec.seed ^ 0x4817);
  const h = spec.height;
  const headRadius = h * 0.092;
  const head = v3(origin[0], origin[1] - h * 0.006, origin[2] + h * 0.845);
  const golden = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < count; i += 1) {
    const u = (i + 0.5) / count;
    const angle = golden * i + rng() * 0.36;
    const side = Math.cos(angle);
    const backBias = Math.max(0.15, Math.sin(angle) * 0.65 + 0.45);
    const root = v3(
      head[0] + side * headRadius * (0.72 + rng() * 0.18),
      head[1] + backBias * headRadius * (0.30 + rng() * 0.35),
      head[2] + headRadius * (0.28 + (u - 0.5) * 0.36),
    );
    const fall = h * (0.07 + spec.hairLength * 0.26) * (0.82 + rng() * 0.38);
    const tip = v3(
      root[0] + side * headRadius * (0.10 + rng() * 0.12),
      root[1] + backBias * headRadius * (0.36 + rng() * 0.24),
      root[2] - fall,
    );
    const radius = h * (0.006 + spec.hairDensity * 0.004);
    const strandHash = stableHash(["hair_strand", spec.seed, i, root[0], root[1], root[2], tip[0], tip[1], tip[2], radius], "character");
    strands.push({ root, tip, radius, sourceRef: `hair_guide_${i}`, strandHash });
  }
  return strands;
}

function characterClothPatches(origin: Vec3, spec: CharacterSpec): ClothPatchSample[] {
  if (spec.clothCoverage <= 0.01) return [];
  const count = Math.max(1, Math.min(8, Math.round(2 + spec.clothCoverage * 6)));
  const patches: ClothPatchSample[] = [];
  const rng = seededRandom(spec.seed ^ 0xc107);
  const h = spec.height;
  for (let i = 0; i < count; i += 1) {
    const lane = count <= 1 ? 0 : (i / (count - 1) - 0.5);
    const center = v3(
      origin[0] + lane * h * (0.10 + spec.build * 0.04),
      origin[1] - h * (0.075 + rng() * 0.015),
      origin[2] + h * (0.43 + rng() * 0.26),
    );
    const wrinkle = clamp01(0.24 + rng() * 0.56 + spec.clothWetness * 0.12);
    const radius = h * (0.034 + rng() * 0.030);
    const wetness = clamp01(spec.clothWetness * (0.72 + rng() * 0.42));
    const dirt = clamp01(spec.clothDirt * (0.68 + rng() * 0.54));
    const patchHash = stableHash(["cloth_patch", spec.seed, i, center[0], center[1], center[2], wrinkle, wetness, dirt], "character");
    patches.push({ center, normal: v3(0, -1, 0), radius, wrinkle, wetness, dirt, sourceRef: `cloth_patch_${i}`, patchHash });
  }
  return patches;
}

function characterDeformationRig(origin: Vec3, spec: CharacterSpec): DeformationRigSpec {
  const h = spec.height;
  return {
    objectId: "character_sdf",
    seed: spec.seed,
    time: spec.expressionSmile + spec.speechOpen * 0.5,
    correctiveStrength: 0.22 + spec.speechOpen * 0.24 + spec.expressionSmile * 0.14,
    joints: [
      {
        id: "spine_breath",
        partId: "torso",
        anchor: v3(origin[0], origin[1], origin[2] + h * 0.52),
        axis: v3(0, 0, 1),
        radius: h * 0.42,
        bend: 0.030 + spec.expressionSmile * 0.018,
        twist: 0.026,
        muscle: 0.055 + spec.build * 0.035,
        phase: 0.3,
        frequency: 0.8,
      },
      {
        id: "jaw_speech",
        partId: "mouth",
        anchor: v3(origin[0], origin[1] - h * 0.080, origin[2] + h * 0.795),
        axis: v3(1, 0, 0),
        radius: h * 0.13,
        bend: spec.speechOpen * 0.080,
        twist: 0,
        muscle: spec.speechOpen * 0.10,
        phase: 1.1,
        frequency: 1.0,
      },
      {
        id: "smile_corrective",
        partId: "face",
        anchor: v3(origin[0], origin[1] - h * 0.087, origin[2] + h * 0.815),
        axis: v3(0, 1, 0),
        radius: h * 0.16,
        bend: spec.expressionSmile * 0.045,
        twist: spec.expressionSmile * 0.035,
        muscle: spec.expressionSmile * 0.09,
        phase: 2.2,
        frequency: 1.0,
      },
    ],
  };
}

/** SDF-authoritative character source. The LLM edits semantic dimensions,
 *  materials and expression fields; renderer caches stay derived. */
export function characterSdf(at: Vec3 = v3(0, 0, 0), specInput: Partial<CharacterSpec> = {}): SdfOp[] {
  const spec = characterSpec(specInput);
  const h = spec.height;
  const shoulder = h * (0.135 + spec.build * 0.035);
  const torsoRadius = h * (0.090 + spec.build * 0.040);
  const limbRadius = h * (0.028 + spec.build * 0.020);
  const headRadius = h * 0.092;
  const skin = characterSkinProfile(spec);
  const faceY = at[1] - headRadius * 0.82;
  const smileLift = spec.expressionSmile * h * 0.010;
  const blinkScale = 1 - spec.expressionBlink * 0.76;
  const speechGap = h * (0.004 + spec.speechOpen * 0.018);
  const ops: SdfOp[] = [
    mat(characterSkinMaterial(skin)),
    { op: "capsule", a: v3(at[0], at[1], at[2] + h * 0.34), b: v3(at[0], at[1], at[2] + h * 0.68), radius: torsoRadius },
    { op: "sphere", center: v3(at[0], at[1] - h * 0.006, at[2] + h * 0.845), radius: headRadius },
    { op: "smin", k: 7.0 },
    { op: "capsule", a: v3(at[0], at[1], at[2] + h * 0.690), b: v3(at[0], at[1] - h * 0.003, at[2] + h * 0.765), radius: h * 0.040 },
    { op: "smin", k: 8.0 },
    { op: "capsule", a: v3(at[0] - shoulder, at[1], at[2] + h * 0.655), b: v3(at[0] - shoulder * 1.34, at[1] - h * 0.018, at[2] + h * 0.420), radius: limbRadius },
    { op: "smin", k: 5.4 },
    { op: "capsule", a: v3(at[0] + shoulder, at[1], at[2] + h * 0.655), b: v3(at[0] + shoulder * 1.34, at[1] - h * 0.018, at[2] + h * 0.420), radius: limbRadius },
    { op: "smin", k: 5.4 },
    { op: "capsule", a: v3(at[0] - torsoRadius * 0.44, at[1], at[2] + h * 0.350), b: v3(at[0] - torsoRadius * 0.55, at[1] - h * 0.012, at[2] + h * 0.090), radius: limbRadius * 1.04 },
    { op: "smin", k: 5.2 },
    { op: "capsule", a: v3(at[0] + torsoRadius * 0.44, at[1], at[2] + h * 0.350), b: v3(at[0] + torsoRadius * 0.55, at[1] - h * 0.012, at[2] + h * 0.090), radius: limbRadius * 1.04 },
    { op: "smin", k: 5.2 },
    mat({ color: v3(0.025, 0.025, 0.028), roughness: 0.42, metallic: 0.0, normalDetailStrength: 0.02, decalWeight: 0.00 }),
    { op: "sphere", center: v3(at[0] - headRadius * 0.36, faceY, at[2] + h * 0.862), radius: headRadius * (0.075 * blinkScale + 0.018) },
    { op: "union" },
    { op: "sphere", center: v3(at[0] + headRadius * 0.36, faceY, at[2] + h * 0.862), radius: headRadius * (0.075 * blinkScale + 0.018) },
    { op: "union" },
    mat({ color: v3(0.42, 0.075, 0.065), roughness: 0.64 - spec.skinWetness * 0.18, metallic: 0.0, normalDetailStrength: 0.06, decalWeight: 0.08 }),
    { op: "roundedBox", center: v3(at[0], faceY - h * 0.006, at[2] + h * 0.793 + smileLift), halfExtents: v3(headRadius * (0.23 + spec.expressionSmile * 0.06), speechGap, headRadius * 0.030), cornerRadius: headRadius * 0.020 },
    { op: "union" },
  ];

  const hairStrands = characterHairStrands(at, spec, 10);
  if (hairStrands.length) {
    ops.push(mat(characterHairMaterial(spec)));
    for (const strand of hairStrands) {
      ops.push({ op: "capsule", a: strand.root, b: strand.tip, radius: strand.radius });
      ops.push({ op: "union" });
    }
  }

  if (spec.clothCoverage > 0.01) {
    ops.push(mat(characterClothMaterial(spec)));
    ops.push({
      op: "roundedBox",
      center: v3(at[0], at[1] - h * 0.020, at[2] + h * 0.525),
      halfExtents: v3(torsoRadius * 1.05, h * 0.030, h * (0.115 + spec.clothCoverage * 0.035)),
      cornerRadius: h * 0.018,
    });
    ops.push({ op: "union" });
    ops.push({
      op: "roundedBox",
      center: v3(at[0], at[1] - h * 0.018, at[2] + h * 0.365),
      halfExtents: v3(torsoRadius * (0.78 + spec.clothCoverage * 0.30), h * 0.026, h * 0.040),
      cornerRadius: h * 0.016,
    });
    ops.push({ op: "union" });
    for (const patch of characterClothPatches(at, spec).slice(0, 4)) {
      ops.push({
        op: "roundedBox",
        center: patch.center,
        halfExtents: v3(patch.radius * (1.2 + patch.wrinkle), h * 0.006, patch.radius * 0.22),
        cornerRadius: h * 0.006,
      });
      ops.push({ op: "union" });
    }
  }

  return ops;
}

export function characterManifest(at: Vec3 = v3(0, 0, 0), specInput: Partial<CharacterSpec> = {}): CharacterManifest {
  const spec = characterSpec(specInput);
  const specHash = characterSpecHash(spec);
  const skin = characterSkinProfile(spec);
  const ops = characterSdf(at, spec);
  const sourceHash = stableHash(["character_source", specHash, at[0], at[1], at[2], ...ops.map((op, i) => opHash(op, i, "character"))], "character");
  const hairStrands = characterHairStrands(at, spec, 18);
  const clothPatches = characterClothPatches(at, spec);
  const hairCacheHash = stableHash(["hair_cache", sourceHash, ...hairStrands.map((strand) => strand.strandHash)], "character");
  const clothCacheHash = stableHash(["cloth_cache", sourceHash, ...clothPatches.map((patch) => patch.patchHash)], "character");
  const deformation = deformSceneWithField(ops, characterDeformationRig(at, spec), spec.expressionSmile + spec.speechOpen * 0.5);
  const facialHash = stableHash([
    "facial_field",
    sourceHash,
    spec.expressionSmile,
    spec.expressionBlink,
    spec.speechOpen,
    deformation.manifest.correctiveHash,
  ], "character");
  const skinHash = skin.profileHash;
  const hairMaterialHash = stableHash(["hair_material", spec.hairMelanin, spec.hairDensity, spec.hairLength], "character");
  const clothMaterialHash = stableHash(["cloth_material", spec.clothCoverage, spec.clothWetness, spec.clothDirt], "character");
  const partDefs = [
    ["torso", "character.body.torso", skinHash],
    ["head", "character.body.head", skinHash],
    ["left_arm", "character.body.left_arm", skinHash],
    ["right_arm", "character.body.right_arm", skinHash],
    ["left_leg", "character.body.left_leg", skinHash],
    ["right_leg", "character.body.right_leg", skinHash],
    ["face", "character.face.blend_fields", facialHash],
    ["hair", "character.hair.groom_guides", hairMaterialHash],
    ["cloth", "character.cloth.patch_fields", clothMaterialHash],
  ] as const;
  const partRefs: CharacterPartRef[] = partDefs.map(([partId, parameterPath, materialHash]) => ({
    partId,
    sdfNodeId: `${partId}_sdf`,
    parameterPath,
    sourceHash: stableHash(["character_part", sourceHash, partId, parameterPath], "character"),
    materialHash,
  }));
  const h = spec.height;
  const boundsMin = v3(at[0] - h * 0.34, at[1] - h * 0.18, at[2] - h * 0.02);
  const boundsMax = v3(at[0] + h * 0.34, at[1] + h * 0.20, at[2] + h * (0.96 + spec.hairLength * 0.04));
  const characterHash = stableHash([
    "character_manifest",
    sourceHash,
    skinHash,
    hairCacheHash,
    clothCacheHash,
    facialHash,
    deformation.manifest.deformedHash,
    deformation.manifest.correctiveHash,
    ops.length,
  ], "character");

  return {
    schema: "forge.banger.character_sdf.v1",
    characterHash,
    specHash,
    sourceHash,
    skinHash,
    hairCacheHash,
    clothCacheHash,
    facialHash,
    deformationHash: deformation.manifest.deformedHash,
    correctiveHash: deformation.manifest.correctiveHash,
    origin: at,
    boundsMin,
    boundsMax,
    skin,
    partRefs,
    hairStrands,
    clothPatches,
    opBudget: ops.length,
  };
}

function worldStreamSpec(spec: Partial<WorldStreamSpec> = {}): WorldStreamSpec {
  return {
    seed: Math.max(1, finiteNumber(spec.seed, DEFAULT_WORLD_STREAM_SPEC.seed) | 0),
    cellSize: Math.max(2, Math.min(128, finiteNumber(spec.cellSize, DEFAULT_WORLD_STREAM_SPEC.cellSize))),
    radiusCells: Math.max(1, Math.min(8, finiteNumber(spec.radiusCells, DEFAULT_WORLD_STREAM_SPEC.radiusCells) | 0)),
    maxLod: Math.max(0, Math.min(10, finiteNumber(spec.maxLod, DEFAULT_WORLD_STREAM_SPEC.maxLod) | 0)),
    visualBudgetMb: Math.max(2, Math.min(4096, finiteNumber(spec.visualBudgetMb, DEFAULT_WORLD_STREAM_SPEC.visualBudgetMb))),
    collisionBudgetMb: Math.max(1, Math.min(1024, finiteNumber(spec.collisionBudgetMb, DEFAULT_WORLD_STREAM_SPEC.collisionBudgetMb))),
    ioBudgetMbPerSec: Math.max(1, Math.min(8192, finiteNumber(spec.ioBudgetMbPerSec, DEFAULT_WORLD_STREAM_SPEC.ioBudgetMbPerSec))),
    frameBudgetMs: Math.max(0.1, Math.min(16, finiteNumber(spec.frameBudgetMs, DEFAULT_WORLD_STREAM_SPEC.frameBudgetMs))),
    targetScreenErrorPx: Math.max(0.1, Math.min(8, finiteNumber(spec.targetScreenErrorPx, DEFAULT_WORLD_STREAM_SPEC.targetScreenErrorPx))),
    prefetchSeconds: Math.max(0, Math.min(8, finiteNumber(spec.prefetchSeconds, DEFAULT_WORLD_STREAM_SPEC.prefetchSeconds))),
    hysteresisCells: Math.max(0, Math.min(4, finiteNumber(spec.hysteresisCells, DEFAULT_WORLD_STREAM_SPEC.hysteresisCells) | 0)),
  };
}

function renderGraphSpec(spec: Partial<RenderGraphSpec> = {}): RenderGraphSpec {
  return {
    frameBudgetMs: Math.max(1, Math.min(100, finiteNumber(spec.frameBudgetMs, DEFAULT_RENDER_GRAPH_SPEC.frameBudgetMs))),
    vramBudgetMb: Math.max(8, Math.min(16384, finiteNumber(spec.vramBudgetMb, DEFAULT_RENDER_GRAPH_SPEC.vramBudgetMb))),
    bandwidthBudgetMb: Math.max(16, Math.min(65536, finiteNumber(spec.bandwidthBudgetMb, DEFAULT_RENDER_GRAPH_SPEC.bandwidthBudgetMb))),
    logicalBindlessSlots: Math.max(16, Math.min(65536, finiteNumber(spec.logicalBindlessSlots, DEFAULT_RENDER_GRAPH_SPEC.logicalBindlessSlots) | 0)),
    targetHz: Math.max(1, Math.min(240, finiteNumber(spec.targetHz, DEFAULT_RENDER_GRAPH_SPEC.targetHz))),
    viewportHeightPx: Math.max(120, Math.min(4320, finiteNumber(spec.viewportHeightPx, DEFAULT_RENDER_GRAPH_SPEC.viewportHeightPx))),
    fovYDeg: Math.max(20, Math.min(130, finiteNumber(spec.fovYDeg, DEFAULT_RENDER_GRAPH_SPEC.fovYDeg))),
    viewForward: streamVec(spec.viewForward, DEFAULT_RENDER_GRAPH_SPEC.viewForward),
    hzbBaseMipPx: Math.max(1, Math.min(128, finiteNumber(spec.hzbBaseMipPx, DEFAULT_RENDER_GRAPH_SPEC.hzbBaseMipPx))),
    maxCandidatePages: Math.max(64, Math.min(1048576, finiteNumber(spec.maxCandidatePages, DEFAULT_RENDER_GRAPH_SPEC.maxCandidatePages) | 0)),
    maxVisiblePages: Math.max(16, Math.min(1048576, finiteNumber(spec.maxVisiblePages, DEFAULT_RENDER_GRAPH_SPEC.maxVisiblePages) | 0)),
  };
}

function streamVec(value: readonly number[] | undefined, fallback: Vec3): Vec3 {
  const src = Array.isArray(value) ? value : fallback;
  return v3(
    finiteNumber(src[0], fallback[0]),
    finiteNumber(src[1], fallback[1]),
    finiteNumber(src[2], fallback[2]),
  );
}

function streamQuality(lod: number): WorldStreamCell["quality"] {
  if (lod <= 1) return "near-sdf";
  if (lod <= 3) return "fieldlet-cache";
  return "surfel-far";
}

function streamCost(quality: WorldStreamCell["quality"], lod: number): { visualMb: number; collisionMb: number; ioMb: number; frameMs: number } {
  if (quality === "near-sdf") {
    return { visualMb: 4.8 / (lod + 1), collisionMb: 1.8, ioMb: 1.35, frameMs: 0.18 };
  }
  if (quality === "fieldlet-cache") {
    return { visualMb: 1.45 / Math.max(1, lod), collisionMb: 0.42, ioMb: 0.42, frameMs: 0.055 };
  }
  if (quality === "surfel-far") {
    return { visualMb: 0.34, collisionMb: 0.08, ioMb: 0.12, frameMs: 0.018 };
  }
  return { visualMb: 0, collisionMb: 0, ioMb: 0, frameMs: 0 };
}

function renderGraphCellPages(cell: WorldStreamCell): number {
  if (!cell.resident) return 0;
  if (cell.quality === "near-sdf") return 8;
  if (cell.quality === "fieldlet-cache") return 4;
  if (cell.quality === "surfel-far") return 1;
  return 0;
}

function renderGraphCellBytes(cell: WorldStreamCell): number {
  const base = cell.visualMb + cell.collisionMb + cell.ioMb * 0.25;
  const qualityBoost = cell.quality === "near-sdf" ? 0.72 : cell.quality === "fieldlet-cache" ? 0.24 : 0.08;
  return Math.max(256, Math.round((base + qualityBoost) * 1024 * 1024));
}

function runtimePassMs(runtime: RenderGraphRuntimeStats, id: string): number {
  const pass = runtime.passes?.find((entry) => entry.id === id);
  return finiteNumber(pass?.cpuMs, 0);
}

function runtimePassWorkgroups(runtime: RenderGraphRuntimeStats, id: string, fallback: number): number {
  const pass = runtime.passes?.find((entry) => entry.id === id);
  return Math.max(0, finiteNumber(pass?.workgroups, fallback) | 0);
}

function vecLen(v: Vec3): number {
  return Math.hypot(v[0], v[1], v[2]);
}

function vecNormalize(v: Vec3, fallback: Vec3 = v3(1, 0, 0)): Vec3 {
  const len = vecLen(v);
  if (len <= 0.0001) return fallback;
  return v3(v[0] / len, v[1] / len, v[2] / len);
}

function cellToCamera(cell: WorldStreamCell, camera: Vec3): Vec3 {
  return v3(cell.center[0] - camera[0], cell.center[1] - camera[1], cell.center[2] - camera[2]);
}

function cullingNoise(cell: WorldStreamCell, page: number): number {
  const n = Math.sin((cell.cellX * 127.1 + cell.cellY * 311.7 + cell.lod * 17.3 + page * 43.9) * 12.9898) * 43758.5453;
  return n - Math.floor(n);
}

function renderGraphCullingPages(
  stream: WorldStreamManifest,
  spec: RenderGraphSpec,
): readonly RenderGraphCullingPage[] {
  const velocityLen = vecLen(stream.velocity);
  const forward = vecNormalize(
    velocityLen > 0.05 ? stream.velocity : spec.viewForward,
    DEFAULT_RENDER_GRAPH_SPEC.viewForward,
  );
  const cosHalfFov = Math.cos((spec.fovYDeg * Math.PI / 180) * 0.62);
  const cells = [...stream.cells].sort((a, b) => b.priority - a.priority || a.cellHash.localeCompare(b.cellHash));
  const pages: RenderGraphCullingPage[] = [];
  let compactedIndex = 0;

  for (const cell of cells) {
    const pageCount = cell.resident ? Math.max(1, renderGraphCellPages(cell)) : 1;
    const toCell = cellToCamera(cell, stream.camera);
    const distance = Math.max(0.001, vecLen(toCell));
    const dir = vecNormalize(toCell, forward);
    const angleCos = vecDot(dir, forward);
    const pageRadius = stream.cellSize * (0.38 + cell.lod * 0.08);
    const frustumVisible = angleCos > cosHalfFov - Math.min(0.35, pageRadius / Math.max(1, distance));
    const baseOccluded = cell.resident
      && frustumVisible
      && cell.predictedCenterDistance > stream.cellSize * 1.8
      && cell.screenErrorPx < stream.targetScreenErrorPx * 0.92;

    for (let page = 0; page < pageCount; page += 1) {
      const pageIndex = pages.length;
      const candidateOverflow = pageIndex >= spec.maxCandidatePages;
      const pageOffset = (page - (pageCount - 1) * 0.5) / Math.max(1, pageCount);
      const boundsHalf = stream.cellSize * (0.5 + cell.lod * 0.08) / Math.max(1, Math.sqrt(pageCount));
      const boundsMin = v3(
        cell.center[0] - boundsHalf + pageOffset * stream.cellSize * 0.35,
        cell.center[1] - boundsHalf,
        cell.center[2] - 2.5 - cell.lod * 0.45,
      );
      const boundsMax = v3(
        cell.center[0] + boundsHalf + pageOffset * stream.cellSize * 0.35,
        cell.center[1] + boundsHalf,
        cell.center[2] + 3.5 + cell.lod * 0.65,
      );
      const hzbMip = Math.max(0, Math.min(12, Math.floor(Math.log2(Math.max(1, spec.viewportHeightPx / Math.max(spec.hzbBaseMipPx, cell.screenErrorPx * spec.hzbBaseMipPx))))));
      const occluded = baseOccluded && cullingNoise(cell, page) > 0.73;
      const occlusionVisible = !occluded;
      let visible = cell.resident && frustumVisible && occlusionVisible && !candidateOverflow;
      let reason: RenderGraphCullingPage["reason"] = visible ? "visible" : !cell.resident ? "stream-culled" : !frustumVisible ? "frustum" : !occlusionVisible ? "hzb-occluded" : "candidate-overflow";
      let compacted = -1;
      if (visible) {
        if (compactedIndex >= spec.maxVisiblePages) {
          visible = false;
          reason = "visible-overflow";
        } else {
          compacted = compactedIndex;
          compactedIndex += 1;
        }
      }
      const indirectGroup = compacted >= 0 ? Math.floor(compacted / 64) : -1;
      const pageHash = stableHash(["cull_page", cell.cellHash, page, cell.sourceHash, cell.quality, boundsMin[0], boundsMin[1], boundsMax[0], boundsMax[1]], "render_graph");
      const recordHash = stableHash([
        "cull_record",
        pageHash,
        cell.screenErrorPx,
        hzbMip,
        frustumVisible,
        occlusionVisible,
        visible,
        compacted,
        indirectGroup,
        reason,
      ], "render_graph");
      pages.push({
        pageIndex,
        cellHash: cell.cellHash,
        sourceHash: cell.sourceHash,
        pageHash,
        quality: cell.quality,
        boundsMin,
        boundsMax,
        screenErrorPx: cell.screenErrorPx,
        hzbMip,
        frustumVisible,
        occlusionVisible,
        visible,
        compactedIndex: compacted,
        indirectGroup,
        reason,
        recordHash,
      });
    }
  }

  return pages;
}

export function worldStreamManifest(
  cameraInput: readonly number[] = v3(0, 0, 1.7),
  velocityInput: readonly number[] = v3(0, 0, 0),
  specInput: Partial<WorldStreamSpec> = {},
): WorldStreamManifest {
  const spec = worldStreamSpec(specInput);
  const camera = streamVec(cameraInput, v3(0, 0, 1.7));
  const velocity = streamVec(velocityInput, v3(0, 0, 0));
  const predictedSource = v3(
    camera[0] + velocity[0] * spec.prefetchSeconds,
    camera[1] + velocity[1] * spec.prefetchSeconds,
    camera[2] + velocity[2] * spec.prefetchSeconds,
  );
  const radius = spec.radiusCells + spec.hysteresisCells;
  const sx = Math.floor(predictedSource[0] / spec.cellSize);
  const sy = Math.floor(predictedSource[1] / spec.cellSize);
  const candidates: WorldStreamCell[] = [];
  const fovScale = 935;

  for (let y = sy - radius; y <= sy + radius; y += 1) {
    for (let x = sx - radius; x <= sx + radius; x += 1) {
      const center = v3((x + 0.5) * spec.cellSize, (y + 0.5) * spec.cellSize, 0);
      const dx = center[0] - camera[0];
      const dy = center[1] - camera[1];
      const dz = center[2] - camera[2];
      const distance = Math.max(0.5, Math.hypot(dx, dy, dz));
      const pdx = center[0] - predictedSource[0];
      const pdy = center[1] - predictedSource[1];
      const pdz = center[2] - predictedSource[2];
      const predictedCenterDistance = Math.max(0.5, Math.hypot(pdx, pdy, pdz));
      const lod = Math.min(spec.maxLod, Math.max(0, Math.floor(Math.log2(Math.max(1, predictedCenterDistance / spec.cellSize)))));
      const quality = streamQuality(lod);
      const cost = streamCost(quality, lod);
      const cellWorldError = spec.cellSize / Math.max(1, 2 ** (lod + 4));
      const screenErrorPx = cellWorldError / distance * fovScale;
      const terrainZ = terrainSurfaceZ(
        center[0],
        center[1],
        DEFAULT_TERRAIN_SPEC.amplitude,
        DEFAULT_TERRAIN_SPEC.frequency,
        DEFAULT_TERRAIN_SPEC.groundZ,
        DEFAULT_TERRAIN_SPEC.octaves,
        terrainErosionStrength(DEFAULT_TERRAIN_SPEC),
      );
      const terrainCellHash = stableHash(["world_terrain_cell", spec.seed, x, y, lod, terrainZ], "world_stream");
      const vegetationSeed = Math.abs((spec.seed ^ (x * 73856093) ^ (y * 19349663)) | 0) + 1;
      const vegetationCacheHash = vegetationClusterManifest(center, { seed: vegetationSeed, height: 1.4 + (Math.abs(x + y) % 5) * 0.18 }, { sampleCount: 6, radiusCells: 1 }).clusterHash;
      const sourceHash = stableHash(["world_cell_source", terrainCellHash, vegetationCacheHash, x, y, lod], "world_stream");
      const visualCacheHash = stableHash(["world_visual_cache", sourceHash, quality, screenErrorPx, cost.visualMb], "world_stream");
      const collisionCacheHash = stableHash(["world_collision_cache", sourceHash, lod, cost.collisionMb], "world_stream");
      const cellHash = stableHash(["world_cell", sourceHash, visualCacheHash, collisionCacheHash, spec.cellSize, x, y], "world_stream");
      const velocityLen = Math.max(0.0001, Math.hypot(velocity[0], velocity[1], velocity[2]));
      const ahead = velocityLen > 0.01
        ? ((center[0] - camera[0]) * velocity[0] + (center[1] - camera[1]) * velocity[1] + (center[2] - camera[2]) * velocity[2]) / (distance * velocityLen)
        : 0;
      const priority = 1 / (1 + predictedCenterDistance) + Math.max(0, ahead) * 0.35 + Math.max(0, spec.targetScreenErrorPx - screenErrorPx) * 0.04;
      candidates.push({
        cellX: x,
        cellY: y,
        lod,
        quality,
        center,
        predictedCenterDistance,
        screenErrorPx,
        resident: false,
        prefetch: false,
        priority,
        visualMb: cost.visualMb,
        collisionMb: cost.collisionMb,
        ioMb: cost.ioMb,
        frameMs: cost.frameMs,
        cellHash,
        sourceHash,
        terrainCellHash,
        visualCacheHash,
        collisionCacheHash,
        vegetationCacheHash,
      });
    }
  }

  const sorted = [...candidates].sort((a, b) => b.priority - a.priority || a.cellHash.localeCompare(b.cellHash));
  const ioWindowMb = spec.ioBudgetMbPerSec * Math.max(0.25, spec.prefetchSeconds);
  let visualUsedMb = 0;
  let collisionUsedMb = 0;
  let ioUsedMb = 0;
  let frameUsedMs = 0;
  const accepted = new Set<string>();
  const prefetched = new Set<string>();

  for (const cell of sorted) {
    const withinVisual = visualUsedMb + cell.visualMb <= spec.visualBudgetMb;
    const withinCollision = collisionUsedMb + cell.collisionMb <= spec.collisionBudgetMb;
    const withinIo = ioUsedMb + cell.ioMb <= ioWindowMb;
    const withinFrame = frameUsedMs + cell.frameMs <= spec.frameBudgetMs;
    const shouldLoad = cell.predictedCenterDistance <= spec.cellSize * (spec.radiusCells + 0.65)
      || cell.screenErrorPx <= spec.targetScreenErrorPx * 1.35;
    if (withinVisual && withinCollision && withinIo && withinFrame && shouldLoad) {
      accepted.add(cell.cellHash);
      visualUsedMb += cell.visualMb;
      collisionUsedMb += cell.collisionMb;
      ioUsedMb += cell.ioMb;
      frameUsedMs += cell.frameMs;
      if (cell.predictedCenterDistance > spec.cellSize * 1.15) prefetched.add(cell.cellHash);
    }
  }

  const cells = candidates
    .map((cell) => ({
      ...cell,
      resident: accepted.has(cell.cellHash),
      prefetch: prefetched.has(cell.cellHash),
      quality: accepted.has(cell.cellHash) ? cell.quality : "culled" as WorldStreamCell["quality"],
    }))
    .sort((a, b) => a.cellY - b.cellY || a.cellX - b.cellX);
  const replay: WorldStreamReplayEvent[] = cells.map((cell, step) => {
    const action: WorldStreamReplayEvent["action"] = cell.resident
      ? cell.prefetch ? "prefetch" : "load"
      : cell.screenErrorPx <= spec.targetScreenErrorPx * 1.35 ? "evict" : "keep";
    const reason = action === "evict"
      ? "budget_or_hysteresis"
      : action === "keep"
        ? "below_priority_or_far_lod"
        : cell.prefetch
          ? "predicted_camera_path"
          : "current_streaming_source";
    return {
      step,
      action,
      cellHash: cell.cellHash,
      reason,
      eventHash: stableHash(["world_replay", step, action, cell.cellHash, reason], "world_stream"),
    };
  });
  const residentCells = cells.filter((cell) => cell.resident);
  const maxScreenErrorPx = residentCells.reduce((m, cell) => Math.max(m, cell.screenErrorPx), 0);
  const popRiskPx = cells.reduce((m, cell) => {
    if (cell.resident || cell.screenErrorPx <= spec.targetScreenErrorPx * 1.35) return m;
    return Math.max(m, cell.screenErrorPx - spec.targetScreenErrorPx * 1.35);
  }, 0);
  const evictCount = replay.filter((event) => event.action === "evict").length;
  const sourceHash = stableHash(["world_source", spec.seed, ...cells.map((cell) => cell.sourceHash)], "world_stream");
  const replayHash = stableHash(["world_replay_root", ...replay.map((event) => event.eventHash)], "world_stream");
  const budget: WorldStreamBudget = {
    visualBudgetMb: spec.visualBudgetMb,
    collisionBudgetMb: spec.collisionBudgetMb,
    ioBudgetMbPerSec: spec.ioBudgetMbPerSec,
    frameBudgetMs: spec.frameBudgetMs,
    visualUsedMb,
    collisionUsedMb,
    ioUsedMb,
    frameUsedMs,
    budgetOk: visualUsedMb <= spec.visualBudgetMb
      && collisionUsedMb <= spec.collisionBudgetMb
      && ioUsedMb <= ioWindowMb
      && frameUsedMs <= spec.frameBudgetMs,
  };
  const streamHash = stableHash([
    "world_stream",
    sourceHash,
    replayHash,
    camera[0],
    camera[1],
    camera[2],
    velocity[0],
    velocity[1],
    velocity[2],
    maxScreenErrorPx,
    popRiskPx,
    budget.budgetOk,
  ], "world_stream");

  return {
    schema: "forge.banger.world_stream.v1",
    streamHash,
    sourceHash,
    replayHash,
    camera,
    velocity,
    predictedSource,
    cellSize: spec.cellSize,
    targetScreenErrorPx: spec.targetScreenErrorPx,
    maxScreenErrorPx,
    popRiskPx,
    residentCount: residentCells.length,
    prefetchCount: prefetched.size,
    evictCount,
    budget,
    cells,
    replay,
  };
}

export function renderGraphManifest(
  cameraInput: readonly number[] = v3(0, 0, 1.7),
  velocityInput: readonly number[] = v3(0, 0, 0),
  streamSpecInput: Partial<WorldStreamSpec> = {},
  graphSpecInput: Partial<RenderGraphSpec> = {},
  runtime: RenderGraphRuntimeStats = {},
): RenderGraphManifest {
  const stream = worldStreamManifest(cameraInput, velocityInput, streamSpecInput);
  const spec = renderGraphSpec(graphSpecInput);
  const residentCells = stream.cells.filter((cell) => cell.resident);
  const culledCells = stream.cells.filter((cell) => !cell.resident);
  const cullingPages = renderGraphCullingPages(stream, spec);
  const visiblePages = cullingPages.filter((page) => page.visible);
  const visiblePageCount = visiblePages.length;
  const streamCulledPageCount = cullingPages.filter((page) => page.reason === "stream-culled").length;
  const frustumCulledPageCount = cullingPages.filter((page) => page.reason === "frustum").length;
  const hzbCulledPageCount = cullingPages.filter((page) => page.reason === "hzb-occluded").length;
  const overflowCulledPageCount = cullingPages.filter((page) => page.reason === "candidate-overflow" || page.reason === "visible-overflow").length;
  const visiblePageListHash = stableHash([
    "visible_pages",
    stream.streamHash,
    ...visiblePages.map((page) => page.pageHash),
  ], "render_graph");
  const occlusionPyramidHash = stableHash([
    "hzb",
    stream.sourceHash,
    stream.targetScreenErrorPx,
    stream.maxScreenErrorPx,
    spec.viewportHeightPx,
    spec.hzbBaseMipPx,
    ...cullingPages.map((page) => page.hzbMip),
  ], "render_graph");
  const indirectDispatches = Math.max(1, Math.ceil(Math.max(1, visiblePageCount) / 64));
  const indirectArgsHash = stableHash([
    "indirect_args",
    visiblePageCount,
    indirectDispatches,
    cullingPages.length,
    streamCulledPageCount,
    frustumCulledPageCount,
    hzbCulledPageCount,
    overflowCulledPageCount,
  ], "render_graph");
  const indirectArgs: RenderGraphIndirectArgs = {
    schema: "forge.banger.indirect_args.v1",
    dispatchWorkgroups: v3(indirectDispatches, 1, 1),
    drawIndexedIndirectCount: visiblePageCount,
    candidateBytes: cullingPages.length * 64,
    visibleBytes: visiblePageCount * 16,
    argsHash: indirectArgsHash,
  };
  const compactionHash = stableHash([
    "compact",
    stream.streamHash,
    visiblePageCount,
    indirectDispatches,
    culledCells.length,
    indirectArgs.argsHash,
    ...visiblePages.map((page) => page.recordHash),
  ], "render_graph");
  const cullingHash = stableHash([
    "culling",
    visiblePageListHash,
    occlusionPyramidHash,
    compactionHash,
    residentCells.length,
    culledCells.length,
    cullingPages.length,
    streamCulledPageCount,
    frustumCulledPageCount,
    hzbCulledPageCount,
    overflowCulledPageCount,
  ], "render_graph");
  const culling: RenderGraphCullingPlan = {
    schema: "forge.banger.gpu_culling_plan.v1",
    mode: "page-cluster-compaction",
    candidateCells: stream.cells.length,
    residentCells: residentCells.length,
    culledCells: culledCells.length,
    candidatePageCount: cullingPages.length,
    streamCulledPageCount,
    frustumCulledPageCount,
    hzbCulledPageCount,
    overflowCulledPageCount,
    visiblePageCount,
    indirectDispatches,
    indirectArgs,
    occlusionPyramidHash,
    visiblePageListHash,
    compactionHash,
    pages: cullingPages,
    cullingHash,
  };

  const entries: RenderGraphResourceEntry[] = [];
  const pushResource = (
    role: RenderGraphResourceEntry["role"],
    cellHash: string,
    sourceHash: string,
    cacheHash: string,
    bytes: number,
    transient: boolean,
    lifetime: readonly string[],
  ) => {
    const slot = entries.length;
    const entryHash = stableHash([
      "resource",
      slot,
      role,
      cellHash,
      sourceHash,
      cacheHash,
      bytes,
      transient,
      ...lifetime,
    ], "render_graph");
    entries.push({ slot, role, cellHash, sourceHash, cacheHash, bytes, transient, lifetime, entryHash });
  };

  pushResource("sdf-source", "world", stream.sourceHash, stream.streamHash, Math.max(1024, stream.cells.length * 192), false, ["stream_residency", "sdf_page_cull_compact", "main_sdf_raycast"]);
  for (const cell of residentCells) {
    pushResource("page-cache", cell.cellHash, cell.sourceHash, cell.visualCacheHash, renderGraphCellBytes(cell), true, ["sdf_page_cull_compact", "main_sdf_raycast"]);
  }
  pushResource("material-table", "world", stream.sourceHash, stableHash(["material_table", ...residentCells.map((cell) => cell.visualCacheHash)], "render_graph"), Math.max(4096, residentCells.length * 640), true, ["main_sdf_raycast"]);
  pushResource("shadow-pages", "sun", stream.sourceHash, stableHash(["shadow_pages", cullingHash, runtime.shadowPages ?? 0], "render_graph"), Math.max(4096, visiblePageCount * 384), true, ["sdf_shadow_pages", "main_sdf_raycast"]);
  pushResource("radiance-probes", "world", stream.sourceHash, stableHash(["radiance_probes", stream.sourceHash, visiblePageCount], "render_graph"), 16 * 16 * 16 * 3 * 16, true, ["radiance_probes", "main_sdf_raycast"]);
  pushResource("history", "frame", stream.sourceHash, stableHash(["history", runtime.frameHash ?? "none", runtime.hasHistory ?? false], "render_graph"), Math.max(0, finiteNumber(runtime.width, 0) * finiteNumber(runtime.height, 0) * 16), true, ["post_temporal_present"]);
  pushResource("output", "frame", stream.sourceHash, stableHash(["output", runtime.frameHash ?? "none", runtime.width ?? 0, runtime.height ?? 0], "render_graph"), Math.max(0, finiteNumber(runtime.width, 0) * finiteNumber(runtime.height, 0) * 4), true, ["main_sdf_raycast", "post_temporal_present"]);

  const resourcesHash = stableHash(["resource_table", spec.logicalBindlessSlots, ...entries.map((entry) => entry.entryHash)], "render_graph");
  const resources: RenderGraphResourceTable = {
    schema: "forge.banger.logical_resource_table.v1",
    mode: "webgpu-storage-table-now_native-bindless-later",
    slotBudget: spec.logicalBindlessSlots,
    usedSlots: entries.length,
    tableBudgetOk: entries.length <= spec.logicalBindlessSlots,
    tableHash: resourcesHash,
    entries,
  };

  const rawBytes = entries.reduce((sum, entry) => sum + entry.bytes, 0);
  const sdfPageBytes = residentCells.reduce((sum, cell) => sum + Math.round(renderGraphCellBytes(cell) * 0.52), 0);
  const surfelBytes = residentCells.reduce((sum, cell) => sum + Math.round(renderGraphCellBytes(cell) * (cell.quality === "surfel-far" ? 0.54 : 0.18)), 0);
  const radianceBytes = 16 * 16 * 16 * 3 * 8;
  const materialBytes = Math.max(2048, residentCells.length * 160);
  const compressedBytes = Math.round(sdfPageBytes * 0.42 + surfelBytes * 0.35 + radianceBytes * 0.55 + materialBytes * 0.68);
  const compressionHash = stableHash([
    "compression",
    rawBytes,
    compressedBytes,
    sdfPageBytes,
    surfelBytes,
    radianceBytes,
    materialBytes,
  ], "render_graph");
  const compression: RenderGraphCompressionPlan = {
    schema: "forge.banger.render_compression.v1",
    rawBytes,
    compressedBytes,
    compressionRatio: rawBytes > 0 ? compressedBytes / rawBytes : 1,
    sdfPageBytes,
    surfelBytes,
    radianceBytes,
    materialBytes,
    compressionHash,
  };

  const pass = (
    id: RenderGraphPass["id"],
    kind: RenderGraphPass["kind"],
    reads: readonly string[],
    writes: readonly string[],
    dispatch: RenderGraphPass["dispatch"],
    workgroups: number,
    estimatedMs: number,
  ): RenderGraphPass => {
    const measuredMs = runtimePassMs(runtime, id);
    const passHash = stableHash([
      "pass",
      id,
      kind,
      dispatch,
      workgroups,
      estimatedMs,
      measuredMs,
      ...reads,
      ...writes,
    ], "render_graph");
    return { id, kind, reads, writes, dispatch, workgroups, estimatedMs, measuredMs, passHash };
  };

  const shadowHash = entries.find((entry) => entry.role === "shadow-pages")?.cacheHash ?? cullingHash;
  const probesHash = entries.find((entry) => entry.role === "radiance-probes")?.cacheHash ?? stream.sourceHash;
  const outputHash = entries.find((entry) => entry.role === "output")?.cacheHash ?? stream.streamHash;
  const passList: RenderGraphPass[] = [
    pass("stream_residency", "stream", [stream.replayHash], [resources.tableHash], "cpu-manifest", stream.cells.length, Math.max(0.02, stream.budget.ioUsedMb / Math.max(1, stream.budget.ioBudgetMbPerSec) * spec.frameBudgetMs)),
    pass("sdf_page_cull_compact", "cull", [resources.tableHash, occlusionPyramidHash], [visiblePageListHash, compactionHash, indirectArgsHash], "gpu-indirect-plan", indirectDispatches, Math.max(0.03, stream.cells.length * 0.003 + visiblePageCount * 0.0015)),
    pass("sdf_shadow_pages", "shadow", [visiblePageListHash], [shadowHash], "gpu-compute", runtimePassWorkgroups(runtime, "shadow_pages", Math.max(1, Math.ceil(finiteNumber(runtime.splats, 0) / 64))), Math.max(0.02, visiblePageCount * 0.002)),
    pass("radiance_probes", "gi", [resources.tableHash, visiblePageListHash], [probesHash], "gpu-compute", runtimePassWorkgroups(runtime, "radiance_probes", 192), Math.max(0.08, visiblePageCount * 0.004)),
    pass("main_sdf_raycast", "main", [resources.tableHash, visiblePageListHash, compression.compressionHash], [outputHash], "gpu-compute", runtimePassWorkgroups(runtime, "main_sdf_raycast", Math.max(1, Math.ceil(finiteNumber(runtime.width, 1280) / 8) * Math.ceil(finiteNumber(runtime.height, 720) / 8))), Math.max(0.18, stream.budget.frameUsedMs)),
    pass("post_temporal_present", "post", [outputHash], [runtime.frameHash ?? stream.streamHash], "copy-present", runtimePassWorkgroups(runtime, "post_temporal_present", 1), 0.04),
  ];
  const passesHash = stableHash(["passes", ...passList.map((entry) => entry.passHash)], "render_graph");
  const estimatedFrameMs = passList.reduce((sum, entry) => sum + entry.estimatedMs, 0);
  const measuredFrameMs = finiteNumber(runtime.cpuFrameMs, passList.reduce((sum, entry) => sum + entry.measuredMs, 0));
  const vramUsedMb = finiteNumber(runtime.approxVramBytes, rawBytes) / (1024 * 1024);
  const bandwidthUsedMb = compression.compressedBytes / (1024 * 1024) + Math.max(0, stream.budget.ioUsedMb);
  const frameCacheHitRatio = clamp01(finiteNumber(runtime.frameCacheHitRatio, 0));
  const profileHash = stableHash([
    "profile",
    spec.frameBudgetMs,
    estimatedFrameMs,
    measuredFrameMs,
    spec.vramBudgetMb,
    vramUsedMb,
    spec.bandwidthBudgetMb,
    bandwidthUsedMb,
    frameCacheHitRatio,
  ], "render_graph");
  const profile: RenderGraphProfile = {
    schema: "forge.banger.render_graph_profile.v1",
    frameBudgetMs: spec.frameBudgetMs,
    estimatedFrameMs,
    measuredFrameMs,
    vramBudgetMb: spec.vramBudgetMb,
    vramUsedMb,
    bandwidthBudgetMb: spec.bandwidthBudgetMb,
    bandwidthUsedMb,
    frameCacheHitRatio,
    passCount: passList.length,
    budgetOk: stream.budget.budgetOk
      && resources.tableBudgetOk
      && estimatedFrameMs <= spec.frameBudgetMs
      && vramUsedMb <= spec.vramBudgetMb
      && bandwidthUsedMb <= spec.bandwidthBudgetMb,
    profileHash,
  };
  const rendererHash = stableHash([
    "renderer",
    runtime.frameHash ?? "none",
    runtime.sourceHash ?? "none",
    runtime.bindGroupBindings ?? 0,
    runtime.width ?? 0,
    runtime.height ?? 0,
  ], "render_graph");
  const graphHash = stableHash([
    "render_graph",
    stream.streamHash,
    rendererHash,
    passesHash,
    resourcesHash,
    cullingHash,
    compressionHash,
    profileHash,
  ], "render_graph");

  return {
    schema: "forge.banger.render_graph.v1",
    graphHash,
    streamHash: stream.streamHash,
    sourceHash: stream.sourceHash,
    rendererHash,
    passesHash,
    resourcesHash,
    cullingHash,
    compressionHash,
    profileHash,
    stream,
    passes: passList,
    resources,
    culling,
    compression,
    profile,
    doctrine: "SDF-authoritative render graph; GPU resources are transient hashable caches",
  };
}

/** Ground field via compact OP_TERRAIN. Optional advanced strengths stay
 *  inside the same op slots: no extra scene nodes, no extra GPU bindings. */
export function terrain(
  amplitude = 1.2,
  frequency = 0.08,
  groundZ = -1.0,
  octaves = 4,
  advanced: { caveStrength?: number; overhangStrength?: number; erosionStrength?: number } = {},
): SdfOp[] {
  return [
    mat(MATERIALS.grass),
    { op: "terrain", amplitude, frequency, groundZ, octaves, ...advanced },
  ];
}

export function terrainStreamManifest(
  streamingSource: Vec3 = v3(0, 0, 2),
  spec: TerrainSpec = DEFAULT_TERRAIN_SPEC,
  options: {
    readonly tileSize?: number;
    readonly radiusTiles?: number;
    readonly maxLod?: number;
    readonly viewportHeightPx?: number;
    readonly fovYDeg?: number;
    readonly targetScreenErrorPx?: number;
  } = {},
): TerrainStreamManifest {
  const tileSize = Math.max(4, options.tileSize ?? 8);
  const radiusTiles = Math.max(1, Math.min(8, options.radiusTiles ?? 2));
  const maxLod = Math.max(0, Math.min(8, options.maxLod ?? 4));
  const viewportHeightPx = Math.max(240, options.viewportHeightPx ?? 1080);
  const fovY = Math.max(30, Math.min(120, options.fovYDeg ?? 60)) * Math.PI / 180;
  const targetScreenErrorPx = Math.max(0.25, options.targetScreenErrorPx ?? 1.25);
  const sourceHash = terrainSpecHash(spec);
  const sx = Math.floor(streamingSource[0] / tileSize);
  const sy = Math.floor(streamingSource[1] / tileSize);
  const tiles: TerrainStreamTile[] = [];

  for (let y = sy - radiusTiles; y <= sy + radiusTiles; y += 1) {
    for (let x = sx - radiusTiles; x <= sx + radiusTiles; x += 1) {
      const minX = x * tileSize;
      const minY = y * tileSize;
      const maxX = minX + tileSize;
      const maxY = minY + tileSize;
      const centerX = minX + tileSize * 0.5;
      const centerY = minY + tileSize * 0.5;
      const dx = centerX - streamingSource[0];
      const dy = centerY - streamingSource[1];
      const distance = Math.max(Math.hypot(dx, dy, streamingSource[2]), 0.5);
      const lod = Math.min(maxLod, Math.max(0, Math.floor(Math.log2(Math.max(1, distance / tileSize)))));
      const visualResolution = Math.max(8, 64 >> lod);
      const collisionResolution = Math.max(8, 32 >> Math.max(0, lod - 1));
      const erosion = terrainErosionSample(centerX, centerY, spec);
      const erosionStrength = erosion.effectiveErosionStrength;
      const heights = [
        terrainSurfaceZ(minX, minY, spec.amplitude, spec.frequency, spec.groundZ, spec.octaves, erosionStrength),
        terrainSurfaceZ(maxX, minY, spec.amplitude, spec.frequency, spec.groundZ, spec.octaves, erosionStrength),
        terrainSurfaceZ(minX, maxY, spec.amplitude, spec.frequency, spec.groundZ, spec.octaves, erosionStrength),
        terrainSurfaceZ(maxX, maxY, spec.amplitude, spec.frequency, spec.groundZ, spec.octaves, erosionStrength),
        terrainSurfaceZ(centerX, centerY, spec.amplitude, spec.frequency, spec.groundZ, spec.octaves, erosionStrength),
      ];
      const minZ = Math.min(...heights) - spec.amplitude * (0.20 + spec.caveStrength * 1.2);
      const maxZ = Math.max(...heights) + spec.amplitude * (0.20 + spec.overhangStrength * 0.9);
      const worldError = tileSize / visualResolution * (1 + erosionStrength * 0.35);
      const pixelsPerWorld = viewportHeightPx / (2 * Math.tan(fovY * 0.5) * distance);
      const screenErrorPx = worldError * pixelsPerWorld;
      const terrainCellHash = stableHash(["cell", sourceHash, erosion.erosionHash, x, y, lod, tileSize]);
      const visualCacheHash = stableHash(["visual", terrainCellHash, erosion.erosionHash, visualResolution, screenErrorPx]);
      const collisionCacheHash = stableHash(["collision", terrainCellHash, erosion.erosionHash, collisionResolution, minZ, maxZ]);
      const resident = screenErrorPx <= targetScreenErrorPx || distance <= tileSize * 1.6;
      tiles.push({
        tileX: x,
        tileY: y,
        lod,
        center: v3(centerX, centerY, (minZ + maxZ) * 0.5),
        boundsMin: v3(minX, minY, minZ),
        boundsMax: v3(maxX, maxY, maxZ),
        screenErrorPx,
        resident,
        erosionHash: erosion.erosionHash,
        erosion,
        terrainCellHash,
        visualCacheHash,
        collisionCacheHash,
        visualResolution,
        collisionResolution,
      });
    }
  }

  tiles.sort((a, b) => Number(b.resident) - Number(a.resident) || a.screenErrorPx - b.screenErrorPx);
  const manifestHash = stableHash(["manifest", sourceHash, streamingSource[0], streamingSource[1], streamingSource[2], ...tiles.map((tile) => tile.terrainCellHash)]);
  return {
    schema: "forge.banger.terrain_stream.v1",
    sourceHash,
    manifestHash,
    streamingSource,
    tileSize,
    targetScreenErrorPx,
    visualResidentCount: tiles.filter((tile) => tile.resident).length,
    collisionResidentCount: tiles.filter((tile) => tile.resident && tile.collisionResolution >= 16).length,
    tiles,
  };
}

/** Single tree : capsule trunk (bark) + sphere canopy (foliage),
 *  smin-blended. The smin auto-lerps bark→foliage at the join. */
export function tree(at: Vec3, height = 1.4, canopyRadius = 0.45): SdfOp[] {
  const trunkTop = v3(at[0], at[1], at[2] + height * 0.55);
  return [
    mat(MATERIALS.bark),
    { op: "capsule", a: at, b: trunkTop, radius: 0.07 },
    mat(MATERIALS.foliage),
    { op: "sphere", center: v3(at[0], at[1], at[2] + height), radius: canopyRadius },
    { op: "smin", k: 6.0 },
  ];
}

/** Box house with a sphere roof — brick walls, red roof, doorway carved. */
interface VegetationBranch {
  readonly base: Vec3;
  readonly tip: Vec3;
  readonly radius: number;
  readonly sourceRef: string;
}

function vecNorm(v: Vec3): Vec3 {
  const len = Math.hypot(v[0], v[1], v[2]) || 1;
  return v3(v[0] / len, v[1] / len, v[2] / len);
}

function vegetationBranches(origin: Vec3, spec: VegetationSpecies): VegetationBranch[] {
  const rng = seededRandom(spec.seed);
  const golden = Math.PI * (3 - Math.sqrt(5));
  const branches: VegetationBranch[] = [];
  const count = Math.max(0, Math.min(10, spec.branchCount));
  for (let i = 0; i < count; i += 1) {
    const u = count <= 1 ? 0.5 : i / (count - 1);
    const angle = golden * i + rng() * 0.62 + spec.seed * 0.013;
    const baseZ = origin[2] + spec.height * (0.24 + u * 0.48);
    const reach = spec.canopyRadius * (0.48 + rng() * 0.36) * (1.05 - u * 0.22);
    const lift = spec.height * (0.10 + rng() * 0.16);
    const base = v3(origin[0], origin[1], baseZ);
    const tip = v3(origin[0] + Math.cos(angle) * reach, origin[1] + Math.sin(angle) * reach, baseZ + lift);
    branches.push({
      base,
      tip,
      radius: spec.trunkRadius * (0.38 - u * 0.16),
      sourceRef: `branch_${i}`,
    });
  }
  return branches;
}

function seasonalFoliageMaterial(spec: VegetationSpecies) {
  const green = MATERIALS.foliage.color;
  const autumn = v3(0.68, 0.42, 0.16);
  const winter = v3(0.30, 0.38, 0.28);
  const warm = clamp01(spec.season * 1.35);
  const cold = clamp01((spec.season - 0.72) * 3.4);
  const color = v3(
    (green[0] * (1 - warm) + autumn[0] * warm) * (1 - cold) + winter[0] * cold,
    (green[1] * (1 - warm) + autumn[1] * warm) * (1 - cold) + winter[1] * cold,
    (green[2] * (1 - warm) + autumn[2] * warm) * (1 - cold) + winter[2] * cold,
  );
  return {
    color,
    roughness: 0.58 + spec.canopyDensity * 0.20,
    metallic: 0.0,
    normalDetailStrength: 0.26 + spec.leafScale * 0.08,
    decalWeight: 0.06 + spec.canopyDensity * 0.10,
  };
}

/** SDF-authoritative vegetation grammar. The LLM edits species parameters;
 *  the viewport receives a compact tree program, not thousands of vertices. */
export function vegetationCluster(at: Vec3, species: Partial<VegetationSpecies> = {}): SdfOp[] {
  const spec = vegetationSpec(species);
  const branches = vegetationBranches(at, spec);
  const leafCount = Math.min(14, spec.leafCount);
  const rng = seededRandom(spec.seed ^ 0x5eed);
  const ops: SdfOp[] = [
    mat(MATERIALS.bark),
    { op: "capsule", a: at, b: v3(at[0], at[1], at[2] + spec.height * 0.68), radius: spec.trunkRadius },
  ];

  for (const branch of branches) {
    ops.push({ op: "capsule", a: branch.base, b: branch.tip, radius: Math.max(0.015, branch.radius) });
    ops.push({ op: "union" });
  }

  ops.push(mat(seasonalFoliageMaterial(spec)));
  ops.push({ op: "sphere", center: v3(at[0], at[1], at[2] + spec.height), radius: spec.canopyRadius });
  ops.push({ op: "smin", k: 3.6 + spec.canopyDensity * 2.2 });

  for (let i = 0; i < leafCount; i += 1) {
    const branch = branches.length ? branches[i % branches.length]! : {
      base: v3(at[0], at[1], at[2] + spec.height * 0.45),
      tip: v3(at[0], at[1], at[2] + spec.height),
      radius: spec.trunkRadius * 0.2,
      sourceRef: "canopy",
    };
    const radial = vecNorm(v3(branch.tip[0] - at[0], branch.tip[1] - at[1], 0.22 + rng() * 0.34));
    const center = v3(
      branch.tip[0] + radial[0] * spec.canopyRadius * (0.10 + rng() * 0.22),
      branch.tip[1] + radial[1] * spec.canopyRadius * (0.10 + rng() * 0.22),
      branch.tip[2] + (rng() - 0.35) * spec.canopyRadius * 0.30,
    );
    const longAxis = spec.leafScale * (0.15 + rng() * 0.08);
    const shortAxis = spec.leafScale * (0.030 + rng() * 0.018);
    const swap = i % 2 === 0;
    ops.push({
      op: "roundedBox",
      center,
      halfExtents: swap ? v3(longAxis, shortAxis, 0.012) : v3(shortAxis, longAxis, 0.012),
      cornerRadius: 0.018,
    });
    ops.push({ op: "union" });
  }
  return ops;
}

export function vegetationClusterManifest(
  origin: Vec3 = v3(0, 0, 0),
  species: Partial<VegetationSpecies> = {},
  options: { sampleCount?: number; cellSize?: number; radiusCells?: number } = {},
): VegetationClusterManifest {
  const spec = vegetationSpec(species);
  const speciesHash = vegetationSpecHash(spec);
  const branches = vegetationBranches(origin, spec);
  const foliage = seasonalFoliageMaterial(spec);
  const sampleCount = Math.max(4, Math.min(24, Number(options.sampleCount) || 12));
  const boundsMin = v3(origin[0] - spec.canopyRadius * 1.45, origin[1] - spec.canopyRadius * 1.45, origin[2] - spec.trunkRadius);
  const boundsMax = v3(origin[0] + spec.canopyRadius * 1.45, origin[1] + spec.canopyRadius * 1.45, origin[2] + spec.height + spec.canopyRadius * 1.15);
  const canopySamples: VegetationCanopySample[] = [];
  const surfels: VegetationSurfel[] = [];
  const rng = seededRandom(spec.seed ^ 0x71af);
  const golden = Math.PI * (3 - Math.sqrt(5));

  for (let i = 0; i < sampleCount; i += 1) {
    const u = (i + 0.5) / sampleCount;
    const angle = i * golden + spec.seed * 0.017;
    const ring = Math.sqrt(u) * spec.canopyRadius;
    const z = origin[2] + spec.height + (rng() - 0.5) * spec.canopyRadius * 0.9;
    const position = v3(origin[0] + Math.cos(angle) * ring, origin[1] + Math.sin(angle) * ring, z);
    const sourceRef = i < branches.length ? branches[i]!.sourceRef : `leaf_${i}`;
    const sampleHash = stableHash(["canopy_sample", speciesHash, i, position[0], position[1], position[2], spec.canopyDensity], "vegetation");
    canopySamples.push({
      position,
      radius: spec.canopyRadius * (0.18 + rng() * 0.18),
      density: clamp01(spec.canopyDensity * (0.68 + rng() * 0.45)),
      sourceRef,
      sampleHash,
    });
    const n = vecNorm(v3(position[0] - origin[0], position[1] - origin[1], 0.35 + rng() * 0.55));
    const surfelHash = stableHash(["surfel", sampleHash, n[0], n[1], n[2], foliage.color[0], foliage.color[1], foliage.color[2]], "vegetation");
    surfels.push({
      position,
      normal: n,
      radius: spec.leafScale * (0.045 + rng() * 0.055),
      albedo: foliage.color,
      roughness: foliage.roughness,
      sourceRef,
      surfelHash,
    });
  }

  const cellSize = Math.max(2, Number(options.cellSize) || spec.canopyRadius * 5.0);
  const radiusCells = Math.max(1, Math.min(4, Number(options.radiusCells) || 1));
  const streamClusters: VegetationStreamCluster[] = [];
  const cx = Math.floor(origin[0] / cellSize);
  const cy = Math.floor(origin[1] / cellSize);
  for (let y = cy - radiusCells; y <= cy + radiusCells; y += 1) {
    for (let x = cx - radiusCells; x <= cx + radiusCells; x += 1) {
      const center = v3((x + 0.5) * cellSize, (y + 0.5) * cellSize, origin[2] + spec.height * 0.5);
      const dist = Math.hypot(center[0] - origin[0], center[1] - origin[1]);
      const lod: VegetationStreamCluster["lod"] = dist < cellSize * 0.85 ? "near-sdf" : dist < cellSize * 1.55 ? "canopy-density" : "surfel-cloud";
      const clusterHash = stableHash(["stream_cluster", speciesHash, x, y, lod, center[0], center[1], spec.windStrength], "vegetation");
      streamClusters.push({
        cellX: x,
        cellY: y,
        lod,
        center,
        resident: dist <= cellSize * (radiusCells + 0.35),
        clusterHash,
        cacheHash: stableHash(["stream_cache", clusterHash, canopySamples.length, surfels.length], "vegetation"),
      });
    }
  }

  const canopyDensityHash = stableHash(["canopy_density", speciesHash, ...canopySamples.map((s) => s.sampleHash)], "vegetation");
  const surfelCloudHash = stableHash(["surfel_cloud", speciesHash, ...surfels.map((s) => s.surfelHash)], "vegetation");
  const windFieldHash = stableHash(["wind_field", speciesHash, spec.windStrength, spec.height, spec.canopyRadius, spec.seed], "vegetation");
  const streamHash = stableHash(["stream", speciesHash, ...streamClusters.map((c) => c.cacheHash)], "vegetation");
  const sourceHash = stableHash(["vegetation_source", speciesHash, origin[0], origin[1], origin[2]], "vegetation");
  const clusterHash = stableHash(["vegetation_cluster", sourceHash, canopyDensityHash, surfelCloudHash, windFieldHash, streamHash], "vegetation");

  return {
    schema: "forge.banger.vegetation_cluster.v1",
    speciesHash,
    clusterHash,
    sourceHash,
    canopyDensityHash,
    surfelCloudHash,
    windFieldHash,
    streamHash,
    origin,
    boundsMin,
    boundsMax,
    nearOpBudget: vegetationCluster(origin, spec).length,
    canopySamples,
    surfels,
    streamClusters,
  };
}

export function house(at: Vec3, size = 1.0): SdfOp[] {
  const half = size * 0.5;
  const wallZ = at[2] + half;
  return [
    mat(MATERIALS.brick),
    { op: "roundedBox", center: v3(at[0], at[1], wallZ), halfExtents: v3(half, half, half), cornerRadius: 0.04 },
    mat(MATERIALS.roof),
    { op: "sphere", center: v3(at[0], at[1], wallZ + half * 0.85), radius: half * 0.7 },
    { op: "union" },
    // doorway carved out of the front face — diff keeps wall material
    { op: "box", center: v3(at[0], at[1] - half, wallZ - half * 0.35), halfExtents: v3(half * 0.18, half * 0.05, half * 0.3) },
    { op: "diff" },
  ];
}

/** Terrain-blended boulder. Kept as a single primitive so landscapes stay
 *  under the compact 128-op viewport budget. */
export function boulder(at: Vec3, radius = 0.45): SdfOp[] {
  return [
    mat(MATERIALS.stone),
    { op: "roundedBox", center: v3(at[0], at[1], at[2] + radius * 0.55), halfExtents: v3(radius * 0.9, radius * 0.7, radius * 0.55), cornerRadius: radius * 0.28 },
  ];
}

/** Mud/stone road patch authored as SDF, then smooth-blended with terrain. */
export function roadPatch(at: Vec3, length = 10.0, width = 0.7): SdfOp[] {
  return [
    mat({ color: v3(0.19, 0.16, 0.12), roughness: 0.96, metallic: 0.0, normalDetailStrength: 0.70, decalWeight: 0.62 }),
    { op: "roundedBox", center: v3(at[0], at[1], at[2] + 0.035), halfExtents: v3(length * 0.5, width * 0.5, 0.055), cornerRadius: 0.09 },
  ];
}

export function waterRunoffPatch(at: Vec3, length = 6.0, width = 0.35): SdfOp[] {
  return [
    mat({ color: v3(0.07, 0.12, 0.14), roughness: 0.06, metallic: 0.0, normalDetailStrength: 0.03, decalWeight: 0.02 }),
    { op: "roundedBox", center: v3(at[0], at[1], at[2] + 0.024), halfExtents: v3(length * 0.5, width * 0.5, 0.035), cornerRadius: 0.13 },
  ];
}

/** Compact ground micro-detail cluster: ruts, footprints, puddle,
 *  pebbles/debris and short grass, all ordinary SDF ops. */
export function groundMicroDetails(at: Vec3): SdfOp[] {
  return [
    mat({ color: v3(0.13, 0.10, 0.075), roughness: 0.98, metallic: 0.0, normalDetailStrength: 0.88, decalWeight: 0.76 }),
    { op: "roundedBox", center: v3(at[0] - 0.54, at[1] - 0.10, at[2] + 0.022), halfExtents: v3(1.45, 0.055, 0.038), cornerRadius: 0.045 },
    { op: "roundedBox", center: v3(at[0] + 0.54, at[1] + 0.08, at[2] + 0.022), halfExtents: v3(1.45, 0.055, 0.038), cornerRadius: 0.045 },
    { op: "union" },
    { op: "roundedBox", center: v3(at[0] - 0.23, at[1] + 0.50, at[2] + 0.026), halfExtents: v3(0.18, 0.075, 0.034), cornerRadius: 0.055 },
    { op: "union" },
    { op: "roundedBox", center: v3(at[0] + 0.06, at[1] + 0.68, at[2] + 0.026), halfExtents: v3(0.18, 0.075, 0.034), cornerRadius: 0.055 },
    { op: "union" },
    mat({ color: v3(0.075, 0.095, 0.105), roughness: 0.08, metallic: 0.0, normalDetailStrength: 0.02, decalWeight: 0.04 }),
    { op: "roundedBox", center: v3(at[0] + 0.95, at[1] - 0.42, at[2] + 0.018), halfExtents: v3(0.42, 0.24, 0.022), cornerRadius: 0.11 },
    { op: "union" },
    mat(MATERIALS.stone),
    { op: "roundedBox", center: v3(at[0] - 1.00, at[1] + 0.34, at[2] + 0.11), halfExtents: v3(0.12, 0.09, 0.08), cornerRadius: 0.045 },
    { op: "sphere", center: v3(at[0] + 1.22, at[1] + 0.22, at[2] + 0.08), radius: 0.09 },
    { op: "union" },
    { op: "roundedBox", center: v3(at[0] - 1.34, at[1] - 0.38, at[2] + 0.075), halfExtents: v3(0.18, 0.055, 0.045), cornerRadius: 0.025 },
    { op: "union" },
    { op: "union" },
    mat(MATERIALS.grass),
    { op: "capsule", a: v3(at[0] + 1.35, at[1] - 0.08, at[2] + 0.02), b: v3(at[0] + 1.55, at[1] - 0.02, at[2] + 0.22), radius: 0.035 },
    { op: "union" },
  ];
}

/** Simple vehicle : painted body + chrome wheels. Type tweaks the
 *  proportions (car = wide low, truck = tall, hover = floating no wheels). */
export function vehicle(at: Vec3, kind: "car" | "truck" | "hover" = "car"): SdfOp[] {
  const cfg = kind === "truck"
    ? { body: v3(0.55, 1.10, 0.35), wheelRadius: 0.18, wheelInset: 0.42, paint: MATERIALS.metal }
    : kind === "hover"
    ? { body: v3(0.50, 1.00, 0.22), wheelRadius: 0,    wheelInset: 0,    paint: MATERIALS.chrome }
    : { body: v3(0.45, 0.95, 0.20), wheelRadius: 0.13, wheelInset: 0.34, paint: MATERIALS.paint_red };
  const bodyZ = at[2] + cfg.body[2] + cfg.wheelRadius;
  const ops: SdfOp[] = [
    mat(cfg.paint),
    { op: "roundedBox", center: v3(at[0], at[1], bodyZ), halfExtents: cfg.body, cornerRadius: 0.06 },
  ];
  if (cfg.wheelRadius > 0) {
    const wx = cfg.body[0] * 0.9, wy = cfg.body[1] * 0.7;
    const wz = at[2] + cfg.wheelRadius;
    const wheelPoses: [Vec3, Vec3][] = [
      [v3(at[0] - wx, at[1] - wy, wz - cfg.wheelInset), v3(at[0] - wx, at[1] - wy, wz + cfg.wheelInset)],
      [v3(at[0] + wx, at[1] - wy, wz - cfg.wheelInset), v3(at[0] + wx, at[1] - wy, wz + cfg.wheelInset)],
      [v3(at[0] - wx, at[1] + wy, wz - cfg.wheelInset), v3(at[0] - wx, at[1] + wy, wz + cfg.wheelInset)],
      [v3(at[0] + wx, at[1] + wy, wz - cfg.wheelInset), v3(at[0] + wx, at[1] + wy, wz + cfg.wheelInset)],
    ];
    ops.push(mat(MATERIALS.rubber));
    for (const [a, b] of wheelPoses) {
      ops.push({ op: "capsule", a, b, radius: cfg.wheelRadius });
      ops.push({ op: "union" });
    }
  }
  return ops;
}

/** Forest via OP_REPEAT — one tree primitive becomes an infinite grid
 *  in the XY plane. The bounds parameter is purely informational here
 *  (REPEAT itself is infinite ; render distance is bounded by fog +
 *  the raymarch far plane). */
export function forest(spacing = 4.0, treeHeight = 1.8, canopyRadius = 0.55): SdfOp[] {
  return [
    { op: "repeat", period: v3(spacing, spacing, 0) },
    ...vegetationCluster(v3(0, 0, 0), {
      height: treeHeight,
      canopyRadius,
      branchCount: 4,
      leafCount: 6,
      seed: 9041,
      canopyDensity: 0.62,
    }),
    { op: "repeat", period: v3(0, 0, 0) }, // reset
  ];
}

/** Wall : infinite line of rounded bricks along the X axis. */
export function wall(spacing = 0.6, height = 1.2): SdfOp[] {
  return [
    { op: "repeat", period: v3(spacing, 0, 0) },
    { op: "roundedBox", center: v3(0, 0, height * 0.5), halfExtents: v3(spacing * 0.45, 0.18, height * 0.5), cornerRadius: 0.04 },
    { op: "repeat", period: v3(0, 0, 0) },
  ];
}

/** Distributed scatter of one composite primitive (e.g. houses) over a
 *  bounded XY rectangle, using a seeded RNG so the layout is
 *  reproducible. Useful when REPEAT is too uniform. */
export function scatter(
  bounds: { min: Vec3; max: Vec3 },
  count: number,
  emit: (at: Vec3) => SdfOp[],
  seed = 1337,
): SdfOp[] {
  const rng = seededRandom(seed);
  const ops: SdfOp[] = [];
  let placed = 0;
  for (let i = 0; i < count; i += 1) {
    const at: Vec3 = [
      bounds.min[0] + (bounds.max[0] - bounds.min[0]) * rng(),
      bounds.min[1] + (bounds.max[1] - bounds.min[1]) * rng(),
      bounds.min[2] + (bounds.max[2] - bounds.min[2]) * rng(),
    ];
    const sub = emit(at);
    if (!sub.length) continue;
    ops.push(...sub);
    if (placed > 0) ops.push({ op: "union" });
    placed += 1;
  }
  return ops;
}

/** INGEN-native showcase scene — uses ONLY ops the WebGPU raymarcher
 *  supports (sphere / capsule / torus / smin / union ; no terrain /
 *  material / repeat). A smin-blended mountain, scattered boulders, a ring
 *  of trees, a torus arch and floating spheres — built to exercise the new
 *  lighting (soft shadows, AO, GI radiance cache, sky). ~52 ops, well under
 *  the 128-op budget. Objects rest on the z=0 grid floor. */
export function showcaseScene(): SdfOp[] {
  const ops: SdfOp[] = [];

  // Central mountain : smin-blended spheres rising from the grid (stone).
  ops.push(mat(MATERIALS.stone));
  ops.push({ op: "sphere", center: v3(0, 0, 1.5), radius: 4.2 });
  ops.push({ op: "sphere", center: v3(3.6, 2.2, 0.4), radius: 2.8 });
  ops.push({ op: "smin", k: 2.4 });
  ops.push({ op: "sphere", center: v3(-3.2, -2.4, 0.7), radius: 3.0 });
  ops.push({ op: "smin", k: 2.4 });
  ops.push({ op: "sphere", center: v3(-1.2, 3.4, 1.0), radius: 2.2 });
  ops.push({ op: "smin", k: 2.0 });
  ops.push({ op: "sphere", center: v3(1.6, -3.2, 0.6), radius: 2.0 });
  ops.push({ op: "smin", k: 2.0 });

  // Boulders scattered around the base.
  const boulders: { c: Vec3; r: number }[] = [
    { c: v3(7, -4.5, 0.66), r: 1.2 },
    { c: v3(-7, 4.5, 0.82), r: 1.5 },
    { c: v3(5.5, 5.5, 0.55), r: 1.0 },
    { c: v3(-5.5, -5.5, 0.72), r: 1.3 },
    { c: v3(8, 1, 0.50), r: 0.9 },
  ];
  for (const b of boulders) {
    ops.push({ op: "sphere", center: b.c, radius: b.r });
    ops.push({ op: "union" });
  }

  // Ring of trees (capsule trunk + sphere canopy, smin-blended).
  const treeCount = 7;
  for (let i = 0; i < treeCount; i += 1) {
    const a = (i / treeCount) * Math.PI * 2;
    const x = Math.cos(a) * 9.5;
    const y = Math.sin(a) * 9.5;
    const h = 1.6 + (i % 3) * 0.3;
    ops.push(mat(MATERIALS.bark));
    ops.push({ op: "capsule", a: v3(x, y, 0), b: v3(x, y, h), radius: 0.12 });
    ops.push(mat(MATERIALS.foliage));
    ops.push({ op: "sphere", center: v3(x, y, h + 0.5), radius: 0.7 });
    ops.push({ op: "smin", k: 5.0 });
    ops.push({ op: "union" });
  }

  // Chrome torus arch + a couple of painted/metal floating spheres — these
  // exercise the metallic-roughness path (sky reflection, GGX highlight).
  ops.push(mat(MATERIALS.chrome));
  ops.push({ op: "torus", center: v3(0, -8.5, 3.0), majorRadius: 2.6, minorRadius: 0.35 });
  ops.push({ op: "union" });
  ops.push(mat(MATERIALS.paint_red));
  ops.push({ op: "sphere", center: v3(4, 0, 5.5), radius: 0.9 });
  ops.push({ op: "union" });
  ops.push(mat(MATERIALS.metal));
  ops.push({ op: "sphere", center: v3(-4, 1, 4.5), radius: 0.7 });
  ops.push({ op: "union" });

  return ops;
}

/** Atmospheric world preset : terrain + scattered trees + a few houses.
 *  Returns a complete SdfOp[] ready for __forgeBangerSetScene. The
 *  caller still has to enable sky / fog separately (those are global
 *  toggles, not part of the scene tree). */
export function defaultLandscape(seed = 42): SdfOp[] {
  const terrainSpec = DEFAULT_TERRAIN_SPEC;
  const erosionStrength = terrainErosionStrength(terrainSpec);
  const onTerrain = (at: Vec3, sink = 0): Vec3 => v3(
    at[0],
    at[1],
    terrainSurfaceZ(at[0], at[1], terrainSpec.amplitude, terrainSpec.frequency, terrainSpec.groundZ, terrainSpec.octaves, erosionStrength) + sink,
  );

  return [
    ...terrain(terrainSpec.amplitude, terrainSpec.frequency, terrainSpec.groundZ, terrainSpec.octaves, {
      caveStrength: terrainSpec.caveStrength,
      overhangStrength: terrainSpec.overhangStrength,
      erosionStrength,
    }),
    ...scatter(
      { min: v3(-8, -8, 0.6), max: v3(8, 8, 0.6) },
      9,
      (at) => tree(onTerrain(at, -0.08), 1.4 + (seed % 7) * 0.05, 0.4),
      seed,
    ),
    { op: "smin", k: 0.38 },
    ...roadPatch(onTerrain(v3(0, -1.2, 0), -0.02), 11.5, 0.75),
    { op: "smin", k: 0.18 },
    ...groundMicroDetails(onTerrain(v3(1.2, -1.55, 0), -0.03)),
    { op: "smin", k: 0.12 },
    ...waterRunoffPatch(onTerrain(v3(-1.3, 1.35, 0), -0.025), 6.8, 0.42),
    { op: "smin", k: 0.10 },
    ...scatter(
      { min: v3(-7, -6, 0), max: v3(7, 6, 0) },
      5,
      (at) => boulder(onTerrain(at, -0.06), 0.30 + ((Math.abs(at[0] * 13.1 + at[1] * 7.7) % 1) * 0.28)),
      seed + 7,
    ),
    { op: "smin", k: 0.26 },
    ...scatter(
      { min: v3(-6, -6, 0), max: v3(6, 6, 0) },
      3,
      (at) => house(onTerrain(at, -0.03), 1.0),
      seed + 1,
    ),
    { op: "smin", k: 0.20 },
  ];
}
