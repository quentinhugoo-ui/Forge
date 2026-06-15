# Agent OS Capability Plan

Canonical objective: make InGen's local agent layer able to control the computer with the same class of capability as Codex Desktop or Claude Code Desktop, while keeping every action verifiable, permissioned, auditable and reversible when possible.

This document is the implementation map for the universal desktop-agent pipeline. It is not a desktop-cleanup plan. File organization is only one test case among many.

## Frontier Frame

Wall being pushed: sandbox reach, agent autonomy, context size, verification quality, UI/event clarity and developer experience.

Frontier hypothesis: a compact capability atlas plus a universal observe-act-verify-retry loop can give the LLM broad Windows reach without bloating every prompt or faking actions.

Current sources used as the floor:

- OpenAI Codex app: local tasks, sandboxing, computer use, appshots, foreground GUI assistance, automations and background work.
- Anthropic Claude Code: tools, permissions, hooks, skills, MCP, subagents, common workflows and long-running coding loops.
- Model Context Protocol: tool listing/calling as a standard external tool surface.
- Microsoft Windows documentation: PowerShell, Windows commands, WinGet, WMI/CIM, Task Scheduler, Event Logs, registry, UI Automation, Settings URI, WSL, Hyper-V, COM/VBA and security boundaries.
- Recent computer-use research direction: OSWorld, OSWorld-MCP, WindowsWorld and MCPWorld style benchmarks, which show that robust agents need both structured tools and GUI/desktop interaction.

Promotion rule: every new capability starts behind a narrow interface, explicit risk level, verification probe and deterministic tests. It graduates only when it beats the current path on capability, clarity, speed or verifiability.

## Non-Negotiable Safety

- Protect `C:\Users\quent\Documents\EVE\MAP`.
- Never bypass UAC, Windows Security, browser security prompts, payment confirmations or credential prompts.
- Never silently read, export or exfiltrate secrets, private keys, browser passwords, tokens, cookies or credential-manager entries.
- Never recursively delete or move without resolved absolute path guards.
- Preserve unrelated user changes.
- Prefer reversible actions, snapshots, logs, manifests and proof hashes.
- Any action with system, network, installer, registry, credential, security, admin, destructive, cloud-write or external-submission impact is approval-gated.

## Current State

Implemented foundation:

- `AgentCapabilityAtlas` exists as a non-executable knowledge layer.
- Existing executable actions remain unchanged: `fs.list`, `fs.search`, `fs.create_directory`, `fs.rename`, `fs.move`, `fs.copy`, `fs.delete_empty_directory`, `fs.delete_tree`, `shell.readonly`, `shell.full`.
- The manifest now exposes planned/boundary capabilities such as WMI, settings, credentials, browser CDP, Office COM, RPA, cloud CLIs, MCP/plugins and automations.
- Git/PR, InGen-owned Windows Task Scheduler actions, WSL/Docker/Hyper-V virtualization, Windows admin read/write guards, package manager probes and CI inspection have graduated to verified executable routes where local tools are available.
- `shell.full` remains the confirmed universal Windows escape hatch.

Current limitation:

- Several atlas entries are still boundary knowledge rather than direct executable backends, especially MCP, deep account-changing browser flows, bundled OCR/media toolchains and destructive system administration.
- The loop still has legacy file-organization compatibility logic; it must not become the general model.
- The event layer can show tool activity, but the next phases must make every event correspond to verified action state.

## Capability Taxonomy

The agent OS must model capabilities by family, surface, status, risk, operations, tools, fallbacks, verification and approval.

Required families:

- Filesystem discovery: list, inspect metadata, locate files, enumerate directories.
- Filesystem search: content search, name search, indexed search, bounded whole-computer search.
- Filesystem mutation: create, copy, move, rename, write, patch, extract, archive, sync.
- Filesystem destructive: empty-directory delete, tree delete, recycle-bin routing when available, cleanup with guards.
- Shell readonly: `rg`, `git status`, `git diff`, `where`, `Get-ChildItem`, `Get-Content`, safe inspection.
- Shell full: PowerShell, CMD, Batch, Windows commands and arbitrary confirmed local commands.
- Package management: WinGet, installer executables, app updates, uninstall flows, version checks.
- Windows registry: query, export, set, delete values, policy/settings discovery.
- Windows services: inspect, start, stop, restart, set startup mode.
- Windows processes: list, start, stop, wait, monitor, inspect command line.
- Windows scheduler: query, create, update, disable, delete scheduled tasks.
- WMI/CIM: OS, hardware, devices, software, drivers, services and system state queries.
- Event logs and diagnostics: Event Viewer logs, application logs, ETW/performance counters where available.
- Network and firewall: adapters, routes, DNS, proxy, VPN, connectivity, firewall rules.
- Windows Settings: `ms-settings:` URIs, Control Panel applets, settings verification.
- Security boundaries: UAC, Defender, firewall, BitLocker, certificates, credentials, local users/groups, ACLs.
- UI Automation and computer use: screenshots, OCR, accessibility tree, clicks, typing, drag/drop, focus, clipboard, multi-monitor state.
- RPA: Power Automate Desktop or similar repeatable GUI workflows.
- Browser automation: Chrome DevTools Protocol, Playwright, Electron WebContents, downloads, page state, network logs.
- Office and COM: Excel, Word, PowerPoint, Outlook, COM automation, VBA object models, exports.
- Documents and media: Markdown, TXT, JSON, CSV, Excel, Word, PDF, PowerPoint, images, audio, video, archives, OCR, conversion.
- Developer workflows: code search, edits, tests, builds, dependencies, Git, branches, worktrees, commits, PRs, CI, reviews, release notes.
- Virtualization: WSL, Docker, Hyper-V, containers, VMs, distro import/export.
- Cloud CLIs and connectors: `gh`, `az`, `aws`, `gcloud`, authenticated MCP/connectors.
- MCP/plugins/skills/hooks/subagents: tool discovery, external tool calls, delegated research and automation hooks.
- Automations and goals: scheduled follow-ups, polling, monitors, resumable tasks, background work and thread wakeups.

Family IDs that must remain covered by the atlas:

- `filesystem.discovery`
- `filesystem.search`
- `filesystem.mutation`
- `filesystem.destructive`
- `shell.readonly`
- `shell.full`
- `package.manager`
- `windows.registry`
- `windows.services`
- `windows.processes`
- `windows.scheduler`
- `windows.wmi`
- `windows.event_logs`
- `windows.network`
- `windows.settings`
- `windows.credentials`
- `windows.certificates`
- `browser.cdp`
- `computer.ui_automation`
- `automation.rpa`
- `office.com`
- `documents.media`
- `dev.git`
- `virtualization.wsl`
- `virtualization.hyperv_docker`
- `cloud.clis`
- `mcp.plugins`
- `automations.goals`
- `security.admin_boundary`

## Universal Loop Contract

Every task must use the same loop:

1. Observe: read user intent, current transcript, runtime manifest, capability atlas and relevant local state.
2. Explain: write one short, natural progress paragraph in plain language.
3. Act: emit exactly one real action request when execution is needed.
4. Capture: collect stdout, stderr, exit code, paths, artifacts, UI state, screenshots, logs or MCP result.
5. Reinject: send the exact compact tool result back to the LLM on the next round.
6. Verify: check the goal through an independent observation.
7. Retry: if the result is insufficient, choose another safe route.
8. Stop: conclude only when verified, approval-blocked, impossible with current tools, failed after a real retry, or explicitly stopped by the user.
9. Summarize: final answer states what changed, how it was verified, what failed and what remains.

The loop must never claim success from intention alone. It must never show fake tool events. Tool events are rendered by the app from real runtime events.

## Ten-Step Implementation Plan

### 1. Capability Atlas Total

Goal: give the LLM a complete map of what the local agent can do, can try, cannot yet do and must never do silently.

Deliverables:

- Extensible `AgentCapabilityAtlasEntry`.
- `createAgentCapabilityAtlas(config)`.
- Compact `capability_atlas=` prompt summary.
- Executable read-only `/capabilities_` CodeAct available from General, Science and Coding Brain.
- Explicit distinction between executable actions and non-executable atlas knowledge.
- Tests for Office COM, browser CDP, WMI, scheduler, settings, credentials, WSL and RPA.

Acceptance:

- At least 15 capability families are present.
- Sensitive families use `approval: prompt` or `approval: blocked`.
- Existing executable actions still work.
- Planned entries do not become direct `AGENT_ACTION_JSON` actions.

Status: implemented in commit `58d53fcbf`.

Update 2026-06-15: `/capabilities_` is now a real read-only `AGENT_ACTION_JSON {"action":"capabilities"}` backend, not just prompt text. It returns a scoped/ranked atlas slice with manifest hash, atlas hash, installed/missing tools, planned/blocked/approval-gated families and proof hash. Renderer events map it to `/agent_capabilities_` so the readable loop stream shows an atlas event instead of raw JSON.

### 2. Runtime Manifest And Prompt Injection

Goal: inject the right operational context without wasting tokens.

Deliverables:

- Split static Brain routing from dynamic runtime manifest.
- Add a manifest hash and delta policy.
- Include current workspace, platform, protected roots, executable actions, planned capabilities, approvals and recent failures.
- Include only compact summaries in normal rounds; expose detailed entries only when relevant.
- Re-inject updated tool result and selected capability context after every action.

Acceptance:

- The LLM sees enough context to choose tools without receiving the whole atlas every turn.
- The manifest says which capabilities are available, planned, blocked or approval-gated.
- Context compaction preserves the active goal, recent tool results, capability constraints and proof trail.

Status: implemented for the planned scope. The manifest carries runtime hashes, atlas hashes, delta policy, installed/missing tool detection, prompt token estimates, selected-capability detail on tool-result continuation and agent-action compaction state. The full local action atlas is injected in the Brain boot system message once at session start and after conversation compaction; normal provider calls use only a compact runtime reminder that points the LLM to `/capabilities_` when it needs a fresh targeted atlas. Science and Coding Brain catalogs mention `/capabilities_` but are still injected only on segment switch and after compaction.

### 3. Universal Agent Loop

Goal: replace domain-specific behavior with a single robust local-action loop.

Deliverables:

- `AgentActionLoopState` with objective, step count, observations, last result, retries, approvals and final status.
- Loop outcome states: `completed`, `needs_approval`, `blocked`, `failed_after_retries`, `max_steps`, `cancelled`.
- Remove or quarantine deterministic desktop-cleanup behavior as a compatibility fallback/test fixture.
- Enforce final summary after any tool-using task.
- Enforce no-final-summary while a required local action remains.

Acceptance:

- Desktop cleanup, code edit, package inspection, Windows setting, browser task and document task all follow the same loop.
- Read-only discovery does not satisfy a mutation request.
- The loop tries a safe alternative before blocking.

Status: implemented for the planned scope. The normal path now carries `AgentActionLoopState`, explicit terminal outcomes, result observations, retry/approval accounting, proof hashes and mandatory English final summaries after tool-using tasks. The runtime no longer stops on a fixed local-action step ceiling; it continues until the model reaches the objective, reports a real block/approval/failure condition, or the user stops the active run. Deterministic desktop organization is quarantined behind `INGEN_AGENT_ACTION_COMPAT_FALLBACK=1` as compatibility behavior, while normal mutation follow-up stays inside the universal loop and blocks with a final status when the model stops without the required next action.

Update 2026-06-15: Brain CodeActs are now treated as loop-stream event boundaries, not merely visible prompt text. General, Science and Coding Brain prompts tell the model to use CodeActs as progress events for non-trivial work. After a non-terminal Brain CodeAct on an actionable request, the host can automatically issue a hidden `BRAIN_CODEACT_LOOP_CONTINUATION` turn so the next concrete CodeAct or `AGENT_ACTION_JSON` action happens inside the same agentic flow instead of ending as a normal chatbot answer. User-pause CodeActs such as questionnaire/workspace/image and surface-opening CodeActs remain allowed to stop cleanly while waiting for user or UI state.

Update 2026-06-15: the chat submit path now uses a single `executeUniversalLoopOrchestratorPass` followed by `executeUniversalLoopContinuation` instead of separate continuation blocks for Brain switching and Brain CodeActs. Each pass runs the same ordered circuit: agent actions, fallback projections, module CodeActs, Brain rules/events, questionnaire pause enforcement, Brain segment execution and verified display injection. The continuation selector can resume after a Brain switch, resume after an actionable CodeAct, pause cleanly for `/workspace_` and `/questionnaire_`, or stop and leave the assistant response as a normal single answer when no loop continuation is needed.

Update 2026-06-15: non-Brain CodeAct events now share the same visual lifecycle as local agent actions: a `working` phase while the streamed event is active, with the existing animated icon treatment, then a `complete` phase that settles into the muted chatbar-gray event line. Brain creation/change/modification events keep their dedicated blue Brain treatment instead of being folded into the generic gray lifecycle.

Update 2026-06-15: the deterministic desktop/file-organization fallback is no longer part of the normal runtime path; it can run only behind `INGEN_AGENT_ACTION_COMPAT_FALLBACK=1`. `executeUniversalLoopContinuation` is now a controlled loop: after each Brain switch or actionable CodeAct continuation it re-runs the same orchestrator pass, then decides again whether to continue, pause/stop, or exit on a repeated continuation key.

### 4. Windows Execution Layer

Goal: make Windows-native control reliable through structured adapters, not ad hoc shell text.

Deliverables:

- Typed execution adapters for PowerShell, CMD, Windows commands and `shell.full`.
- Structured command result: command line, cwd, stdout, stderr, exit code, duration, timeout, artifacts, observed changes.
- Windows route catalog: `winget`, `reg.exe`, `schtasks`, `netsh`, `dism`, `sc.exe`, `tasklist`, `taskkill`, `robocopy`, `icacls`, `certutil`, `wevtutil`, `wsl.exe`, `Start-Process`, `ms-settings:`.
- Timeout and cancellation policy.
- Confirmation policy for computer-wide writes.

Acceptance:

- Each adapter has at least one read scenario and one gated write scenario where safe.
- Failed command output is reinjected for retry.
- `shell.full` remains available but structured adapters are preferred when possible.

Status: implemented for the planned scope. The host now exposes `AgentWindowsExecutionPolicy`, typed adapters (`powershell`, `cmd`, `windows_command`, `shell_full`), a Windows route catalog, timeout/cancellation policy, command confirmation policy and structured command results with adapter, route id, duration, timeout, timeout flag, stdout/stderr previews, artifacts and observed changes. `run_command` stays the confirmed universal execution action, while prompt/runtime manifests tell the model to prefer typed Windows routes before `shell_full`.

### 5. Verification And Retry Engine

Goal: guarantee that the app checks whether work actually happened.

Deliverables:

- `AgentVerificationProbe` types: filesystem, command exit, process state, service state, registry state, package state, browser state, UI state, event log, artifact hash, MCP result, manual confirmation.
- Retry strategy catalog: API/CLI, PowerShell, CMD, native command, WMI/CIM, registry, Settings URI, browser CDP, GUI/computer-use, manual approval.
- Failure taxonomy: denied, missing tool, bad path, timeout, permission, protected root, command error, unverifiable, partial success.
- Independent verification after every mutation.

Acceptance:

- The agent cannot report completion if verification fails.
- At least one alternate route is attempted for recoverable failures.
- Protected-root, credential and UAC boundaries block immediately.

Status: implemented for the planned scope. The host now exposes `AgentVerificationPolicy`, probe kinds, retry strategies and failure categories. Filesystem mutations create independent verification probes before returning success, command execution creates `command_exit` probes, verification failure forces `accepted:false`, protected roots produce `protected_root` without retry routes, and compact tool-result reinjection includes verification, failure category and retry route hints for the next loop turn.

### 6. Computer Use And GUI Control

Goal: control apps and Windows surfaces that do not expose clean APIs.

Deliverables:

- Screenshot/appshot capture.
- OCR and visual element summaries.
- UI Automation accessibility tree.
- Mouse, keyboard, scroll, drag/drop and clipboard operations.
- Window focus and multi-monitor state.
- Safe GUI event pacing and cancellation.
- Foreground user-presence mode for risky GUI actions.

Acceptance:

- The agent can inspect and interact with a simple GUI app.
- It verifies by screenshot or accessibility tree after interaction.
- It does not approve security, payment, credential or destructive prompts for the user.

Status: executable for the current Windows backend scope. The host now exposes `AgentComputerUsePolicy` with direct actions for bounded GUI inspection, confirmed appshot capture, confirmed window focus, confirmed clipboard read/write, read-only `computer_ui_tree`, confirmed `computer_ocr`, and prompt-gated `computer_click`, `computer_type_text`, `computer_scroll` and `computer_drag`. UI Automation tree inspection uses Windows UIAutomationClient with bounded depth/node count. OCR succeeds only with a detected local OCR engine, and `document_toolchain_inspect`/`document_toolchain_install` now make `tesseract.exe` availability/install verification explicit rather than silently depending on PATH. Input gestures require `confirmed:true`, foreground user presence, one-action pacing, before/after foreground snapshots and blocking for security, payment, credential, destructive, password/PIN/passkey, credit-card/checkout and UAC-like prompts.

Still planned/blocked: semantic screen target selection, bundled OCR when no local OCR engine exists, high-level RPA flow recording, and any attempt to approve UAC/security/payment/credential prompts on the user's behalf.

### 7. Browser, Web And Downloads

Goal: make web workflows controllable and verifiable.

Deliverables:

- Browser/CDP adapter for contained WebExplorer or external browser when approved.
- Page state, DOM state, screenshot, network logs and download tracking.
- Download validation: filename, size, hash when possible, signature/checksum when available.
- Form submission and account-change approval boundaries.
- Web-to-local artifact handoff.

Acceptance:

- The agent can navigate, inspect, download and verify a file.
- It asks for confirmation before external submissions, purchases, account changes or credential prompts.
- Downloads are referenced by path and artifact hash.

Status: executable for the current isolated Playwright backend scope. The host now exposes `AgentBrowserWebPolicy` with direct fetch actions plus `browser_playwright_inspect`, `browser_screenshot`, `browser_click`, `browser_type_text` and `browser_playwright_download`. Playwright actions launch a fresh headless Chromium context without profile credentials, collect bounded DOM/ARIA/network evidence, verify screenshots by file hash, and verify page-triggered downloads through Playwright's download event and persisted SHA-256 artifact. Browser click/type actions require `confirmed:true`; form-associated clicks require `formSubmissionConfirmed:true`, and password/credential/one-time-code/payment fields are blocked rather than silently filled.

Still planned/blocked: persistent browser sessions, reuse of authenticated browser profile state, contained WebExplorer DOM control, account-changing submissions/purchases without explicit confirmation, and MCP browser tools.

### 8. Documents, Office, Media And Data

Goal: support ordinary computer work beyond code.

Deliverables:

- Parsers/writers for Markdown, TXT, JSON, CSV and common structured files.
- Office/PDF/image/audio/video workflows through libraries, COM or external CLIs where available.
- Conversion and extraction pipeline with compact manifests.
- Hash/proof summaries for generated artifacts.
- Clear fallback when a proprietary app or codec is missing.

Acceptance:

- The agent can create, edit, convert and verify representative document artifacts.
- Large outputs are stored on disk and summarized compactly.
- Office COM and macros remain prompt-gated.

Status: executable for the local backends that have runtime proof. The host now exposes `AgentDocumentMediaPolicy`, executable document/data actions for artifact inspection, UTF-8 text/Markdown writes, JSON validation and pretty-write, RFC-4180-style CSV validation/write, bounded text/Markdown conversion, PDF text extraction through PDF.js, prompt-gated Office COM inspection/export-to-PDF, prompt-gated image OCR, media metadata and document toolchain management. `document_toolchain_inspect` detects `tesseract.exe`, `ffprobe.exe` and Office COM ProgIDs without claiming missing tools succeeded; `document_toolchain_install` can install/verify Tesseract OCR and FFmpeg/ffprobe through exact WinGet package ids with `confirmed:true`, then re-detects binaries before success. Office COM availability is probed without opening a document, Office document operations remain confirmed, macros are explicitly blocked, and Office install/update remains blocked because it is a licensed application install rather than a safe package-manager backend. Results include readback verification, command exit checks, size, SHA-256, parser status and compact document/media/toolchain summaries. Proprietary conversions beyond Office PDF export remain prompt-gated.

### 9. Developer, Cloud, MCP And Automation Surfaces

Goal: match Codex/Claude Code class workflows for coding and external tools.

Deliverables:

- Repo-safe code operations: inspect, edit, test, build, lint, format only when requested, commit, push, PR.
- Worktree/session isolation and preservation of unrelated changes.
- CI and log inspection.
- MCP tools/list and tools/call integration.
- Skills, hooks and subagent delegation.
- Cloud CLI integration with approval-gated writes.
- Long-running goals, monitors, reminders and scheduled tasks.

Acceptance:

- The agent can complete a code task with tests and Git hygiene.
- It can call an MCP tool and use the result in the same loop.
- Background work is visible, cancellable and summarized.

Status: partially executable beyond the planned scope. The host now exposes `AgentDeveloperAutomationPolicy` with direct GitHub/Git actions for repo status, diff, confirmed commit, confirmed push and confirmed PR creation. Commit stages only explicit `paths` or already-staged changes, then verifies a new `HEAD`; push verifies the remote branch head with `git ls-remote`; PR creation uses non-interactive `gh pr create` and verifies the resulting URL with `gh pr view`.

Windows scheduler automation is now a real backend for InGen-owned tasks. `automation.schedule` creates a visible Task Scheduler task through `schtasks /Create`, verifies it with `schtasks /Query`, and mirrors the proof into the append-only automation ledger. `automation.list` queries Task Scheduler and filters to `InGenAgent_` root tasks. `automation.cancel` deletes only `InGenAgent_` tasks through `schtasks /Delete`, verifies the task is gone, and appends a cancellation record. Creation and cancellation require `confirmed:true`; arbitrary system task names, folders and dangerous scheduler mutation remain blocked or require a separately confirmed shell route. If Windows denies Task Scheduler access or the backend is unavailable, the action returns a verified blocked/failure result and never emits a false scheduled/done state.

WSL, Docker and Hyper-V are now executable with explicit proof boundaries. `virtualization.inspect` probes WSL status/version/distributions, Docker version/container inventory and Hyper-V VM inventory without mutation, returning available/missing backend state as runtime evidence. `virtualization.run_command` can run a confirmed command through WSL, an existing Docker container or a named Hyper-V VM guest route, and verifies the exit code; if WSL is unavailable and `nativeFallback:true` is explicitly set, it can run the same confirmed command in the native workspace as a verified fallback. Hyper-V lifecycle commands for a named VM are confirmed, PowerShell-backed and verified by `Get-VM`; WSL distro import/export/unregister/install, Docker image/container lifecycle mutation and any destructive VM operation beyond the named confirmed route remain blocked or require a separately confirmed shell route.

Windows admin, package and CI routes are executable through narrow adapters. `windows_setting_inspect` reads OS or explicit registry state without CIM dependency; `windows_setting_apply` is limited to explicit `HKCU:\` value writes with `confirmed:true` and readback proof. `windows_sensitive_inspect` reads sensitive surfaces such as firewall, Defender, BitLocker and user environment state; `windows_sensitive_apply` is allowlisted, requires `confirmed:true`, supports verified user-env mutation and typed firewall enable/disable, and blocks unsupported security-weakening mutations before execution. `process_service_inspect` can inspect a process or service; `process_service_control` can start, stop or restart a named service only with `confirmed:true` and service-state proof. `package_inspect` reports WinGet availability/package state without claiming success when `winget.exe` is missing; `package_install_update` requires `confirmed:true`, an exact package id and command-exit verification. `ci_checks_inspect` and `ci_run_inspect` use `gh` for read-only checks/log inspection, `ci_rerun_failed` can rerun failed jobs only with `confirmed:true` and `gh run view` verification, and `dev_github_pr_review_submit` can submit approve/comment/request-changes reviews only with `confirmed:true` plus `gh pr view` review-state verification.

Cloud CLIs are now executable through explicit provider-scoped actions. `cloud_cli.inspect` detects `aws`, `az`, `gcloud`, `gh` and `stripe`, captures version/context probes with credential redaction, and reports missing tools as unavailable rather than failed success. `cloud_cli.run_readonly` allows only read-shaped commands and blocks credential/token/secret access. `cloud_cli.run_write` requires `confirmed:true`, keeps tenant/project/account context in the proof summary, verifies command exit, redacts credential-shaped output and blocks destructive verbs such as delete/remove/destroy/terminate/purge/revoke/cancel/logout. Real MCP `tools/list`/`tools/call`, subagents/hooks and non-scheduler thread wakeups remain planned connector backends rather than fake direct execution. MCP is intentionally out of scope for this session.

### 10. UX, Events, Audit And Benchmarks

Goal: make the agent's work legible, trustworthy and benchmarkable.

Deliverables:

- English event labels.
- Running-to-completed event transitions.
- Expandable command trees with exact commands, stdout/stderr, exit code and artifacts.
- File modification events with real added/removed counters.
- Context compaction event: centered, animated, then `context compressed`.
- Final summary event for every tool-using loop.
- Local benchmark suite covering filesystem, code, install/update, GUI, browser, document, Windows setting, process/service, scheduler, WSL/dev, Git/PR and blocked-danger cases.

Acceptance:

- Events are not fake prose; they are generated from runtime data.
- Paragraphs between events are natural, clear and non-jargon.
- The UI never says work was done when no observed state changed.
- Benchmarks prove success, retry and safe blocking behavior.

Status: implemented for the planned scope, with Git/PR, scheduler, virtualization, Windows admin, package/toolchain management, document/media, CI/review automation and cloud CLIs promoted to executable where local backends exist. Runtime events already render English labels, running-to-completed transitions, expandable command trees, file modification counters, context compaction markers and final loop summaries. The chat canvas now keeps the same animated agent status used by the initial "is thinking" state visible as a delayed, event-aware working indicator while an assistant run remains active between streamed paragraphs, tool calls and verification turns; it waits for a short quiet window, adapts text such as shell/file/search work from the latest event, avoids fake token or elapsed-time counters, lets Brain segment events use only their dedicated transition animation, and exits with a short erase-like animation instead of blinking in and out. Coding Brain now keeps Windows/local action tools and loop-stream rules after `/codingbrain_`; for visual coding artifacts, `/coding_live_preview_` is allowed only after a real local file has been created or modified through AGENT_ACTION_JSON and verified, the model must introduce it with a short natural paragraph, the transcript renders a dedicated "Live preview opened" event, the renderer opens a sandboxed split-canvas iframe for the file while refreshing it as work continues, and a runtime guard forces copy-paste/code-block answers back into AGENT_ACTION_JSON instead of ending with zero tool calls. Shell payloads attached to CodeAct events are kept out of the readable assistant prose and retained only as technical event detail, preventing raw PowerShell/CMD scripts from appearing as normal paragraphs. Every `executeAgentActionRequest` call now appends `started`, `result` or `blocked`, `verification` and `summary` entries to `.ingen-agent-artifacts/agent-action-runtime.jsonl`; each result carries an `AgentRuntimeAuditSummary` with entry hashes and log SHA-256. If the audit append fails, the wrapper returns a verified failure instead of reporting success. The shared `AGENT_ACTION_BENCHMARK_SUITE` now records the required local-agent benchmark contract across filesystem, code, install/update, document-toolchain, GUI, browser, document, Windows setting, Windows-sensitive, process/service, scheduler, WSL/dev, Git/PR, CI/review, cloud, blocked-danger, context compaction and final-summary cases. `runAgentActionBenchmarkSuite` executes every case through a real local runner, a verified safe block, or an explicit runtime-contract proof; it no longer emits `planned`. Git/PR/review now expects verified confirmed execution. Scheduler expects a verified Task Scheduler create/query/delete route or a clean permission/missing-backend block. Install/update, document toolchain install, browser download, Windows setting mutation, sensitive Windows mutation and CI/review mutation cases prove prompt gating instead of fake mutation. WSL/dev expects `virtualization.run_command` with command-exit proof or a clean native fallback/missing-backend result; cloud commands expect provider-scoped read-only/write-confirmed execution or a clean blocked-danger result.

## Public Interfaces

Core shared types:

- `AgentCapabilityAtlasEntry`
- `AgentCapabilityFamily`
- `AgentCapabilitySurface`
- `AgentCapabilityApproval`
- `AgentCapabilityVerification`
- `AgentActionLoopState`
- `AgentToolResult`
- `AgentVerificationProbe`
- `AgentPermissionPolicy`
- `AgentEventTimeline`

Core host functions:

- `createAgentCapabilityAtlas(config)`
- `createAgentActionHostManifest(config)`
- `agentActionHostPromptManifest(config)`
- `executeAgentActionRequest(config, request)`
- `executeAssistantAgentActionLoop(params)`

Rule: do not create a new direct action kind until the capability has a typed request, typed result, risk classification, approval policy and verification probe.

## Permission Matrix

Read without approval:

- Workspace listing/search.
- Bounded command inspection.
- WMI/CIM read queries.
- Event log read queries.
- Package list queries.

Prompt or confirmed:

- Writes outside workspace.
- File move/copy/rename/create in computer scope.
- Any recursive delete.
- Arbitrary shell command.
- Installer, update, uninstall through exact package ids or confirmed shell routes.
- Registry/service/process/network/firewall mutation; direct typed registry writes are limited to confirmed explicit `HKCU:\` value updates.
- Browser form submission, download execution, account change.
- Office COM write, macro run, Outlook send.
- WSL/Hyper-V/Docker lifecycle mutation; only named, confirmed, verified Hyper-V lifecycle routes are direct today.
- Cloud writes.
- Automations and scheduled tasks.

Blocked unless explicitly redesigned with a safe user-presence path:

- Credential extraction.
- Private key export.
- UAC bypass.
- Security prompt approval on behalf of the user.
- Payment confirmation.
- Silent weakening of Windows Security, Defender, firewall or encryption.
- Protected root access against `C:\Users\quent\Documents\EVE\MAP`.

## Verification Matrix

- File operation: old/new path, stats, hash when useful.
- Command: exit code, stdout/stderr, expected output predicate.
- Package: installed package list or version query.
- Registry: read back key/value.
- Service: service status query.
- Process: process list or wait status.
- Scheduler: task query and next run state.
- Browser: DOM/screenshot/download state.
- GUI: screenshot/accessibility tree state.
- Document/media: output path, parser readback, hash.
- Git/dev: command output, working tree diff, test result.
- MCP/cloud: connector result plus independent local artifact when possible.
- Automation: ledger entry, schedule state, cancellation path.

## Benchmarks

Required local scenarios:

- Organize a temp desktop fixture without deleting apps or protected files.
- Create and edit a text/Markdown file, then verify content.
- Search a repo, modify code, run tests, summarize diff.
- Run a read-only Windows diagnostic command.
- Attempt a protected-root action and verify it is blocked.
- Simulate a failed command and verify retry prompt behavior.
- Inspect installed packages with a read-only route.
- Inspect OS/settings state and block or confirm registry changes with readback proof.
- Drive a simple GUI fixture once computer-use is implemented.
- Download a file in a contained browser and verify artifact hash, or prove confirmation is required.
- Parse and produce CSV/PDF/Office-like fixtures.
- Query WSL availability without installing anything, and run a bounded confirmed command with exit proof or clean fallback.
- Create a scheduled task only in a temp/approved test lane.
- Verify final summary appears after every tool-using loop.

## Documentation And Maintenance Rules

- This document is the plan of record for local OS-agent capability.
- `MIGRATION_FRONT.md` remains the plan of record for frontend shell work.
- `FORGE_NATIVE_BYTECODE.md` remains the plan of record for Forge/Monster runtime work.
- Keep this document current when a planned capability becomes executable.
- Remove failed experimental paths instead of documenting around them.
- Every new capability must add tests and verification before being promoted.
