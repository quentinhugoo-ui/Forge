import { afterEach, describe, expect, it, vi } from "vitest";
import {
  GMAIL_COMMAND,
  GMAIL_SIGN_IN_URL,
  extractGmailCodeAct,
  gmailWebExplorerNavigationUrl,
  parseGmailCodeAct,
  readGmailCodeAct,
  renderGmailCodeActResult,
  renderGmailTemplateResult
} from "../src/main/gmail-codeact";
import { GMAIL_API_BASE_URL, GMAIL_OAUTH_TOKEN_URL, gmailApiRequiredScopes, runGmailApiBridge } from "../src/main/gmail-api-bridge";

const originalFetch = globalThis.fetch;

describe("Gmail CodeAct", () => {
  afterEach(() => {
    globalThis.fetch = originalFetch;
    delete process.env.GMAIL_ACCESS_TOKEN;
    delete process.env.INGEN_GMAIL_ACCESS_TOKEN;
    delete process.env.GOOGLE_OAUTH_CLIENT_ID;
    delete process.env.GOOGLE_OAUTH_CLIENT_SECRET;
    delete process.env.GOOGLE_OAUTH_REFRESH_TOKEN;
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


  it("renders only the machine result, not a fixed user-facing sentence", () => {
    const request = parseGmailCodeAct('/gmail_ intent="open" mode="split_webexplorer"');
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

  it("exchanges a refresh token for a short Gmail access token without echoing secrets", async () => {
    process.env.GOOGLE_OAUTH_CLIENT_ID = "client-id";
    process.env.GOOGLE_OAUTH_CLIENT_SECRET = "client-secret";
    process.env.GOOGLE_OAUTH_REFRESH_TOKEN = "refresh-secret";
    const calls: Array<{ url: string; init?: RequestInit }> = [];
    globalThis.fetch = vi.fn(async (url: string | URL | Request, init?: RequestInit) => {
      const href = String(url);
      calls.push({ url: href, init });
      if (href === GMAIL_OAUTH_TOKEN_URL) {
        return new Response(JSON.stringify({ access_token: "ya29.short", expires_in: 3600, token_type: "Bearer" }), { status: 200 });
      }
      if (href.startsWith(`${GMAIL_API_BASE_URL}/users/me/messages`)) {
        return new Response(JSON.stringify({ messages: [], resultSizeEstimate: 0 }), { status: 200 });
      }
      return new Response(JSON.stringify({ error: "unexpected" }), { status: 500 });
    }) as typeof fetch;

    const request = parseGmailCodeAct('/gmail_ intent="search" query="from:quent" max_results=2');
    const result = await runGmailApiBridge(request!);

    expect(result.status).toBe("ok");
    expect(calls[0].url).toBe(GMAIL_OAUTH_TOKEN_URL);
    expect(String(calls[0].init?.body)).toContain("refresh_token=refresh-secret");
    expect(calls[1].init?.headers).toMatchObject({ authorization: "Bearer ya29.short" });
    const rendered = JSON.stringify(result);
    expect(rendered).not.toContain("refresh-secret");
    expect(rendered).not.toContain("client-secret");
    expect(rendered).not.toContain("ya29.short");
  });
  it("maps Gmail insufficient scope errors to auth_required", async () => {
    process.env.INGEN_GMAIL_ACCESS_TOKEN = "metadata-token";
    globalThis.fetch = vi.fn(async (url: string | URL | Request) => {
      const href = String(url);
      if (href.startsWith(`${GMAIL_API_BASE_URL}/users/me/messages`)) {
        return new Response(JSON.stringify({
          error: {
            code: 403,
            message: "Metadata scope does not support 'q' parameter",
            status: "PERMISSION_DENIED"
          }
        }), { status: 403 });
      }
      return new Response(JSON.stringify({ error: "unexpected" }), { status: 500 });
    }) as typeof fetch;

    const request = parseGmailCodeAct('/gmail_ intent="search" query="newer_than:1d" max_results=2');
    const result = await runGmailApiBridge(request!);

    expect(result.status).toBe("auth_required");
    expect(result.requiredScopes).toEqual(["https://www.googleapis.com/auth/gmail.readonly"]);
    expect(result.warnings.join(" ")).toContain("current token does not include the scope");
    expect(result.error).toContain("Metadata scope does not support");
  });
  it("does not accept the removed /gmail_com CodeAct", () => {
    expect(parseGmailCodeAct("/gmail_com")).toBeUndefined();
    expect(readGmailCodeAct("/gmail_com")).toBeUndefined();
  });
  it("normalizes every open Gmail surface navigation to the Google Accounts sign-in URL", () => {
    const gmailOpen = parseGmailCodeAct('/gmail_ intent="open" query="gmail" keywords=""');
    const gmailSearch = parseGmailCodeAct('/gmail_ intent="search" query="facture" keywords=""');

    expect(gmailWebExplorerNavigationUrl(gmailOpen!)).toBe(GMAIL_SIGN_IN_URL);
    expect(gmailWebExplorerNavigationUrl(gmailSearch!)).toContain("https://mail.google.com/mail/");
  });
});
