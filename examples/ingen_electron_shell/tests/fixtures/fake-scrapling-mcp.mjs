let input = Buffer.alloc(0);
let nextServerId = 1;

process.stdin.on("data", (chunk) => {
  input = Buffer.concat([input, chunk]);
  readFrames();
});

function readFrames() {
  while (true) {
    const headerEnd = input.indexOf("\r\n\r\n");
    if (headerEnd < 0) return;
    const header = input.slice(0, headerEnd).toString("utf8");
    const match = /content-length\s*:\s*(\d+)/i.exec(header);
    if (!match) {
      input = input.slice(headerEnd + 4);
      continue;
    }
    const length = Number(match[1]);
    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + length;
    if (input.length < bodyEnd) return;
    const body = input.slice(bodyStart, bodyEnd).toString("utf8");
    input = input.slice(bodyEnd);
    handleMessage(JSON.parse(body));
  }
}

function handleMessage(message) {
  if (typeof message.id !== "number") {
    return;
  }
  if (message.method === "initialize") {
    writeResponse(message.id, {
      protocolVersion: "2024-11-05",
      serverInfo: { name: "fake-scrapling", version: "0.0.0-test" },
      capabilities: { tools: {} }
    });
    return;
  }
  if (message.method === "tools/list") {
    writeResponse(message.id, {
      tools: [
        {
          name: "fetch",
          description: "fake Scrapling fetch",
          inputSchema: {
            type: "object",
            properties: {
              url: { type: "string" },
              timeout: { type: "number" },
              selectors: { type: "array" },
              css_selector: { type: "string" },
              wait_selector: { type: "string" },
              network_idle: { type: "boolean" }
            }
          }
        },
        {
          name: "screenshot",
          description: "fake Scrapling screenshot",
          inputSchema: {
            type: "object",
            properties: {
              url: { type: "string" },
              image_type: { type: "string" },
              full_page: { type: "boolean" },
              timeout: { type: "number" }
            }
          }
        }
      ]
    });
    return;
  }
  if (message.method === "tools/call") {
    const name = message.params?.name;
    if (name === "screenshot") {
      writeResponse(message.id, {
        content: [
          {
            type: "image",
            mimeType: "image/png",
            data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII="
          }
        ]
      });
      return;
    }
    writeResponse(message.id, {
      content: [
        {
          type: "text",
          text: JSON.stringify({
            fields: [{ name: "title", value: "Example Domain", selector: "h1" }],
            media: [{ src: "https://example.com/hero.png", alt: "Hero" }],
            links: [{ href: "https://example.com/about" }]
          })
        }
      ],
      structuredContent: {
        fields: [{ name: "title", value: "Example Domain", selector: "h1" }]
      }
    });
    return;
  }
  writeResponse(message.id, null, { code: -32601, message: `Unknown method ${message.method}` });
}

function writeResponse(id, result, error) {
  const response = {
    jsonrpc: "2.0",
    id,
    ...(error ? { error } : { result })
  };
  const body = Buffer.from(JSON.stringify(response), "utf8");
  const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
  process.stdout.write(Buffer.concat([header, body]));
  nextServerId += 1;
}
