import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

const mainSource = readFileSync(join(process.cwd(), "src", "main", "main.ts"), "utf8");
const launcherSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.cmd"), "utf8");
const launcherVbsSource = readFileSync(join(process.cwd(), "run_ingen_electron_shell.vbs"), "utf8");

describe("canonical memory durability", () => {
  it("forces one userData root for every Electron launch path", () => {
    expect(mainSource).toContain('const INGEN_CANONICAL_USER_DATA_DIR_NAME = eventTextLabMode ? "InGenEventTextLabRuntime" : "InGenRuntime"');
    expect(mainSource).toContain('app.setPath("userData", canonicalUserDataDir)');
    expect(mainSource).toContain('app.setPath("sessionData", join(canonicalUserDataDir, "session-data"))');
    expect(launcherSource).toContain("set INGEN_ELECTRON_USER_DATA_DIR=%APPDATA%\\InGenRuntime");
    expect(launcherSource).toContain('"--user-data-dir=%INGEN_ELECTRON_USER_DATA_DIR%"');
    expect(launcherVbsSource).toContain('runtimeUserData = shell.ExpandEnvironmentStrings("%APPDATA%") & "\\InGenRuntime"');
    expect(launcherVbsSource).toContain('env("INGEN_ELECTRON_USER_DATA_DIR") = runtimeUserData');
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
});
