export interface RealEstateOnboardingQuestion {
  readonly id: string;
  readonly prompt: string;
}

export interface RealEstateOnboardingState {
  readonly required?: boolean;
  readonly currentIndex?: number;
  readonly total?: number;
  readonly profileHash?: string;
  readonly question?: RealEstateOnboardingQuestion | null;
}

export interface RealEstateOnboardingReport {
  readonly state?: RealEstateOnboardingState | null;
  readonly profileHash?: string;
  readonly error?: string;
  readonly suggestedAnswers?: readonly string[];
  readonly triggeredCollectors?: readonly string[];
}

export function realEstateOnboardingQuestionLine(state: RealEstateOnboardingState | null): string {
  return state?.question ? "Forge Agence Immo" : "";
}

export function realEstateOnboardingPromptText(): string {
  return "";
}

export function realEstateOnboardingReplyLooksUsable(text: string): boolean {
  const value = text.trim();
  if (!value) return false;
  return !/n['’]a pas pu r[ée]pondre|r[ée]essaie|reconnexion|se pr[ée]pare encore|pas renvoy[ée] de texte/i.test(value);
}

