import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const repoRoot = resolve(new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:\/)/, "$1"));
const sourcePath = process.env.INGEN_CHAT_ARCHIVE_PATH ??
  "C:\\Users\\quent\\AppData\\Roaming\\InGenRuntime\\brain\\chat-session-archive.json";
const outputDir = resolve(repoRoot, "reports", "session-history-adaptation-ricoeur");
const htmlPath = join(outputDir, "session-history-adaptation-ricoeur.html");
const pdfPath = join(outputDir, "session-history-adaptation-ricoeur.pdf");
const electronPrintHelperPath = join(outputDir, "print-pdf-electron.cjs");

const targetSessionIds = [
  "chat-mqcugnzf-1e5de37f95",
  "chat-mqct0a8y-f800ab95eb"
];

const sessionNotes = {
  "chat-mqcugnzf-1e5de37f95": {
    label: "Adaptation roman cinema",
    question:
      "Comment passer d'un roman a un objet filmique: selection, condensation, dramatisation, puis application a Pierre Loti et a l'imaginaire ottoman fin XIXe.",
    axis: [
      "L'adaptation est traitee comme une traduction de langage: du texte interieur vers des signes visibles, sonores et dramatiques.",
      "La conversation glisse vers un cas d'etude coherent: harems ottomans, education feminine, culture europeenne, puis Loti/Aziyade comme laboratoire de regard orientaliste et de melodrame.",
      "La fin devient une fiche de personnages: arcs et fonctions dramaturgiques de Loti, Aziyade, Samuel, Plumkett, William Brown et la soeur de Loti."
    ],
    takeaway:
      "Le fil utile est: choisir un axe, condenser, rendre visible, puis relire chaque personnage par sa fonction dans le conflit central."
  },
  "chat-mqct0a8y-f800ab95eb": {
    label: "Pensee sociale Ricoeur",
    question:
      "Comprendre la portee sociale de Ricoeur a travers la capacite humaine, la reconnaissance, la justice, la memoire et le temps raconte.",
    axis: [
      "Le noyau Ricœurien est formule comme une articulation entre sujet capable, autrui, institutions justes et memoire critique.",
      "Les livres sont resumes comme des entrees complementaires: Amour et Justice, Histoire et verite, La Memoire, l'Histoire, l'Oubli, Parcours de la reconnaissance, Temps et recit I-II-III, Le Juste.",
      "Les doubles prompts ont produit deux reponses voisines sur Temps et recit II et III: elles confirment les themes mais creent une redondance a nettoyer dans une version finale."
    ],
    takeaway:
      "Le fil utile est: la justice doit etre institutionnelle sans devenir froide; le recit rend le temps humain; la reconnaissance transforme l'individu en sujet social."
  }
};

function sha256(text) {
  return createHash("sha256").update(text).digest("hex");
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function compactText(value, max = 780) {
  const text = String(value ?? "").replace(/\s+/g, " ").trim();
  if (text.length <= max) return text;
  return `${text.slice(0, max - 1).trimEnd()}…`;
}

function firstUsefulParagraph(value) {
  const paragraphs = String(value ?? "")
    .replace(/\r\n/g, "\n")
    .split(/\n{2,}/)
    .map((item) => item.trim())
    .filter(Boolean);
  return compactText(paragraphs.find((item) => !item.startsWith("/")) ?? paragraphs[0] ?? "", 900);
}

function roleLabel(role) {
  if (role === "user") return "Utilisateur";
  if (role === "assistant") return "Assistant";
  return "Systeme";
}

function statsFor(session) {
  const userMessages = session.messages.filter((message) => message.role === "user").length;
  const assistantMessages = session.messages.filter((message) => message.role === "assistant").length;
  const chars = session.messages.reduce((total, message) => total + String(message.text ?? "").length, 0);
  return { userMessages, assistantMessages, chars };
}

function timelineHtml(session) {
  return session.messages.map((message, index) => {
    const text = message.role === "assistant"
      ? firstUsefulParagraph(message.text)
      : compactText(message.text, 520);
    return `
      <article class="turn">
        <div class="turnMeta">
          <span class="turnIndex">${index + 1}</span>
          <span class="role ${message.role}">${roleLabel(message.role)}</span>
          <span>${escapeHtml(message.createdAt)}</span>
        </div>
        <p>${escapeHtml(text)}</p>
      </article>
    `;
  }).join("");
}

function transcriptHtml(session) {
  return session.messages.map((message, index) => `
    <article class="transcriptTurn">
      <h4>${index + 1}. ${roleLabel(message.role)} <span>${escapeHtml(message.createdAt)}</span></h4>
      <pre>${escapeHtml(message.text)}</pre>
    </article>
  `).join("");
}

function sessionSection(session) {
  const notes = sessionNotes[session.sessionId];
  const stats = statsFor(session);
  return `
    <section class="session pageBreak">
      <div class="eyebrow">${escapeHtml(notes.label)}</div>
      <h2>${escapeHtml(session.title)}</h2>
      <div class="metaGrid">
        <div><strong>Session ID</strong><span>${escapeHtml(session.sessionId)}</span></div>
        <div><strong>Fenetre</strong><span>${escapeHtml(session.createdAt)} → ${escapeHtml(session.updatedAt)}</span></div>
        <div><strong>Messages</strong><span>${session.messages.length} (${stats.userMessages} utilisateur, ${stats.assistantMessages} assistant)</span></div>
        <div><strong>Volume</strong><span>${stats.chars.toLocaleString("fr-FR")} caracteres</span></div>
      </div>

      <h3>Question directrice</h3>
      <p>${escapeHtml(notes.question)}</p>

      <h3>Synthese</h3>
      <ul>${notes.axis.map((item) => `<li>${escapeHtml(item)}</li>`).join("")}</ul>

      <div class="takeaway"><strong>A retenir.</strong> ${escapeHtml(notes.takeaway)}</div>

      <h3>Chronologie lisible</h3>
      ${timelineHtml(session)}
    </section>
  `;
}

function buildHtml(sessions, sourceHash) {
  const generatedAt = new Date().toISOString();
  const selectedHash = sha256(JSON.stringify(sessions.map((session) => ({
    sessionId: session.sessionId,
    title: session.title,
    proofHash: session.proofHash,
    messages: session.messages.map((message) => ({
      role: message.role,
      turnId: message.turnId,
      createdAt: message.createdAt,
      textHash: sha256(message.text ?? "")
    }))
  }))));

  return `<!doctype html>
<html lang="fr">
<head>
  <meta charset="utf-8" />
  <title>Historique synthetique - Adaptation roman cinema / Pensee sociale Ricoeur</title>
  <style>
    @page { margin: 18mm 16mm; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      color: #181a1f;
      background: #ffffff;
      font-family: "Segoe UI", Arial, sans-serif;
      line-height: 1.42;
      font-size: 10.5pt;
    }
    h1, h2, h3, h4 { margin: 0; line-height: 1.16; }
    h1 { font-size: 28pt; max-width: 760px; }
    h2 { font-size: 20pt; margin-top: 8px; }
    h3 { font-size: 12.5pt; margin-top: 22px; margin-bottom: 8px; color: #0b5b66; }
    h4 { font-size: 9.5pt; margin: 0 0 6px; color: #252a31; }
    p { margin: 0 0 9px; }
    ul { margin: 0 0 10px 18px; padding: 0; }
    li { margin: 0 0 6px; }
    .cover {
      min-height: 92vh;
      display: flex;
      flex-direction: column;
      justify-content: space-between;
      padding: 8mm 0;
    }
    .coverTop { border-left: 5px solid #0f9c9d; padding-left: 18px; }
    .subtitle { margin-top: 14px; font-size: 13pt; color: #49515d; max-width: 760px; }
    .sourceBox {
      border: 1px solid #d8dde3;
      border-radius: 8px;
      padding: 14px;
      background: #f7f9fa;
      font-family: "Consolas", "Courier New", monospace;
      font-size: 8.5pt;
      overflow-wrap: anywhere;
    }
    .pageBreak { break-before: page; }
    .eyebrow {
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #0f7f83;
      font-weight: 700;
      font-size: 8.5pt;
    }
    .metaGrid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
      margin: 16px 0 12px;
    }
    .metaGrid div {
      border: 1px solid #dce2e8;
      border-radius: 7px;
      padding: 9px 10px;
      min-height: 52px;
    }
    .metaGrid strong {
      display: block;
      color: #687280;
      font-size: 8pt;
      text-transform: uppercase;
      margin-bottom: 4px;
    }
    .metaGrid span { overflow-wrap: anywhere; }
    .takeaway {
      margin: 12px 0 4px;
      padding: 12px 14px;
      border-left: 4px solid #e88f4f;
      background: #fff7f0;
    }
    .turn {
      break-inside: avoid;
      border-top: 1px solid #e4e8ec;
      padding: 9px 0 7px;
    }
    .turnMeta {
      display: flex;
      gap: 8px;
      align-items: center;
      color: #657180;
      font-size: 8.5pt;
      margin-bottom: 5px;
    }
    .turnIndex {
      width: 22px;
      height: 22px;
      border-radius: 50%;
      display: inline-grid;
      place-items: center;
      background: #edf6f6;
      color: #0c7476;
      font-weight: 700;
    }
    .role {
      font-weight: 700;
      color: #252a31;
    }
    .role.assistant { color: #8b4b13; }
    .role.user { color: #075f66; }
    .transcriptTurn {
      break-inside: avoid;
      border-top: 1px solid #e4e8ec;
      padding: 10px 0;
    }
    .transcriptTurn h4 span {
      color: #7a8491;
      font-weight: 400;
      margin-left: 8px;
      font-size: 8pt;
    }
    pre {
      white-space: pre-wrap;
      margin: 0;
      font-family: "Consolas", "Courier New", monospace;
      font-size: 7.7pt;
      line-height: 1.32;
      color: #252a31;
    }
    .appendixIntro {
      color: #4c5663;
      margin-bottom: 12px;
    }
  </style>
</head>
<body>
  <section class="cover">
    <div class="coverTop">
      <div class="eyebrow">Rapport local InGen</div>
      <h1>Historique synthétique de deux sessions</h1>
      <p class="subtitle">Adaptation roman cinéma · Pensée sociale Ricœur</p>
    </div>
    <div>
      <p>Document construit a partir de l'archive locale InGenRuntime. Il preserve une lecture utile: synthese, chronologie, puis transcript complet en annexe.</p>
      <div class="sourceBox">
        Source: ${escapeHtml(sourcePath)}<br />
        Archive SHA-256: ${sourceHash}<br />
        Selection SHA-256: ${selectedHash}<br />
        Genere le: ${generatedAt}
      </div>
    </div>
  </section>

  ${sessions.map(sessionSection).join("")}

  <section class="pageBreak">
    <div class="eyebrow">Annexe</div>
    <h2>Transcript complet</h2>
    <p class="appendixIntro">Les messages ci-dessous sont repris dans l'ordre de l'archive locale. Les reponses assistant longues sont conservees pour verification, meme si la lecture principale se fait dans les syntheses precedentes.</p>
    ${sessions.map((session) => `
      <section class="pageBreak">
        <h3>${escapeHtml(session.title)}</h3>
        ${transcriptHtml(session)}
      </section>
    `).join("")}
  </section>
</body>
</html>`;
}

async function main() {
  const raw = await readFile(sourcePath, "utf8");
  const archive = JSON.parse(raw);
  const sessions = targetSessionIds.map((sessionId) => {
    const session = archive.sessions?.find((candidate) => candidate.sessionId === sessionId);
    if (!session) {
      throw new Error(`Session not found: ${sessionId}`);
    }
    return session;
  });
  await mkdir(outputDir, { recursive: true });
  const html = buildHtml(sessions, sha256(raw));
  await writeFile(htmlPath, html, "utf8");

  await writeFile(electronPrintHelperPath, `const { app, BrowserWindow } = require("electron");
const { writeFileSync } = require("node:fs");

const [, , htmlPath, pdfPath] = process.argv;

app.whenReady().then(async () => {
  const window = new BrowserWindow({
    show: false,
    width: 1240,
    height: 1754,
    webPreferences: {
      sandbox: true,
      contextIsolation: true,
      nodeIntegration: false
    }
  });
  await window.loadFile(htmlPath);
  await new Promise((resolve) => setTimeout(resolve, 500));
  const data = await window.webContents.printToPDF({
    pageSize: "A4",
    printBackground: true,
    margins: { marginType: "default" }
  });
  writeFileSync(pdfPath, data);
  await app.quit();
}).catch((error) => {
  console.error(error);
  app.exit(1);
});
`, "utf8");

  await printPdfWithElectron(htmlPath, pdfPath);
  await rm(electronPrintHelperPath, { force: true });

  console.log(JSON.stringify({ htmlPath, pdfPath, sessions: sessions.length }, null, 2));
}

function printPdfWithElectron(inputHtmlPath, outputPdfPath) {
  const electronExe = resolve(repoRoot, "examples", "ingen_electron_shell", "node_modules", "electron", "dist", "electron.exe");
  return new Promise((resolvePromise, reject) => {
    const child = spawn(electronExe, [electronPrintHelperPath, inputHtmlPath, outputPdfPath], {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString();
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise();
      } else {
        reject(new Error(`Electron PDF print failed with code ${code}\n${stdout}\n${stderr}`));
      }
    });
  });
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
