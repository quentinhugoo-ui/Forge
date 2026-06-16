# CodeAct Loop-Stream Methodology

The application does not reason. The LLM owns every decision. A CodeAct is only
an explicit boundary chosen by the LLM, validated by the host, executed by a
narrow runtime contract, then returned to the LLM as compact evidence.

Most loop-stream CodeActs must use two round trips:

```text
1. LLM -> /codeact_
2. App -> CODEACT_TEMPLATE_RESULT
3. LLM -> /codeact_ template_proof_hash="sha256:..." filled_slots...
4. App -> CODEACT_RESULT
```

## Why

The loop stream breaks when the app silently guesses missing fields, executes a
half-command, or swallows an action result. The template handoff keeps the LLM in
charge while giving the app a typed contract it can safely validate.

## Required Runtime Rules

- A bare `/codeact_` returns only `CODEACT_TEMPLATE_RESULT`.
- A filled `/codeact_` without the matching `template_proof_hash` returns
  `CODEACT_TEMPLATE_RESULT reason=template_required`.
- A filled `/codeact_` with the matching `template_proof_hash` may execute.
- Every executable CodeAct must return a result to the LLM.
- The result must include status, bounded evidence, provenance and a proof hash.
- The app must not choose a follow-up CodeAct. It may suggest next action refs,
  but the LLM emits the next CodeAct.
- The app must not infer intent beyond schema validation, routing an explicit
  command, collecting observations, executing the contract, and returning proof.

## Template Result Shape

```text
CODEACT_TEMPLATE_RESULT
schema=forge.<domain>.<codeact>.template_result.v1
command=/codeact_
status=template
reason=empty_command|template_required
template_proof_hash=sha256:<stable-template-hash>
allowed_values={...}
template:
  /codeact_
  template_proof_hash="sha256:<stable-template-hash>"
  required_slot=""
  optional_slot="allowed|values"
proof_hash=sha256:<result-proof-hash>
```

## Execution Result Shape

```text
CODEACT_RESULT
schema=forge.<domain>.<codeact>.result.v1
command=/codeact_
status=ok|partial|error
request_hash=sha256:<filled-request-hash>
evidence={...bounded...}
provenance={...}
proof_hash=sha256:<result-proof-hash>
```

## One-Way Exceptions

Exceptions must stay rare and explicit:

- `/rename_session_` is pre-loop, one-way and silent. It labels the session
  before real work begins and must never block the loop stream.
- Brain switch CodeActs may be single-step only if they return a validation
  result and the active Brain's CodeAct catalog.

## UI Expectations

The conversation should show CodeAct progress without pretending the app is
thinking:

- show an event row when a CodeAct boundary is detected,
- reuse the same animated module icon language as the sidebar,
- show template receipt as a contract handoff,
- show execution as a result card with evidence snippets,
- never hide a missing result behind assistant prose.

For `/searcharchive_`, the visible card must show the query, status, searched
scope, proof hashes and snippets from matching archived sessions.
