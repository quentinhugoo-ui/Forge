import { describe, expect, it } from "vitest";
import audit from "../src/shared/generated/final-cutover-audit.generated.json";

describe("final cutover audit", () => {
  it("keeps the complete Electron shell cut over to Rust backend authority", () => {
    expect(audit.schema).toBe("ingen.electron.final_cutover_audit.v2");
    expect(audit.status).toBe("cutover_complete");
    expect(audit.blockers).toEqual([]);
    expect(audit.proof_hash).toMatch(/^[a-f0-9]{64}$/);
  });

  it("locks the Electron-only product shell", () => {
    const passed = new Map(audit.checks.map((check) => [check.id, check.passed]));
    expect(passed.get("electron_product_shell_only")).toBe(true);
    expect(passed.get("typed_ipc_contract_generated")).toBe(true);
    expect(passed.get("electron_security_defaults")).toBe(true);
    expect(passed.get("strict_context_bridge_ipc")).toBe(true);
    expect(passed.get("rust_backend_projection_required")).toBe(true);
    expect(passed.get("electron_launcher_present")).toBe(true);
  });

  it("keeps renderer product slices present", () => {
    const passed = new Map(audit.checks.map((check) => [check.id, check.passed]));
    expect(passed.get("renderer_slice_files_present")).toBe(true);
  });
});
