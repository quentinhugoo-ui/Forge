export type RealEstateToolIcon =
  | "annonces"
  | "cadastre"
  | "conformite"
  | "database"
  | "energy"
  | "estimation"
  | "matching"
  | "pin"
  | "portal"
  | "rapport"
  | "risk"
  | "site"
  | "urbanisme";

export type RealEstateToolTuple = readonly [string, string, RealEstateToolIcon];

export interface RealEstateToolGroup {
  readonly label: string;
  readonly icon: RealEstateToolIcon;
  readonly tools: readonly RealEstateToolTuple[];
}

export interface RealEstateToolDefinition {
  readonly id: string;
  readonly label: string;
  readonly icon: RealEstateToolIcon;
  readonly command: `/${string}_`;
}

export const realEstateToolGroups = Object.freeze([
  {
    label: "Production immo",
    icon: "site",
    tools: [
      ["mandat-vendeur", "Mandat vendeur", "rapport"],
      ["estimation", "Estimation", "estimation"],
      ["rapport-vendeur", "Rapport vendeur", "rapport"],
      ["diagnostics", "Diagnostics", "energy"],
      ["conformite", "Conformite", "conformite"],
      ["diffusion", "Diffusion", "annonces"],
      ["audit-annonces", "Audit annonces", "portal"],
      ["performance-diffusion", "Performance diffusion", "database"],
    ],
  },
  {
    label: "Marche & veille",
    icon: "database",
    tools: [
      ["marche-veille", "Marche & veille", "database"],
      ["dvf", "DVF", "database"],
      ["cadastre", "Cadastre", "cadastre"],
      ["dpe-ademe", "DPE / ADEME", "energy"],
      ["georisques", "Georisques", "risk"],
      ["urbanisme", "Urbanisme", "urbanisme"],
      ["veille-locale", "Veille locale", "pin"],
      ["concurrence", "Concurrence", "portal"],
      ["reputation", "Reputation", "site"],
    ],
  },
  {
    label: "Contacts",
    icon: "matching",
    tools: [
      ["prospects", "Prospects", "matching"],
      ["vendeurs", "Vendeurs", "rapport"],
      ["acquereurs", "Acquereurs", "matching"],
      ["matching-acheteurs", "Matching acheteurs", "matching"],
      ["repondeur-ia", "Repondeur IA", "portal"],
      ["chatbot-site", "Chatbot site", "annonces"],
      ["partenaires", "Partenaires", "site"],
    ],
  },
  {
    label: "Pilotage agence",
    icon: "energy",
    tools: [
      ["pilotage-agence", "Pilotage agence", "energy"],
      ["pipeline", "Pipeline", "database"],
      ["kpi-agence", "KPI agence", "rapport"],
      ["planning-visites", "Planning visites", "cadastre"],
      ["coaching-equipe", "Coaching equipe", "matching"],
      ["performance-commerciaux", "Performance commerciaux", "database"],
    ],
  },
  {
    label: "Back-office",
    icon: "conformite",
    tools: [
      ["back-office", "Back-office", "conformite"],
      ["comptabilite", "Comptabilite", "database"],
      ["fiscalite", "Fiscalite", "rapport"],
      ["tresorerie", "Tresorerie", "database"],
      ["courtiers", "Courtiers", "matching"],
      ["assurances", "Assurances", "conformite"],
      ["notaires", "Notaires", "rapport"],
      ["travaux", "Travaux", "energy"],
    ],
  },
  {
    label: "Equipe",
    icon: "matching",
    tools: [
      ["recrutement", "Recrutement", "matching"],
      ["onboarding", "Onboarding", "site"],
      ["formation", "Formation", "rapport"],
    ],
  },
] satisfies readonly RealEstateToolGroup[]);
