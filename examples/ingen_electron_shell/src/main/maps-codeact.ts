import { createHash } from "node:crypto";
import {
  BRAIN_MAPS_COMMAND,
  BRAIN_MAPS_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const MAPS_COMMAND = BRAIN_MAPS_COMMAND;
export const MAPS_RESULT_SCHEMA = BRAIN_MAPS_RESULT_SCHEMA;
export const MAPS_TEMPLATE_RESULT_SCHEMA = "forge.webexplorer.maps.template_result.v1";
export const MAPS_DEFAULT_TARGET = "default_google_earth_view";
export const BANGER_MAPS_ROUTE = "banger://maps-sphere";
export const BANGER_MAPS_TILESET_PROVIDER = "google_photorealistic_3d_tiles";
export const BANGER_MAPS_RENDERER_CONTRACT = "forge.banger.google_photorealistic_tiles_config.v1";
export const GOOGLE_EARTH_DEFAULT_URL =
  "https://earth.google.com/web/@48.56768844,29.71746065,-845.33787847a,4386237.90060282d,35y,64.15278862h,59.46514162t,0.00000084r/data=CgRCAggBOgMKATBCAggASg0I____________ARAA";

const MAX_TARGET_CHARS = 240;
const DEFAULT_EARTH_DISTANCE_METERS = "4386237.90060282";
const DEFAULT_EARTH_HEADING_DEGREES = "64.15278862";
const DEFAULT_EARTH_TILT_DEGREES = "59.46514162";

export type MapsSearchKind =
  | "auto"
  | "gps"
  | "place"
  | "address"
  | "city"
  | "country"
  | "continent"
  | "river"
  | "mountain"
  | "street"
  | "avenue"
  | "public_place"
  | "business"
  | "administration";

export type MapsGeocodeProvider = "google_places" | "google_geocoding" | "auto";

export interface MapsCodeActRequest {
  schema: "forge.webexplorer.maps.request.v1";
  command: typeof MAPS_COMMAND;
  templateProofHash: string;
  searchKind: MapsSearchKind;
  target: string;
  query: string;
  placeId: string;
  keywords: string[];
  latitude?: number;
  longitude?: number;
  countryHint: string;
  regionHint: string;
  language: "auto" | "fr" | "en";
  geocodeProvider: MapsGeocodeProvider;
  url: string;
  source: "explicit_codeact";
  proofHash: string;
}

export interface MapsTemplateResult {
  schema: typeof MAPS_TEMPLATE_RESULT_SCHEMA;
  command: typeof MAPS_COMMAND;
  status: "template";
  reason: "empty_command" | "template_required";
  template: string;
  allowedValues: {
    searchKind: MapsSearchKind[];
    language: MapsCodeActRequest["language"][];
    geocodeProvider: MapsGeocodeProvider[];
    engine: ["cesiumjs"];
    tileset: ["google_photorealistic_3d_tiles"];
    geocodePolicy: ["explicit_query_only"];
    openMode: ["split_native_canvas"];
    output: ["conversation_and_navigation", "navigation_only"];
    safety: ["no_device_location_no_home_location"];
  };
  proofHash: string;
}

export type MapsCodeAct =
  | { kind: "template"; result: MapsTemplateResult }
  | { kind: "request"; request: MapsCodeActRequest };

const MAPS_SEARCH_KINDS: MapsSearchKind[] = [
  "auto",
  "gps",
  "place",
  "address",
  "city",
  "country",
  "continent",
  "river",
  "mountain",
  "street",
  "avenue",
  "public_place",
  "business",
  "administration"
];
const MAPS_LANGUAGES: MapsCodeActRequest["language"][] = ["auto", "fr", "en"];
const MAPS_GEOCODE_PROVIDERS: MapsGeocodeProvider[] = ["google_places", "google_geocoding", "auto"];

export function mapsTemplateProofHash(): string {
  return stableHash({
    command: MAPS_COMMAND,
    schema: MAPS_TEMPLATE_RESULT_SCHEMA,
    fields: [
      "template_proof_hash",
      "search_kind",
      "query",
      "target",
      "place_id",
      "latitude",
      "longitude",
      "country_hint",
      "region_hint",
      "language",
      "geocode_provider",
      "engine",
      "tileset",
      "geocode_policy",
      "open_mode",
      "output",
      "safety"
    ],
    fixedRuntime: {
      camera: "host_fixed",
      tileset: BANGER_MAPS_TILESET_PROVIDER,
      rendererContract: BANGER_MAPS_RENDERER_CONTRACT
    },
    allowedValues: mapsTemplateAllowedValues()
  });
}

export function mapsTemplateResult(reason: MapsTemplateResult["reason"] = "empty_command"): MapsTemplateResult {
  const templateProofHash = mapsTemplateProofHash();
  const result: MapsTemplateResult = {
    schema: MAPS_TEMPLATE_RESULT_SCHEMA,
    command: MAPS_COMMAND,
    status: "template",
    reason,
    template: [
      `${MAPS_COMMAND}`,
      `template_proof_hash="sha256:${templateProofHash}"`,
      'search_kind="auto|gps|place|address|city|country|continent|river|mountain|street|avenue|public_place|business|administration"',
      'query=""',
      'target=""',
      'place_id=""',
      'latitude=""',
      'longitude=""',
      'country_hint=""',
      'region_hint=""',
      'language="auto|fr|en"',
      'geocode_provider="google_places|google_geocoding|auto"',
      'engine="cesiumjs"',
      'tileset="google_photorealistic_3d_tiles"',
      'geocode_policy="explicit_query_only"',
      'open_mode="split_native_canvas"',
      'output="conversation_and_navigation|navigation_only"',
      'safety="no_device_location_no_home_location"'
    ].join("\n"),
    allowedValues: mapsTemplateAllowedValues(),
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

export function renderMapsTemplateResult(result: MapsTemplateResult): string {
  return [
    "MAPS_TEMPLATE_RESULT",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `reason=${result.reason}`,
    `template_proof_hash=sha256:${mapsTemplateProofHash()}`,
    `allowed_values=${JSON.stringify(result.allowedValues)}`,
    "template:",
    indentBlock(result.template, "  "),
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

export function readMapsCodeAct(input: string): MapsCodeAct | undefined {
  const trimmed = mapsCodeActText(input).trim();
  if (!trimmed) {
    return undefined;
  }
  const body = trimmed.slice(MAPS_COMMAND.length).trim();
  if (!body) {
    return { kind: "template", result: mapsTemplateResult("empty_command") };
  }
  const fields = parseTemplateFields(body);
  if (!templateProofHashAccepted(fields.get("template_proof_hash") ?? fields.get("templateProofHash"))) {
    return { kind: "template", result: mapsTemplateResult("template_required") };
  }
  const request = parseMapsCodeAct(trimmed);
  if (!request) {
    return { kind: "template", result: mapsTemplateResult("template_required") };
  }
  return { kind: "request", request };
}

export function parseMapsCodeAct(input: string): MapsCodeActRequest | undefined {
  const trimmed = mapsCodeActText(input).trim();
  if (!trimmed || !readMapsCommand(trimmed)) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(MAPS_COMMAND.length).trim());
  const query = clampText(fields.get("query") ?? fields.get("q") ?? fields.get("target") ?? MAPS_DEFAULT_TARGET, MAX_TARGET_CHARS);
  const target = clampText(
    fields.get("target") ?? query,
    MAX_TARGET_CHARS
  );
  const placeId = clampText(fields.get("place_id") ?? fields.get("placeId") ?? "", MAX_TARGET_CHARS);
  const latitude = readCoordinate(fields.get("latitude") ?? fields.get("lat"), -90, 90);
  const longitude = readCoordinate(fields.get("longitude") ?? fields.get("lon") ?? fields.get("lng"), -180, 180);
  return createMapsCodeActRequest({
    command: MAPS_COMMAND,
    templateProofHash: normalizeProofHash(fields.get("template_proof_hash") ?? fields.get("templateProofHash")),
    searchKind: readChoice(fields.get("search_kind") ?? fields.get("searchKind"), MAPS_SEARCH_KINDS, coordinatesPresent(latitude, longitude) ? "gps" : "auto"),
    target,
    query,
    placeId,
    keywords: parseKeywords(fields.get("keywords")),
    latitude,
    longitude,
    countryHint: clampText(fields.get("country_hint") ?? fields.get("countryHint") ?? "", 80),
    regionHint: clampText(fields.get("region_hint") ?? fields.get("regionHint") ?? "", 120),
    language: readChoice(fields.get("language") ?? fields.get("locale"), MAPS_LANGUAGES, "auto"),
    geocodeProvider: readChoice(fields.get("geocode_provider") ?? fields.get("geocodeProvider"), MAPS_GEOCODE_PROVIDERS, "google_places"),
    source: "explicit_codeact"
  });
}

export function extractMapsCodeAct(input: string): MapsCodeActRequest | undefined {
  const explicit = readMapsCodeAct(input);
  if (explicit?.kind === "request") {
    return explicit.request;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => Boolean(readMapsCommand(item)));
  const lineCodeAct = line ? readMapsCodeAct(line) : undefined;
  return lineCodeAct?.kind === "request" ? lineCodeAct.request : undefined;
}

export function renderMapsCodeActResult(request: MapsCodeActRequest): string {
  return [
    "MAPS_RESULT",
    `schema=${MAPS_RESULT_SCHEMA}`,
    `command=${request.command}`,
    `status=ok`,
    `engine=cesiumjs`,
    `tileset=${BANGER_MAPS_TILESET_PROVIDER}`,
    `search_kind=${request.searchKind}`,
    `query=${JSON.stringify(request.query)}`,
    `target=${JSON.stringify(request.target)}`,
    `place_id=${JSON.stringify(request.placeId)}`,
    `latitude=${request.latitude ?? ""}`,
    `longitude=${request.longitude ?? ""}`,
    `country_hint=${JSON.stringify(request.countryHint)}`,
    `region_hint=${JSON.stringify(request.regionHint)}`,
    `language=${request.language}`,
    `geocode_provider=${request.geocodeProvider}`,
    `route=${JSON.stringify(BANGER_MAPS_ROUTE)}`,
    `visual_target=banger_native_maps_sphere`,
    `tileset_provider=${BANGER_MAPS_TILESET_PROVIDER}`,
    `renderer_contract=${BANGER_MAPS_RENDERER_CONTRACT}`,
    `viewer_contract=forge.maps.cesium.photorealistic_tiles.v1`,
    `source=${request.source}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

export function createMapsCodeActRequest(
  params: Omit<MapsCodeActRequest, "schema" | "url" | "proofHash" | "templateProofHash" | "searchKind" | "placeId" | "countryHint" | "regionHint" | "language" | "geocodeProvider"> &
    Partial<Pick<MapsCodeActRequest, "templateProofHash" | "searchKind" | "placeId" | "countryHint" | "regionHint" | "language" | "geocodeProvider">>
): MapsCodeActRequest {
  const request: MapsCodeActRequest = {
    schema: "forge.webexplorer.maps.request.v1",
    ...params,
    placeId: params.placeId ?? "",
    templateProofHash: params.templateProofHash ?? "",
    searchKind: params.searchKind ?? (coordinatesPresent(params.latitude, params.longitude) ? "gps" : "auto"),
    countryHint: params.countryHint ?? "",
    regionHint: params.regionHint ?? "",
    language: params.language ?? "auto",
    geocodeProvider: params.geocodeProvider ?? "google_places",
    url: googleEarthUrl(params.latitude, params.longitude),
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

export function googleEarthUrl(latitude?: number, longitude?: number): string {
  if (typeof latitude !== "number" || typeof longitude !== "number") {
    return GOOGLE_EARTH_DEFAULT_URL;
  }
  return [
    "https://earth.google.com/web/@",
    `${latitude.toFixed(8)},${longitude.toFixed(8)},0a,`,
    `${DEFAULT_EARTH_DISTANCE_METERS}d,35y,`,
    `${DEFAULT_EARTH_HEADING_DEGREES}h,${DEFAULT_EARTH_TILT_DEGREES}t,0r`
  ].join("");
}

function readMapsCommand(value: string): typeof MAPS_COMMAND | undefined {
  const trimmed = value.trim();
  const lower = trimmed.toLowerCase();
  return lower === MAPS_COMMAND || lower.startsWith(`${MAPS_COMMAND} `) ? MAPS_COMMAND : undefined;
}

function mapsTemplateAllowedValues(): MapsTemplateResult["allowedValues"] {
  return {
    searchKind: MAPS_SEARCH_KINDS,
    language: MAPS_LANGUAGES,
    geocodeProvider: MAPS_GEOCODE_PROVIDERS,
    engine: ["cesiumjs"],
    tileset: ["google_photorealistic_3d_tiles"],
    geocodePolicy: ["explicit_query_only"],
    openMode: ["split_native_canvas"],
    output: ["conversation_and_navigation", "navigation_only"],
    safety: ["no_device_location_no_home_location"]
  };
}

function mapsCodeActText(input: string): string {
  const commandIndex = input.toLowerCase().indexOf(MAPS_COMMAND);
  if (commandIndex < 0) {
    return "";
  }
  const lines = input.slice(commandIndex).split(/\r?\n/);
  const block: string[] = [];
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]?.trim() ?? "";
    if (index > 0 && (!line || line.startsWith("/") || /^[A-Z_]+_RESULT\b/.test(line))) {
      break;
    }
    block.push(line);
  }
  return block.join("\n");
}

function templateProofHashAccepted(value: unknown): boolean {
  return normalizeProofHash(value) === mapsTemplateProofHash();
}

function normalizeProofHash(value: unknown): string {
  return String(value ?? "").trim().replace(/^sha256:/i, "");
}

function coordinatesPresent(latitude: number | undefined, longitude: number | undefined): boolean {
  return typeof latitude === "number" && typeof longitude === "number";
}

function readChoice<T extends string>(value: unknown, choices: readonly T[], fallback: T): T {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return choices.find((choice) => choice === normalized) ?? fallback;
}

function parseKeywords(value: unknown): string[] {
  if (typeof value !== "string" || !value.trim()) {
    return [];
  }
  return value.split(/[,|;\n]+/).map((item) => clampText(item, 80)).filter(Boolean).slice(0, 12);
}

function indentBlock(value: string, prefix: string): string {
  return value.split(/\r?\n/).map((line) => `${prefix}${line}`).join("\n");
}

function readCoordinate(value: unknown, min: number, max: number): number | undefined {
  if (typeof value !== "string" || !value.trim()) {
    return undefined;
  }
  const parsed = Number(value.trim().replace(",", "."));
  return Number.isFinite(parsed) && parsed >= min && parsed <= max ? parsed : undefined;
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    const value = (match[2] ?? match[3] ?? match[4] ?? "").trim();
    fields.set(key, value);
  }
  return fields;
}

function clampText(text: string, maxChars: number): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3)).trimEnd()}...`;
}

function stableHash(value: unknown): string {
  return createHash("sha256").update(stableJson(value)).digest("hex");
}

function stableJson(value: unknown): string {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  const object = value as Record<string, unknown>;
  return `{${Object.keys(object).sort().map((key) => `${JSON.stringify(key)}:${stableJson(object[key])}`).join(",")}}`;
}
