import type { RealEstateToolDefinition } from "./tools.js";

export const realEstateCanvasThinkingPhrases = Object.freeze([
  "analyse les mandats",
  "croise les signaux locaux",
  "consulte la mémoire agence",
  "prépare le plan d'action",
  "recoupe DVF, DPE et cadastre",
  "classe les priorités commerciales",
  "cherche les angles de relance",
  "synthétise la veille concurrence",
]);

export const realEstateCanvasThinkingFirstPhrases = Object.freeze([
  "réfléchit",
  "analyse",
  "prépare la réponse",
  "travaille sur le dossier",
]);

export const realEstateChatPlaceholderIdeas = Object.freeze([
  "Analyse 18 mois de mandats, visites et relances pour prédire les vendeurs à rappeler cette semaine",
  "Croise DVF, DPE, cadastre, urbanisme et météo locale pour scorer le potentiel d'un quartier",
  "Audite les annonces de l'agence et des concurrents pour sortir les angles qui convertissent mieux",
  "Prépare un plan d'appels vendeur avec objections, preuves locales et timing de relance",
  "Simule 500 scénarios de prix pour un bien avec risque de décrochage, marge et délai de vente",
  "Classe les acquéreurs par capacité, urgence, financement probable et matching biens disponibles",
  "Construis un cockpit agence: pipeline, KPI, trésorerie, visites, tâches et alertes critiques",
  "Analyse les avis Google, réseaux sociaux et annonces concurrentes pour trouver les faiblesses du marché",
  "Prépare une campagne de recrutement avec profils, scripts d'approche et score de potentiel commercial",
  "Connecte courtiers, assurances, notaires et fiscalité pour détecter les dossiers à risque",
]);

export const realEstateChatPlaceholderScenarios = Object.freeze([
  {
    codex: "Analyse les mandats vendeurs et priorise les relances",
    gemini: "Croise DVF, DPE, cadastre et urbanisme",
    claude: "Prépare le rapport vendeur avec preuves locales",
  },
  {
    codex: "Audite les annonces de l'agence et des concurrents",
    gemini: "Mesure la performance SeLoger, Leboncoin et site agence",
    claude: "Réécris les annonces avec angles de conversion",
  },
  {
    codex: "Classe les prospects vendeurs par probabilité de mandat",
    gemini: "Scanne la veille locale et les signaux faibles",
    claude: "Prépare les scripts d'appel et les objections",
  },
  {
    codex: "Construis le cockpit KPI agence pour la semaine",
    gemini: "Simule trésorerie, commissions et planning visites",
    claude: "Propose les décisions commerciales prioritaires",
  },
]);

type RedactionCounts = Record<string, number>;

export function redactRealEstateClientData(text: string): { text: string; counts: RedactionCounts } {
  let output = String(text || "");
  const counts: RedactionCounts = {};
  const replaceWithCount = (pattern: RegExp, key: string, marker: string): void => {
    output = output.replace(pattern, () => {
      counts[key] = (counts[key] || 0) + 1;
      return marker;
    });
  };
  replaceWithCount(/\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi, "email", "[EMAIL_CLIENT_REDACTED]");
  replaceWithCount(/\bFR[0-9A-Z](?:[ -]?[0-9A-Z]){13,}\b/gi, "iban", "[IBAN_CLIENT_REDACTED]");
  replaceWithCount(/\b(?:\+33|0033|0)[1-9](?:[\s.-]?\d{2}){4}\b/g, "phone", "[TELEPHONE_CLIENT_REDACTED]");
  replaceWithCount(/\b[12]\s?\d{2}\s?\d{2}\s?\d{2}\s?\d{3}\s?\d{3}(?:\s?\d{2})?\b/g, "nir", "[NIR_CLIENT_REDACTED]");
  const addressKeywords = /\b(adresse|address|rue|avenue|av\.|boulevard|bd|impasse|chemin|route|allee|allée|place|quai|residence|résidence)\b/i;
  output = output.split(/(\r?\n)/).map((line) => {
    if (!line || /^\r?\n$/.test(line)) return line;
    if (!/\d/.test(line) || !addressKeywords.test(line)) return line;
    counts.address = (counts.address || 0) + 1;
    const labelMatch = line.match(/^(\s*(?:adresse|address)\s*[:=-]\s*)/i);
    return labelMatch ? `${labelMatch[1]}[ADRESSE_CLIENT_REDACTED]` : "[ADRESSE_CLIENT_REDACTED]";
  }).join("");
  return { text: output, counts };
}

export function buildRealEstatePrivacyPacket(text: string, redactSecrets: (value: string) => string): string {
  const counts = redactRealEstateClientData(redactSecrets(text)).counts;
  const redactions = Object.entries(counts)
    .filter(([, count]) => count > 0)
    .map(([key, count]) => `${key}:${count}`)
    .join(",");
  return [
    "FORGE_REAL_ESTATE_PRIVACY:",
    "mode=local_first_client_data_minimized",
    "scope=agence_immo",
    "raw_client_files=local_only",
    "provider_guard=backend_revalidates_before_runtime",
    `redactions=${redactions || "none_detected"}`,
  ].join("\n");
}

export function realEstateCommandFromText(
  text: string,
  toolByCommand: ReadonlyMap<string, RealEstateToolDefinition>,
): RealEstateToolDefinition | null {
  const token = String(text || "").trim().split(/\s+/, 1)[0] || "";
  if (!/^\/[a-z0-9_]+_$/.test(token)) return null;
  return toolByCommand.get(token) || null;
}

function compactRealEstateContextText(text: string, maxChars = 900): string {
  const clean = String(text || "").replace(/\s+/g, " ").trim();
  if (clean.length <= maxChars) return clean;
  return `${clean.slice(0, Math.max(0, maxChars - 3))}...`;
}

function realEstateMemoryCommitLines(context: any): string[] {
  const memory = context?.memory_commits || null;
  const selected = Array.isArray(memory?.selected) ? memory.selected.slice(0, 3) : [];
  const lines = [
    "FORGE_REAL_ESTATE_MEMORY_CONTEXT:",
    `memory_source=${memory?.path || "real_estate_memory_commits.jsonl"}`,
    `memory_status=${memory?.status || "unknown"}`,
    `memory_selected=${memory?.selection?.selected || selected.length || 0}`,
    `memory_matched=${memory?.selection?.matched || 0}`,
  ];
  selected.forEach((commit: any, index: number) => {
    const brainText = commit?.brainCommitRequest?.text || commit?.noteText || "";
    const evidence = commit?.evidence || {};
    lines.push([
      `memory_commit_${index + 1}:`,
      `score=${commit?.score ?? 0}`,
      `action_id=${commit?.actionId || ""}`,
      `scenario=${commit?.scenario || ""}`,
      `decision=${commit?.decision || ""}`,
      `confidence=${commit?.confidence ?? ""}`,
      `ranked_actions_proof=${evidence?.rankedActionsProofHash || ""}`,
      `rust_compute_proof=${evidence?.rustComputeProofHash || ""}`,
      `brief=${compactRealEstateContextText(brainText, 1100)}`,
    ].join("\n"));
  });
  if (!selected.length) {
    lines.push("memory_note=Aucun commit memoire agence recent n'a encore ete trouve; utiliser le cache Data Sync et demander une relance pipeline seulement si la fraicheur est insuffisante.");
  }
  return lines;
}

function realEstateLlmCacheLines(context: any): string[] {
  const cache = context?.harvester_snapshot?.latestLlmIntelCache || context?.harvester_snapshot?.latest_llm_intel_cache || null;
  if (!cache) return [];
  const opportunities = Array.isArray(cache.topOpportunities || cache.top_opportunities)
    ? (cache.topOpportunities || cache.top_opportunities).slice(0, 3)
    : [];
  const lines = [
    "FORGE_REAL_ESTATE_LLM_INTEL_CACHE:",
    `cache_id=${cache.cacheId || cache.cache_id || ""}`,
    `status=${cache.status || ""}`,
    `source_pack=${cache.sourcePackId || cache.source_pack_id || ""}`,
    `bounded_projection=real_estate_llm_intel_cache_v1`,
    `projection_hash=${cache.projectionHash || cache.projection_hash || ""}`,
    `evidence_hash=${cache.evidenceHash || cache.evidence_hash || ""}`,
    `memory_evidence_hash=${cache.memoryEvidenceHash || cache.memory_evidence_hash || ""}`,
    `kasm_contract_hash=${cache.kasmContractHash || cache.kasm_contract_hash || ""}`,
    `brain_ref=${cache.brainRef || cache.brain_ref || ""}`,
    `action_brief=${compactRealEstateContextText(cache.actionBrief || cache.action_brief || "", 900)}`,
  ];
  opportunities.forEach((opportunity: any, index: number) => {
    lines.push(
      `opportunity_${index + 1}=${compactRealEstateContextText(opportunity.factLine || opportunity.fact_line || opportunity.recommendedAction || opportunity.recommended_action || "", 420)}`,
    );
  });
  return lines;
}

export function realEstateCommandPacket(
  text: string,
  context: any,
  toolByCommand: ReadonlyMap<string, RealEstateToolDefinition>,
): string {
  const tool = realEstateCommandFromText(text, toolByCommand);
  if (!tool) return "";
  const packet = [
    "FORGE_REAL_ESTATE_PROGRAM_COMMAND:",
    `slash=${tool.command}`,
    `program_name=${tool.command.slice(1, -1)}`,
    `tool_id=${tool.id}`,
    `label=${tool.label}`,
    "scope=agence_immo",
    "memory_layer=semantic",
    "first_step=brain_recall(scope:agence_immo,memory_layer:semantic)",
    "data_context=real_estate_harvester_snapshot",
    "route=slash/MCP command -> Forge brain/memory -> KASM/Data Sync context -> answer/action",
  ];
  if (context) {
    packet.push(...realEstateMemoryCommitLines(context));
    packet.push(...realEstateLlmCacheLines(context));
    packet.push("instruction=Use the memory commits, proof hashes and KASM/Data Sync cache before native model reasoning; do not ask for raw agency files when compact evidence is enough.");
  }
  return packet.join("\n");
}

export function realEstateBrainRefLabel(context: any): string {
  const refs = context?.brain?.refs || context?.brain?.brain_context?.refs || null;
  return refs?.scoped_layer_note?.hash
    || refs?.scoped_llm_note?.hash
    || refs?.latest_memory?.hash
    || context?.brain?.brain_context?.scoped_note_hash
    || "";
}
