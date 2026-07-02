import { afterEach, describe, expect, it } from "vitest";
import {
  GMAIL_COMMAND,
  GMAIL_COM_COMMAND,
  GMAIL_SIGN_IN_URL,
  extractGmailCodeAct,
  gmailWebExplorerNavigationUrl,
  parseGmailCodeAct,
  readGmailCodeAct,
  renderGmailCodeActResult,
  renderGmailTemplateResult
} from "../src/main/gmail-codeact";
import { gmailApiRequiredScopes, runGmailApiBridge } from "../src/main/gmail-api-bridge";

describe("Gmail CodeAct", () => {
  afterEach(() => {
    delete process.env.GMAIL_ACCESS_TOKEN;
  });
  it("returns a compact template for bare /gmail_", () => {
    const codeAct = readGmailCodeAct("/gmail_");

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.reason).toBe("empty_command");
    expect(codeAct.result.template).toContain('/gmail_');
    expect(codeAct.result.template).toContain('template_proof_hash="sha256:');
    expect(codeAct.result.template).toContain('intent="open|search|inspect|summarize|draft|reply"');
    expect(codeAct.result.template).toContain('message_id=""');
    expect(codeAct.result.template).toContain('mode="gmail_api"');
    expect(renderGmailTemplateResult(codeAct.result)).toContain("GMAIL_TEMPLATE_RESULT");
  });

  it("requires the returned template proof before executing /gmail_", () => {
    const codeAct = readGmailCodeAct('/gmail_ intent="search" query="from:quent" keywords=[]');

    expect(codeAct?.kind).toBe("template");
    if (codeAct?.kind !== "template") throw new Error("expected template");
    expect(codeAct.result.reason).toBe("template_required");
  });

  it("parses an explicit filled /gmail_ search command", () => {
    const template = readGmailCodeAct("/gmail_");
    if (template?.kind !== "template") throw new Error("expected template");
    const hash = /template_proof_hash="sha256:([a-f0-9]{64})"/.exec(template.result.template)?.[1];
    expect(hash).toBeDefined();

    const codeAct = readGmailCodeAct(`/gmail_ template_proof_hash="sha256:${hash}" intent="search" query="from:quent subject:facture has:attachment" keywords=["facture","pdf"]`);

    expect(codeAct?.kind).toBe("request");
    if (codeAct?.kind !== "request") throw new Error("expected request");
    expect(codeAct.request).toMatchObject({
      command: GMAIL_COMMAND,
      intent: "search",
      query: "from:quent subject:facture has:attachment",
      keywords: ["facture", "pdf"],
      mode: "gmail_api",
      sendPolicy: "user_approval_required"
    });
    expect(codeAct.request.url).toContain("https://mail.google.com/mail/");
    expect(codeAct.request.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("parses inspect requests with a bounded Gmail message id", () => {
    const request = parseGmailCodeAct('/gmail_ intent="inspect" message_id="18f93abcd"');

    expect(request).toMatchObject({
      command: GMAIL_COMMAND,
      intent: "inspect",
      messageId: "18f93abcd",
      mode: "gmail_api"
    });
  });

  it("keeps parseGmailCodeAct permissive for compatibility tests and internal helpers", () => {
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
    const codeAct = readGmailCodeAct("/gmail_com");

    expect(codeAct?.kind).toBe("request");
    if (codeAct?.kind !== "request") throw new Error("expected request");
    expect(codeAct.request).toMatchObject({
      command: GMAIL_COM_COMMAND,
      intent: "open",
      url: expect.stringContaining("https://accounts.google.com/v3/signin/identifier")
    });
    expect(codeAct.request.url).toContain("continue=https%3A%2F%2Fmail.google.com%2Fmail%2Fu%2F0%2F");
    expect(codeAct.request.url).toContain("service=mail");
    expect(codeAct.request.url).toContain("flowEntry=ServiceLogin");
    expect(codeAct.request.proofHash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("renders only the machine result, not a fixed user-facing sentence", () => {
    const request = parseGmailCodeAct("/gmail_com");
    expect(request).toBeDefined();

    const rendered = renderGmailCodeActResult(request!);

    expect(rendered.startsWith("GMAIL_RESULT")).toBe(true);
    expect(rendered).toContain('execution="split_webexplorer_navigation"');
    expect(rendered).toContain("send_policy=user_approval_required");
    expect(rendered).not.toContain("J'ouvre Gmail");
    expect(rendered).not.toContain("intention");
  });

  it("selects minimum Gmail API scopes for search, metadata and draft flows", () => {
    const search = parseGmailCodeAct('/gmail_ intent="search" query="from:quent"');
    const metadata = parseGmailCodeAct('/gmail_ intent="search" query=""');
    const draft = parseGmailCodeAct('/gmail_ intent="draft" recipient="a@example.com" subject="Hello" body="Draft"');

    expect(gmailApiRequiredScopes(search!)).toEqual(["https://www.googleapis.com/auth/gmail.readonly"]);
    expect(gmailApiRequiredScopes(metadata!)).toEqual(["https://www.googleapis.com/auth/gmail.metadata"]);
    expect(gmailApiRequiredScopes(draft!)).toEqual(["https://www.googleapis.com/auth/gmail.compose"]);
  });

  it("returns a compact auth_required Gmail API result when OAuth is not connected", async () => {
    const previous = process.env.INGEN_GMAIL_ACCESS_TOKEN;
    delete process.env.INGEN_GMAIL_ACCESS_TOKEN;
    delete process.env.GMAIL_ACCESS_TOKEN;
    try {
      const request = parseGmailCodeAct('/gmail_ intent="search" query="from:quent" max_results=2');
      const result = await runGmailApiBridge(request!);

      expect(result.status).toBe("auth_required");
      expect(result.execution).toBe("gmail_rest_api");
      expect(result.requiredScopes).toEqual(["https://www.googleapis.com/auth/gmail.readonly"]);
      expect(JSON.stringify(result)).not.toContain("access_token");
      expect(JSON.stringify(result)).not.toContain("refresh_token");
    } finally {
      if (previous === undefined) {
        delete process.env.INGEN_GMAIL_ACCESS_TOKEN;
      } else {
        process.env.INGEN_GMAIL_ACCESS_TOKEN = previous;
      }
    }
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
