import { createHash } from "node:crypto";
import {
  BRAIN_MAPS_COMMAND,
  BRAIN_MAPS_RESULT_SCHEMA
} from "../shared/ipc-contract.js";

export const MAPS_COMMAND = BRAIN_MAPS_COMMAND;
export const MAPS_RESULT_SCHEMA = BRAIN_MAPS_RESULT_SCHEMA;
export const GOOGLE_EARTH_DEFAULT_URL =
  "https://earth.google.com/web/@48.56768844,29.71746065,-845.33787847a,4386237.90060282d,35y,64.15278862h,59.46514162t,0.00000084r/data=CgRCAggBOgMKATBCAggASg0I____________ARAA";

const MAX_TARGET_CHARS = 240;
const DEFAULT_EARTH_DISTANCE_METERS = "4386237.90060282";
const DEFAULT_EARTH_HEADING_DEGREES = "64.15278862";
const DEFAULT_EARTH_TILT_DEGREES = "59.46514162";

export interface MapsCodeActRequest {
  schema: "forge.webexplorer.maps.request.v1";
  command: typeof MAPS_COMMAND;
  target: string;
  query: string;
  keywords: string[];
  latitude?: number;
  longitude?: number;
  url: string;
  source: "explicit_codeact";
  proofHash: string;
}

export function parseMapsCodeAct(input: string): MapsCodeActRequest | undefined {
  const trimmed = input.trim();
  if (!readMapsCommand(trimmed)) {
    return undefined;
  }
  const fields = parseTemplateFields(trimmed.slice(MAPS_COMMAND.length).trim());
  const target = clampText(
    fields.get("target") ?? fields.get("query") ?? fields.get("q") ?? "default_google_earth_view",
    MAX_TARGET_CHARS
  );
  const latitude = readCoordinate(fields.get("latitude") ?? fields.get("lat"), -90, 90);
  const longitude = readCoordinate(fields.get("longitude") ?? fields.get("lon") ?? fields.get("lng"), -180, 180);
  return buildMapsCodeActRequest({
    command: MAPS_COMMAND,
    target,
    query: target,
    keywords: [],
    latitude,
    longitude,
    source: "explicit_codeact"
  });
}

export function extractMapsCodeAct(input: string): MapsCodeActRequest | undefined {
  const explicit = parseMapsCodeAct(input);
  if (explicit) {
    return explicit;
  }
  const line = input
    .split(/\r?\n/)
    .map((item) => item.trim())
    .find((item) => Boolean(readMapsCommand(item)));
  return line ? parseMapsCodeAct(line) : undefined;
}

export function renderMapsCodeActResult(request: MapsCodeActRequest): string {
  return [
    "MAPS_RESULT",
    `schema=${MAPS_RESULT_SCHEMA}`,
    `command=${request.command}`,
    `target=${JSON.stringify(request.target)}`,
    `latitude=${request.latitude ?? ""}`,
    `longitude=${request.longitude ?? ""}`,
    `url=${JSON.stringify(request.url)}`,
    `source=${request.source}`,
    `proof_hash=sha256:${request.proofHash}`
  ].join("\n");
}

function buildMapsCodeActRequest(params: Omit<MapsCodeActRequest, "schema" | "url" | "proofHash">): MapsCodeActRequest {
  const request: MapsCodeActRequest = {
    schema: "forge.webexplorer.maps.request.v1",
    ...params,
    url: earthUrl(params.latitude, params.longitude),
    proofHash: ""
  };
  request.proofHash = stableHash({ ...request, proofHash: "" });
  return request;
}

function earthUrl(latitude?: number, longitude?: number): string {
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
  return trimmed === MAPS_COMMAND || trimmed.startsWith(`${MAPS_COMMAND} `) ? MAPS_COMMAND : undefined;
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
