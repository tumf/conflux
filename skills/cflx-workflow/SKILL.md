---
name: cflx-workflow
description: Legacy compatibility router for Conflux apply, rejecting, cleanup-review, accept, and archive operations. New orchestrator prompts use dedicated cflx-* operation skills. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Workflow Compatibility Router

Use this skill only for legacy prompts that load `cflx-workflow`. New orchestrator prompts load `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, or `cflx-archive` directly; those dedicated skills are the authoritative guidance for new runs.

This router is self-contained so legacy prompts do not need another skill or reference file.

## Shared Rules

- Never ask questions or wait for user input.
- If `openspec/CONSTITUTION.md` exists, read it first and treat it as higher priority than proposal or spec deltas.
- Base completion and routing only on repository-verifiable evidence.
- Choose exactly one operation from the prompt context and follow only that section.

## Apply

Implement the approved change; do not only inspect, summarize, or plan.

1. Read `proposal.md`, optional `design.md`, and `tasks.md` under `openspec/changes/<change-id>/`.
2. Implement each active unchecked task and run its planned verification.
3. Mark a task `[x]` only after its implementation artifact, runtime wiring when applicable, and verification evidence exist.
4. Update `tasks.md` after each completed task. Internal agent todos are not OpenSpec completion evidence.
5. Finish only when all active tasks are complete or validly moved to a non-checkbox Future Work section.

Unit-test claims require isolated tests using mocks, fakes, or in-memory doubles. Tests using real filesystem, process, VCS, network, database, clock, credentials, or OS state are integration or e2e evidence and cannot satisfy a unit-test task.

Recoverable infrastructure failures such as Docker, DNS, registry, credential, port, rate-limit, or pending managed-job failures are non-terminal stalled holds. Record concrete evidence and recovery actions; do not create `REJECTED.md` for these cases.

For a terminally invalid change intent, record a non-checkbox `## Implementation Blocker #<n>` in `tasks.md`, create `REJECTED.md` as a review proposal, and end with an `IMPLEMENTATION_BLOCKER:` payload containing category, evidence location, and required action.

## Rejecting Review

Review apply-generated `REJECTED.md` and the matching `Implementation Blocker` evidence. Return exactly one standalone final marker:

- `REJECTION_REVIEW: CONFIRM` when evidence proves the change intent is invalid, obsolete, contradictory, or constitution-violating.
- `REJECTION_REVIEW: RESUME` when repository-only work can resolve the issue or evidence is insufficient.
- `REJECTION_REVIEW: BLOCK` when the intent remains valid but a real external or infrastructure prerequisite creates a non-terminal stalled hold.

Do not emit acceptance markers.

## Cleanup Review

Clean only apply-generated dirty state before acceptance.

1. Inspect every dirty file with `git status --porcelain`.
2. Keep and stage only intentional handoff files.
3. Never use `git add -A` or `git add .`.
4. Verify the worktree is clean.
5. On success, emit exactly one standalone final line: `CLEANUP_REVIEW: CLEAN`.

Do not perform new implementation or emit acceptance/rejection markers.

## Acceptance

Acceptance is read-only review. Do not implement fixes or edit `tasks.md`.

Review the proposal, tasks, spec deltas, implementation diff, planned verification ownership, tests, working-tree cleanliness, and actual commit-path blockers. Every FAIL finding must cite actionable repository evidence such as a file, symbol, or line.

Use these outcomes:

- `pass`: all requirements and active task claims have repository evidence.
- `fail`: repository-only work can resolve the finding.
- `continue`: review is incomplete and another acceptance attempt is required.
- `gated`: compatibility token for a valid non-terminal stalled hold that repository-only apply work cannot resolve. It is only a stalled hold when accompanied by a structured `blocker` payload; a bare token is a protocol error.

The canonical verdict is strict JSON on its own line:

- `{"acceptance":"pass"}`
- `{"acceptance":"fail","findings":["<evidence>"]}`
- `{"acceptance":"continue"}`
- `{"acceptance":"gated","blocker":{"category":"<supported>","evidence":["<concrete>"],"next_action":"<unblock>","resumable":true}}`

Supported blocker categories: `credential`, `external_approval`, `policy`,
`external_service`, `pending_verification`, `infrastructure`,
`schema_incompatibility`, `human_decision`. Choose one from observed evidence —
the runtime never infers a category from prose. A bare `{"acceptance":"gated"}`
or plain `ACCEPTANCE: GATED`, an unsupported category, empty evidence, or a
missing `next_action`/`resumable` is an acceptance protocol error: the runtime
retries acceptance within a fixed budget and then reports a terminal protocol
error. Emit `FAIL` or `CONTINUE` instead when you cannot supply all four fields.

During compatibility rollout, emit the matching legacy marker as the next and final line:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: GATED`

`ACCEPTANCE: BLOCKED` is accepted only as legacy input compatibility. Do not wrap verdicts in headings, blockquotes, bullets, emphasis, or code fences, and do not append text to verdict lines.

A valid `Implementation Blocker` produces a stalled hold only when repository-only work cannot resolve it and the structured blocker payload is supplied. Recoverable infrastructure failures remain non-terminal stalled holds. Never create `APPLY_BLOCKED` or any other runtime marker under the change directory; the runtime records stalled holds outside the worktree. Missing or ambiguous verification planning, false unit-test claims, dirty worktrees, and missing repository implementation are FAIL findings.

## Archive

Archive only after acceptance passes and all active tasks are complete.

1. Verify the change exists and is archive-ready.
2. Run `cflx openspec archive <change-id> --yes`; use `--skip-specs` only for tooling-only changes.
3. Never create or move archive entries directly with `mkdir`, `mv`, `git mv`, or scripts. A failed CLI archive command is terminal for this operation.
4. Run strict/archive-gate validation as applicable.
5. Review `git diff openspec/specs/` and confirm each canonical spec change matches the delta.
6. Confirm the active change is no longer left unarchived.

Do not perform feature implementation during archive.
