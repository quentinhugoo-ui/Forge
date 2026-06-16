import { createHash } from "node:crypto";
import {
  BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA,
  BRAIN_LOCAL_ACTIONS_COMMAND,
  BRAIN_MODIFY_NAMED_BRAIN_COMMAND,
  BRAIN_NEWBRAIN_COMMAND
} from "../shared/ipc-contract.js";

export interface DomainBrainCodeActResult {
  schema: typeof BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA;
  command: typeof BRAIN_NEWBRAIN_COMMAND | typeof BRAIN_MODIFY_NAMED_BRAIN_COMMAND | string;
  status: "ok" | "error";
  operation: "create_specialized_brain" | "modify_specialized_brain";
  brainName: string;
  title: string;
  activationCommand: string;
  activationTriggers: string[];
  changedKind: string;
  codeActs: string[];
  warnings: string[];
  proofHash: string;
}

const MAX_FIELD_CHARS = 1_200;
const MAX_CODEACTS = 24;

export function extractDomainBrainCodeActResult(text: string): DomainBrainCodeActResult | undefined {
  if (text.includes("DOMAIN_BRAIN_RESULT")) {
    return undefined;
  }
  return extractNewBrainResult(text) ?? extractModifyBrainResult(text);
}

export function renderDomainBrainCodeActResult(result: DomainBrainCodeActResult): string {
  return [
    "DOMAIN_BRAIN_RESULT v1",
    `schema=${result.schema}`,
    `command=${result.command}`,
    `status=${result.status}`,
    `operation=${result.operation}`,
    `brain_name=${JSON.stringify(result.brainName)}`,
    `title=${JSON.stringify(result.title)}`,
    `activation_command=${JSON.stringify(result.activationCommand)}`,
    `activation_triggers=${JSON.stringify(result.activationTriggers)}`,
    `changed_kind=${JSON.stringify(result.changedKind)}`,
    `specialized_codeacts=${JSON.stringify(result.codeActs)}`,
    `warnings=${JSON.stringify(result.warnings)}`,
    `proof_hash=sha256:${result.proofHash}`
  ].join("\n");
}

function extractNewBrainResult(text: string): DomainBrainCodeActResult | undefined {
  const block = codeActFieldBlocksFromText(text, isNewBrainInvocationLine)[0];
  if (!block) {
    return undefined;
  }
  const fields = parseTemplateFields(block);
  const brainName = specializedBrainSlug(fields.get("brain_name") ?? fields.get("name") ?? fields.get("domain") ?? "");
  if (!brainName) {
    return errorResult({
      command: BRAIN_NEWBRAIN_COMMAND,
      operation: "create_specialized_brain",
      warning: "missing brain_name"
    });
  }
  const title = cleanField(fields.get("title"), 160) || titleFromBrainName(brainName);
  const codeActs = normalizeSpecializedCodeActs(fields.get("initial_codeacts"));
  const result: DomainBrainCodeActResult = {
    schema: BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA,
    command: BRAIN_NEWBRAIN_COMMAND,
    status: "ok",
    operation: "create_specialized_brain",
    brainName,
    title,
    activationCommand: specializedBrainCodeActCommand(brainName),
    activationTriggers: splitList(fields.get("activation_triggers") ?? fields.get("triggers")),
    changedKind: "specialized_brain",
    codeActs,
    warnings: [],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function extractModifyBrainResult(text: string): DomainBrainCodeActResult | undefined {
  const block = codeActFieldBlocksFromText(text, isModifyBrainInvocationLine)[0];
  if (!block) {
    return undefined;
  }
  const fields = parseTemplateFields(block);
  const brainName = brainNameFromModifyBlock(block, fields);
  if (!brainName) {
    return errorResult({
      command: BRAIN_MODIFY_NAMED_BRAIN_COMMAND,
      operation: "modify_specialized_brain",
      warning: "missing brain_name"
    });
  }
  const requestedKind = cleanField(fields.get("entry_kind") ?? fields.get("kind") ?? fields.get("type"), 80) || "lesson";
  const content = cleanField(fields.get("content") ?? fields.get("text") ?? fields.get("template"), MAX_FIELD_CHARS);
  const addedCodeAct = firstCodeActCommand(content);
  const result: DomainBrainCodeActResult = {
    schema: BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA,
    command: `/modify"${brainName}"brain_`,
    status: "ok",
    operation: "modify_specialized_brain",
    brainName,
    title: titleFromBrainName(brainName),
    activationCommand: specializedBrainCodeActCommand(brainName),
    activationTriggers: [],
    changedKind: requestedKind,
    codeActs: normalizeSpecializedCodeActs(addedCodeAct),
    warnings: addedCodeAct ? [] : ["full specialized Brain registry is stored in the renderer; this result confirms the requested update and default local action route"],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function errorResult(params: {
  command: typeof BRAIN_NEWBRAIN_COMMAND | typeof BRAIN_MODIFY_NAMED_BRAIN_COMMAND;
  operation: DomainBrainCodeActResult["operation"];
  warning: string;
}): DomainBrainCodeActResult {
  const result: DomainBrainCodeActResult = {
    schema: BRAIN_DOMAIN_BRAIN_RESULT_SCHEMA,
    command: params.command,
    status: "error",
    operation: params.operation,
    brainName: "",
    title: "",
    activationCommand: "",
    activationTriggers: [],
    changedKind: "",
    codeActs: [BRAIN_LOCAL_ACTIONS_COMMAND],
    warnings: [params.warning],
    proofHash: ""
  };
  result.proofHash = stableHash({ ...result, proofHash: "" });
  return result;
}

function isNewBrainInvocationLine(line: string): boolean {
  const trimmed = line.trim();
  if (trimmed.startsWith(BRAIN_NEWBRAIN_COMMAND)) {
    return true;
  }
  return commandAssignment(trimmed) === BRAIN_NEWBRAIN_COMMAND;
}

function isModifyBrainInvocationLine(line: string): boolean {
  const trimmed = line.trim();
  if (/^\/modify(?:"[^"]+"|'[^']+')brain_/.test(trimmed)) {
    return true;
  }
  return /^\/modify(?:"[^"]+"|'[^']+')brain_$/.test(commandAssignment(trimmed));
}

function commandAssignment(value: string): string {
  const match = /(?:^|\s)command\s*=\s*("([^"]+)"|'([^']+)'|([^\s]+))/.exec(value);
  return match?.[2] ?? match?.[3] ?? match?.[4] ?? "";
}

function codeActFieldBlocksFromText(text: string, isInvocationLine: (line: string) => boolean): string[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const blocks: string[] = [];
  let insideFence: string | null = null;
  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const trimmed = lines[lineIndex]?.trim() ?? "";
    const fence = /^(```+|~~~+)/.exec(trimmed)?.[1] ?? null;
    if (fence && (!insideFence || trimmed.startsWith(insideFence))) {
      insideFence = insideFence ? null : fence;
      continue;
    }
    if (insideFence || !isInvocationLine(trimmed)) {
      continue;
    }
    const blockLines = [trimmed];
    for (let nextIndex = lineIndex + 1; nextIndex < lines.length; nextIndex += 1) {
      const nextLine = lines[nextIndex]?.trim() ?? "";
      if (!nextLine || /^(```+|~~~+)/.test(nextLine) || nextLine.startsWith("/")) {
        break;
      }
      if (!/^[a-zA-Z_][\w-]*\s*=/.test(nextLine)) {
        break;
      }
      blockLines.push(nextLine);
      lineIndex = nextIndex;
    }
    blocks.push(blockLines.join(" "));
  }
  return blocks;
}

function parseTemplateFields(body: string): Map<string, string> {
  const fields = new Map<string, string>();
  const fieldRegex = /(?:^|\s)([a-zA-Z_][\w-]*)\s*=\s*(?:"((?:\\.|[^"])*)"|'((?:\\.|[^'])*)'|([\s\S]*?))(?=\s+[a-zA-Z_][\w-]*\s*=|$)/g;
  let match: RegExpExecArray | null;
  while ((match = fieldRegex.exec(body)) !== null) {
    const key = match[1]?.trim();
    if (!key) continue;
    fields.set(key, decodeTemplateValue(match[2] ?? match[3] ?? match[4] ?? "").trim());
  }
  return fields;
}

function decodeTemplateValue(value: string): string {
  return value
    .replace(/\\"/gu, "\"")
    .replace(/\\'/gu, "'")
    .replace(/\\n/gu, "\n")
    .replace(/\\t/gu, "\t")
    .replace(/\\\\/gu, "\\");
}

function specializedBrainSlug(value: string): string {
  return cleanField(value, 96)
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 48);
}

function specializedBrainCodeActCommand(brainName: string): string {
  const slug = specializedBrainSlug(brainName);
  return slug ? `/${slug.endsWith("brain") ? slug : `${slug}brain`}_` : "";
}

function brainNameFromModifyBlock(block: string, fields: Map<string, string>): string {
  const commandMatch = /\/modify(?:"([^"]+)"|'([^']+)')brain_/.exec(block);
  return specializedBrainSlug(commandMatch?.[1] ?? commandMatch?.[2] ?? fields.get("brain_name") ?? "");
}

function firstCodeActCommand(value: string): string {
  return /^\/[a-zA-Z0-9][a-zA-Z0-9_]*_/.exec(value.trim())?.[0] ?? "";
}

function normalizeSpecializedCodeActs(value: string | undefined): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of [BRAIN_LOCAL_ACTIONS_COMMAND, ...splitList(value)]) {
    const command = firstCodeActCommand(item) || (item.trim().startsWith("/") ? item.trim() : "");
    if (!command || seen.has(command)) {
      continue;
    }
    seen.add(command);
    result.push(command);
    if (result.length >= MAX_CODEACTS) {
      break;
    }
  }
  return result;
}

function splitList(value: string | undefined): string[] {
  return cleanField(value, MAX_FIELD_CHARS)
    .split(/[|,;\n]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function titleFromBrainName(brainName: string): string {
  const words = (specializedBrainSlug(brainName) || "specialized_brain").split("_").filter(Boolean);
  return `${words.map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(" ")} Brain`;
}

function cleanField(value: string | undefined, maxLength: number): string {
  return (value ?? "").replace(/\s+/g, " ").slice(0, maxLength).trim();
}

function stableHash(value: unknown): string {
  return createHash("sha256").update(JSON.stringify(stableJson(value))).digest("hex");
}

function stableJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(stableJson);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, item]) => [key, stableJson(item)])
    );
  }
  return value;
}
