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
- The manifest now exposes planned/boundary capabilities such as WMI, scheduler, settings, credentials, browser CDP, Office COM, RPA, WSL, Hyper-V/Docker, cloud CLIs, MCP/plugins and automations.
- `shell.full` remains the confirmed universal Windows escape hatch.

Current limitation:

- Most atlas entries are not yet direct executable backends.
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
8. Stop: conclude only when verified, approval-blocked, impossible with current tools or max-step bounded.
9. Summarize: final answer states what changed, how it was verified, what failed and what remains.

The loop must never claim success from intention alone. It must never show fake tool events. Tool events are rendered by the app from real runtime events.

## Ten-Step Implementation Plan

### 1. Capability Atlas Total

Goal: give the LLM a complete map of what the local agent can do, can try, cannot yet do and must never do silently.

Deliverables:

- Extensible `AgentCapabilityAtlasEntry`.
- `createAgentCapabilityAtlas(config)`.
- Compact `capability_atlas=` prompt summary.
- Explicit distinction between executable actions and non-executable atlas knowledge.
- Tests for Office COM, browser CDP, WMI, scheduler, settings, credentials, WSL and RPA.

Acceptance:

- At least 15 capability families are present.
- Sensitive families use `approval: prompt` or `approval: blocked`.
- Existing executable actions still work.
- Planned entries do not become direct `AGENT_ACTION_JSON` actions.

Status: implemented in commit `58d53fcbf`.

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

Status: implemented for the planned scope. The manifest carries runtime hashes, atlas hashes, delta policy, installed/missing tool detection, prompt token estimates, selected-capability detail on tool-result continuation and agent-action compaction state.

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

Status: implemented for the planned scope. The normal path now carries `AgentActionLoopState`, explicit terminal outcomes, result observations, retry/approval accounting, proof hashes and mandatory English final summaries after tool-using tasks. Deterministic desktop organization is quarantined behind `INGEN_AGENT_ACTION_COMPAT_FALLBACK=1` as compatibility behavior, while normal mutation follow-up stays inside the universal loop and blocks with a final status when the model stops without the required next action.

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

Status: implemented for the planned scope. The host now exposes `AgentComputerUsePolicy`, executable GUI actions for bounded inspection, confirmed appshot capture, confirmed window focus and confirmed clipboard read/write. GUI actions use foreground user-presence mode, single-action-then-verify pacing, explicit forbidden prompt categories, and structured verification/artifact results. Full OCR, full UI Automation trees and low-level mouse/keyboard/scroll/drag-drop remain planned backends rather than fake direct execution.

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

Status: implemented for the planned scope. The host now exposes `AgentBrowserWebPolicy`, executable web actions for bounded URL inspection, confirmed URL downloads with persisted artifact size and SHA-256, and confirmed external browser navigation. Page summaries include status, content type, title, link/form/download-candidate counts and planned screenshot/DOM/network-log fields. Deep CDP sessions, contained WebExplorer DOM control, Playwright network logs and form submission automation remain planned backends rather than fake direct execution.

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
- Installer, update, uninstall.
- Registry/service/process/network/firewall mutation.
- Browser form submission, download execution, account change.
- Office COM write, macro run, Outlook send.
- WSL/Hyper-V/Docker lifecycle mutation.
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
- Open a settings URI in planned/approval mode.
- Drive a simple GUI fixture once computer-use is implemented.
- Download a file in a contained browser and verify artifact hash.
- Parse and produce CSV/PDF/Office-like fixtures.
- Query WSL availability without installing anything.
- Create a scheduled task only in a temp/approved test lane.
- Verify final summary appears after every tool-using loop.

## Documentation And Maintenance Rules

- This document is the plan of record for local OS-agent capability.
- `MIGRATION_FRONT.md` remains the plan of record for frontend shell work.
- `FORGE_NATIVE_BYTECODE.md` remains the plan of record for Forge/Monster runtime work.
- Keep this document current when a planned capability becomes executable.
- Remove failed experimental paths instead of documenting around them.
- Every new capability must add tests and verification before being promoted.
