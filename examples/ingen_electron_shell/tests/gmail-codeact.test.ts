import { describe, expect, it } from "vitest";
import {
  GMAIL_COMMAND,
  GMAIL_COM_COMMAND,
  GMAIL_SIGN_IN_URL,
  extractGmailCodeAct,
  gmailWebExplorerNavigationUrl,
  parseGmailCodeAct,
  renderGmailCodeActResult
} from "../src/main/gmail-codeact";

describe("Gmail CodeAct", () => {
  it("parses an explicit /gmail_ search command", () => {
    const request = parseGmailCodeAct('/gmail_ intent="search" query="from:quent subject:facture has:attachment" keywords="facture, pdf"');

    expect(request).toMatchObject({
      command: GMAIL_COMMAND,
      intent: "search",
      query: "from:quent subject:facture has:attachment",
      keywords: ["facture", "pdf"]
    });
    expect(request?.url).toContain("https://mail.google.com/mail/");
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("extracts /gmail_ from assistant text only when explicitly emitted", () => {
    const naturalText = extractGmailCodeAct("Regarde mes mails importants");
    const assistantText = extractGmailCodeAct([
      "Je vais ouvrir Gmail.",
      '/gmail_ intent="open" query="" keywords=""'
    ].join("\n"));

    expect(naturalText).toBeUndefined();
    expect(assistantText).toMatchObject({
      command: GMAIL_COMMAND,
      intent: "open"
    });
  });

  it("opens the split WebExplorer directly on the Gmail Google Accounts sign-in URL for /gmail_com", () => {
    const request = parseGmailCodeAct("/gmail_com");

    expect(request).toMatchObject({
      command: GMAIL_COM_COMMAND,
      intent: "open",
      url: expect.stringContaining("https://accounts.google.com/v3/signin/identifier")
    });
    expect(request?.url).toContain("continue=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F");
    expect(request?.url).toContain("service=mail");
    expect(request?.url).toContain("flowEntry=ServiceLogin");
    expect(request?.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("renders only the machine result, not a fixed user-facing sentence", () => {
    const request = parseGmailCodeAct("/gmail_com");
    expect(request).toBeDefined();

    const rendered = renderGmailCodeActResult(request!);

    expect(rendered.startsWith("GMAIL_RESULT")).toBe(true);
    expect(rendered).not.toContain("J'ouvre Gmail");
    expect(rendered).not.toContain("intention");
  });

  it("normalizes every open Gmail surface navigation to the Google Accounts sign-in URL", () => {
    const gmailCom = parseGmailCodeAct("/gmail_com");
    const gmailOpen = parseGmailCodeAct('/gmail_ intent="open" query="gmail" keywords=""');
    const gmailSearch = parseGmailCodeAct('/gmail_ intent="search" query="facture" keywords=""');

    expect(gmailWebExplorerNavigationUrl(gmailCom!)).toBe(GMAIL_SIGN_IN_URL);
    expect(gmailWebExplorerNavigationUrl(gmailOpen!)).toBe(GMAIL_SIGN_IN_URL);
    expect(gmailWebExplorerNavigationUrl(gmailSearch!)).toContain("https://mail.google.com/mail/");
  });
});
