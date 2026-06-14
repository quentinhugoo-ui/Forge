import { describe, expect, it } from "vitest";
import {
  AGENT_BROWSER_DOWNLOAD_COMMAND,
  AGENT_BROWSER_INSPECT_COMMAND,
  AGENT_BROWSER_OPEN_COMMAND,
  AGENT_BROWSER_CLICK_COMMAND,
  AGENT_BROWSER_PLAYWRIGHT_DOWNLOAD_COMMAND,
  AGENT_BROWSER_PLAYWRIGHT_INSPECT_COMMAND,
  AGENT_BROWSER_SCREENSHOT_COMMAND,
  AGENT_BROWSER_TYPE_TEXT_COMMAND,
  AGENT_COPY_PATH_COMMAND,
  AGENT_AUTOMATION_CANCEL_COMMAND,
  AGENT_AUTOMATION_LIST_COMMAND,
  AGENT_AUTOMATION_SCHEDULE_COMMAND,
  AGENT_CLOUD_INSPECT_COMMAND,
  AGENT_CLOUD_READONLY_COMMAND,
  AGENT_CLOUD_WRITE_COMMAND,
  AGENT_DELETE_TREE_COMMAND,
  AGENT_DOCUMENT_CONVERT_COMMAND,
  AGENT_DOCUMENT_IMAGE_OCR_COMMAND,
  AGENT_DOCUMENT_INSPECT_COMMAND,
  AGENT_DOCUMENT_MEDIA_METADATA_COMMAND,
  AGENT_DOCUMENT_OFFICE_EXPORT_PDF_COMMAND,
  AGENT_DOCUMENT_OFFICE_INSPECT_COMMAND,
  AGENT_DOCUMENT_PDF_EXTRACT_COMMAND,
  AGENT_DOCUMENT_WRITE_COMMAND,
  AGENT_AUTOMATION_RECORD_COMMAND,
  AGENT_DEV_CHECK_COMMAND,
  AGENT_DEV_COMMIT_COMMAND,
  AGENT_DEV_DIFF_COMMAND,
  AGENT_DEV_PUSH_COMMAND,
  AGENT_DEV_STATUS_COMMAND,
  AGENT_GITHUB_PR_CREATE_COMMAND,
  AGENT_VIRTUALIZATION_INSPECT_COMMAND,
  AGENT_VIRTUALIZATION_RUN_COMMAND,
  AGENT_APPSHOT_COMMAND,
  AGENT_COMPUTER_INSPECT_COMMAND,
  AGENT_CLICK_COMMAND,
  AGENT_DRAG_COMMAND,
  AGENT_FOCUS_WINDOW_COMMAND,
  AGENT_LIST_COMMAND,
  AGENT_OCR_COMMAND,
  AGENT_READONLY_SHELL_COMMAND,
  AGENT_SCROLL_COMMAND,
  AGENT_SEARCH_COMMAND,
  AGENT_SHELL_COMMAND,
  AGENT_TYPE_TEXT_COMMAND,
  AGENT_UI_TREE_COMMAND,
  agentActionEventFromLine
} from "../src/renderer/agent-action-events";

describe("agent action transcript events", () => {
  it("maps direct event commands to transcript event copy", () => {
    expect(agentActionEventFromLine(`${AGENT_SEARCH_COMMAND} query="needle"`)).toEqual({
      command: AGENT_SEARCH_COMMAND,
      text: "computer search returned bounded matches"
    });
  });

  it("maps structured action kinds and tool ids to event commands", () => {
    expect(agentActionEventFromLine('AGENT_ACTION action="list" path="."')?.command).toBe(AGENT_LIST_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="fs.copy" path="a" toPath="b"')?.command).toBe(AGENT_COPY_PATH_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="shell.readonly" command="rg"')?.command).toBe(AGENT_READONLY_SHELL_COMMAND);
    expect(agentActionEventFromLine("executeAgentAction fs.delete_tree confirmed=true")?.command).toBe(AGENT_DELETE_TREE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="run_command" command="powershell.exe"')?.command).toBe(AGENT_SHELL_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_inspect"')?.command).toBe(AGENT_COMPUTER_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="computer.appshot"')?.command).toBe(AGENT_APPSHOT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="computer.focus_window"')?.command).toBe(AGENT_FOCUS_WINDOW_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_ui_tree"')?.command).toBe(AGENT_UI_TREE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="computer.ocr"')?.command).toBe(AGENT_OCR_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_click"')?.command).toBe(AGENT_CLICK_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_type_text"')?.command).toBe(AGENT_TYPE_TEXT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_scroll"')?.command).toBe(AGENT_SCROLL_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="computer_drag"')?.command).toBe(AGENT_DRAG_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_inspect_url"')?.command).toBe(AGENT_BROWSER_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="browser.download"')?.command).toBe(AGENT_BROWSER_DOWNLOAD_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="browser.open_url"')?.command).toBe(AGENT_BROWSER_OPEN_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_playwright_inspect"')?.command).toBe(AGENT_BROWSER_PLAYWRIGHT_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_screenshot"')?.command).toBe(AGENT_BROWSER_SCREENSHOT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_click"')?.command).toBe(AGENT_BROWSER_CLICK_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_type_text"')?.command).toBe(AGENT_BROWSER_TYPE_TEXT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="browser_playwright_download"')?.command).toBe(AGENT_BROWSER_PLAYWRIGHT_DOWNLOAD_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="document_inspect" path="report.md"')?.command).toBe(AGENT_DOCUMENT_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="document.write_json" path="data.json"')?.command).toBe(AGENT_DOCUMENT_WRITE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="document.convert_text"')?.command).toBe(AGENT_DOCUMENT_CONVERT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="document_pdf_extract_text" path="report.pdf"')?.command).toBe(AGENT_DOCUMENT_PDF_EXTRACT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="document.office_inspect"')?.command).toBe(AGENT_DOCUMENT_OFFICE_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="document_office_export_pdf"')?.command).toBe(AGENT_DOCUMENT_OFFICE_EXPORT_PDF_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="document.image_ocr"')?.command).toBe(AGENT_DOCUMENT_IMAGE_OCR_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="document_media_metadata"')?.command).toBe(AGENT_DOCUMENT_MEDIA_METADATA_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="dev_repo_status"')?.command).toBe(AGENT_DEV_STATUS_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="dev.git_diff"')?.command).toBe(AGENT_DEV_DIFF_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="dev_git_commit"')?.command).toBe(AGENT_DEV_COMMIT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="dev.git_push"')?.command).toBe(AGENT_DEV_PUSH_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="dev.github_pr_create"')?.command).toBe(AGENT_GITHUB_PR_CREATE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="dev.run_check"')?.command).toBe(AGENT_DEV_CHECK_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="cloud_cli_inspect"')?.command).toBe(AGENT_CLOUD_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="cloud.run_readonly"')?.command).toBe(AGENT_CLOUD_READONLY_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="cloud_cli_run_write"')?.command).toBe(AGENT_CLOUD_WRITE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="virtualization_inspect"')?.command).toBe(AGENT_VIRTUALIZATION_INSPECT_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="virtualization.run_command"')?.command).toBe(AGENT_VIRTUALIZATION_RUN_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="automation_schedule"')?.command).toBe(AGENT_AUTOMATION_SCHEDULE_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION tool="automation.list"')?.command).toBe(AGENT_AUTOMATION_LIST_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION capability="automation.cancel"')?.command).toBe(AGENT_AUTOMATION_CANCEL_COMMAND);
    expect(agentActionEventFromLine('AGENT_ACTION action="automation_record"')?.command).toBe(AGENT_AUTOMATION_RECORD_COMMAND);
  });

  it("keeps path metadata for file modification event cards", () => {
    expect(agentActionEventFromLine(`${AGENT_COPY_PATH_COMMAND} path="src/App.tsx" toPath="src/App.copy.tsx"`)).toMatchObject({
      command: AGENT_COPY_PATH_COMMAND,
      path: "src/App.tsx",
      toPath: "src/App.copy.tsx"
    });
  });

  it("ignores unrelated prose", () => {
    expect(agentActionEventFromLine("Je vais chercher dans les fichiers avant de modifier.")).toBeUndefined();
  });
});
