const DEFAULT_URL = "https://forge-6cai.onrender.com/health";
const DEFAULT_TIMEOUT_MS = 8000;

function keepaliveUrl() {
  const value = process.env.FORGE_KEEPALIVE_URL || DEFAULT_URL;
  return value.trim();
}

function timeoutMs() {
  const parsed = Number.parseInt(process.env.FORGE_KEEPALIVE_TIMEOUT_MS || "", 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return parsed;
  }
  return DEFAULT_TIMEOUT_MS;
}

function previewBody(value) {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (normalized.length <= 240) {
    return normalized;
  }
  return `${normalized.slice(0, 240)}...`;
}

async function main() {
  const url = keepaliveUrl();
  if (!url) {
    console.error("FORGE_KEEPALIVE_URL is empty");
    process.exitCode = 1;
    return;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs());
  try {
    const response = await fetch(url, {
      method: "GET",
      headers: {
        "user-agent": "forge-render-keepalive/1.0",
        "x-forge-keepalive": "render-cron",
      },
      signal: controller.signal,
    });
    const body = await response.text();
    console.log(
      `forge render keepalive ${response.status} ${response.statusText} ${url} ${previewBody(body)}`,
    );
    if (!response.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    console.error(`forge render keepalive failed for ${url}: ${error.message}`);
    process.exitCode = 1;
  } finally {
    clearTimeout(timeout);
  }
}

await main();
