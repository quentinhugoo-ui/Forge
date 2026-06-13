type DayPart = "morning" | "day" | "evening" | "night";

export interface WelcomeMessage {
  text: string;
  author?: string;
}

const DAY_PART_MESSAGES: Record<DayPart, string[]> = {
  morning: [
    "Morning spark, {name}?",
    "Fresh start, {name}?"
  ],
  day: [
    "Let's make progress.",
    "Let's move things forward, {name}.",
    "What needs momentum, {name}?"
  ],
  evening: [
    "What needs momentum, {name}?",
    "What's worth making?"
  ],
  night: [
    "Quiet hours, sharp ideas."
  ]
};

const DIRECT_MESSAGES: string[] = [
  "Ready to build, {name}?",
  "What's worth making?",
  "Where do we begin, {name}?",
  "What are we making?",
  "What's next, {name}?"
];

const QUOTE_MESSAGES: WelcomeMessage[] = [
  { text: "Stay hungry. Stay foolish.", author: "Steve Jobs" },
  { text: "It is always Day 1.", author: "Jeff Bezos" },
  { text: "Do things that don't scale.", author: "Paul Graham" },
  { text: "Focus on signal over noise.", author: "Elon Musk" },
  { text: "One-person billion-dollar company.", author: "Sam Altman" }
];

function dayPartFrom(hour: number): DayPart {
  if (hour >= 5 && hour < 11) return "morning";
  if (hour >= 11 && hour < 18) return "day";
  if (hour >= 18 && hour < 22) return "evening";
  return "night";
}

function personalize(text: string, firstName: string): string {
  const name = firstName.trim();
  if (!name) {
    return text
      .replace(/,\s*\{name\}/g, "")
      .replace(/\s*\{name\}/g, "")
      .replace(/\s+([?.!])/g, "$1")
      .replace(/\s{2,}/g, " ")
      .trim();
  }
  return text.replace("{name}", name);
}

function randomIndex(length: number): number {
  return length > 0 ? Math.floor(Math.random() * length) : 0;
}

export function selectWelcomeMessage(firstName: string, date = new Date()): WelcomeMessage {
  const dayPart = dayPartFrom(date.getHours());
  const directMessages = [...DAY_PART_MESSAGES[dayPart], ...DIRECT_MESSAGES].map((text) => ({
    text: personalize(text, firstName)
  }));
  const messages = [...directMessages, ...QUOTE_MESSAGES];
  return messages[randomIndex(messages.length)];
}
