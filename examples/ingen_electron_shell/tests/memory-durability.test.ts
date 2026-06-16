import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const preloadSource = readFileSync(join(process.cwd(), "src", "preload", "preload.ts"), "utf8");
const brainStoreSource = readFileSync(join(process.cwd(), "src", "renderer", "brain-user-memory-store.ts"), "utf8");
const ipcContractSource = readFileSync(join(process.cwd(), "src", "shared", "ipc-contract.ts"), "utf8");
const launcherSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.cmd"), "utf8");
const launcherVbsSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.vbs"), "utf8");

describe("canonical memory durability", () => {
  it("forces one userData root for every Electron launch path", () => {
    expect(mainSource).toContain('const INGEN_CANONICAL_USER_DATA_DIR_NAME = eventTextLabMode ? "InGenEventTextLabRuntime" : "InGenRuntime"');
    expect(mainSource).toContain('app.setPath("userData", canonicalUserDataDir)');
    expect(mainSource).toContain('app.setPath("sessionData", join(canonicalUserDataDir, "session-data"))');
    expect(launcherSource).toContain("set INGEN_ELECTRON_USER_DATA_DIR=%APPDATA%\\InGenRuntime");
    expect(launcherSource).toContain('"--user-data-dir=%INGEN_ELECTRON_USER_DATA_DIR%"');
    expect(launcherVbsSource).toContain('run_ingen_electron_shell.cmd');
    expect(launcherVbsSource).toContain("always delegate freshness");
    expect(launcherVbsSource).not.toContain("AppActivate");
    expect(launcherVbsSource).not.toContain("INGEN_ELECTRON_DESKTOP_FAST_PATH");
  });

  it("treats legacy memory roots only as migration inputs", () => {
    expect(mainSource).toContain("function legacyUserDataDirs()");
    expect(mainSource).toContain("function memoryLegacyRootDirs()");
    expect(mainSource).toContain("reconcileCanonicalMemoryStore()");
    expect(mainSource).toContain("reconcileCanonicalBrainIdentityStore()");
    expect(mainSource).toContain("reconcileCanonicalChatArchiveStore()");
    expect(mainSource).toContain('schema: "ingen.memory.canonical_store.v1"');
    expect(mainSource).toContain("sourceOfTruth: memoryRootPath()");
    expect(mainSource).toContain("Legacy roots are migration inputs only.");
  });

  it("writes session and Brain memory with backups and atomic replacement", () => {
    expect(mainSource).toContain("function backupMemoryFile");
    expect(mainSource).toContain("function atomicWriteJsonFile");
    expect(mainSource).toContain("await handle.writeFile(json, \"utf8\")");
    expect(mainSource).toContain("await handle.sync()");
    expect(mainSource).toContain("await rename(tempPath, filePath)");
    expect(mainSource).toContain('await atomicWriteJsonFile(brainIdentityStorePath(), brainIdentityContext, "brain-identity")');
    expect(mainSource).toContain('await persistMergedChatArchive(Array.from(chatArchiveSessions.values()), "chat-archive")');
    expect(mainSource).not.toContain(".slice(0, 200);\n  const envelope = {\n    schema: \"forge.brain.chat_archive_store.v1\"");
  });

  it("keeps session assets inside the canonical Brain store", () => {
    expect(mainSource).toContain("function canonicalSessionAssetPath");
    expect(mainSource).toContain('join(memoryRootPath(), "session-assets"');
    expect(mainSource).toContain("function persistArchiveAttachmentLocalCopy");
    expect(mainSource).toContain("copyFileSync(sourcePath, targetPath)");
    expect(mainSource).toContain("return cached ? persistArchiveAttachmentLocalCopy(preview, cached.path, cached.mimeType) : preview");
    expect(mainSource).toContain("function normalizeArchiveSessionAssets");
  });

  it("projects canonical chat archive sessions ahead of demo sidebar rows", () => {
    expect(mainSource).toContain("const archiveIds = new Set(chatArchiveSessions.keys())");
    expect(mainSource).toContain(".sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))");
    expect(mainSource).toContain("localChatSessions.splice(0, localChatSessions.length, ...restored, ...transient)");
    expect(mainSource).toMatch(
      /function backendSessionItems\(\): SidebarSessionItem\[]\s*\{\s*ensureAssistantPatternDemoSession\(\);\s*if \(chatArchiveLoaded\) \{\s*syncLocalChatSessionsFromArchive\(\);/
    );
    expect(mainSource).not.toContain("localChatSessions.push(...restored)");
  });

  it("persists renderer Brain registries through the canonical main-process store", () => {
    expect(ipcContractSource).toContain("interface BrainCanonicalMemorySnapshot");
    expect(mainSource).toContain('return join(memoryRootPath(), "renderer-brain-memory.json")');
    expect(mainSource).toContain("function mergeBrainCanonicalMemorySnapshotToDisk");
    expect(mainSource).toContain('ipcMain.handle("forge:get-brain-memory-snapshot"');
    expect(mainSource).toContain('ipcMain.handle("forge:merge-brain-memory-snapshot"');
    expect(preloadSource).toContain("getBrainMemorySnapshot()");
    expect(preloadSource).toContain("mergeBrainMemorySnapshot(snapshot");
    expect(brainStoreSource).toContain("export async function hydrateCanonicalBrainMemory");
    expect(brainStoreSource).toContain("persistCanonicalBrainMemorySnapshot()");
    expect(brainStoreSource).toContain("readBrainLearningMemoryEntries()");
    expect(brainStoreSource).toContain("readBrainCustomCodeActs()");
    expect(brainStoreSource).toContain("readBrainSpecializedBrains()");
    expect(brainStoreSource).toContain("void hydrateCanonicalBrainMemory()");
  });

  it("copies agent-created files into a canonical session artifact ledger", () => {
    expect(mainSource).toContain('return join(memoryRootPath(), "session-artifacts.json")');
    expect(mainSource).toContain('return join(memoryRootPath(), "session-artifacts")');
    expect(mainSource).toContain('schema: "ingen.session_artifact.ledger.v1"');
    expect(mainSource).toContain("function persistAgentActionSessionArtifacts");
    expect(mainSource).toContain("function executeTrackedAgentAction");
    expect(mainSource).toContain("await persistAgentActionSessionArtifacts(request, result)");
    expect(mainSource).toContain("await copyFile(originalPath, canonicalPath)");
    expect(mainSource).toContain("sha256File(originalPath)");
    expect(mainSource).toContain("sessionArtifactLedgerPath: sessionArtifactLedgerStorePath()");
    expect(mainSource).toContain("await ensureSessionArtifactLedgerOnDisk()");
    expect(mainSource).toContain("return executeTrackedAgentAction(request)");
    expect(mainSource).not.toContain("return executeAgentActionRequest(agentActionHostConfig(), request)");
  });
});
