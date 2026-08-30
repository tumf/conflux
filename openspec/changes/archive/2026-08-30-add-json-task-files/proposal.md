---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/task_parser.rs
  - src/openspec_cmd/validation.rs
  - src/execution/apply.rs
  - src/execution/archive.rs
  - src/archive_layout.rs
  - skills/cflx-apply/SKILL.md
verifications:
  - id: json-task-file-tests
    requirement: Markdown and JSON task files provide one fail-closed contract across parser, validation, prompts, archive, merge, TUI and Web paths
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/task_parser.rs
    evidence: cargo test --lib
    rerun: cargo test --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Support `tasks.json` as a task-file alternative

**Change Type**: implementation

## Problem / Context

Conflux hard-codes `tasks.md` across task progress, proposal validation, Apply and Acceptance mutation, archive and merge authorization, OpenSpec list/show, prompts, and archived-path evidence. A change cannot use a structured task artifact even when deterministic machine updates are preferable.

Independent task readers make a filename-only fallback unsafe: different workflow phases could select different sources, and the current acceptance follow-up writer is Markdown-specific. The permanent change therefore needs one shared task-file identity, parser, validator, and mutation boundary.

## Proposed Solution

Add a shared task-file abstraction supporting exactly one authoritative file per active or archived change entry:

- existing `tasks.md`, unchanged;
- new `tasks.json`, using the versioned schema in `design.md`.

Preserve each existing resolver mode: comprehensive progress, active-only, archived, and workspace-local Acceptance mutation. Within a selected change entry, choose the sole supported filename. If both filenames exist, fail closed before progress, mutation, acceptance, archive, or merge decisions.

Route progress reads, strict validation, OpenSpec list/show, Apply format checks, Acceptance follow-up read/write/cleanup, rejection recovery, archive completion, final-merge authorization, prompts, and archive path recognition through the shared abstraction. JSON mutations must be atomic and preserve unowned extension fields.

## Acceptance Criteria

- A change containing only valid `tasks.json` can complete the same proposal, Apply, Acceptance, archive, resolve, TUI, Web, list/show, and resume paths as an equivalent `tasks.md` change.
- Existing `tasks.md` behavior and checkbox semantics remain backward compatible.
- A change entry containing both `tasks.md` and `tasks.json` is rejected as ambiguous; neither file silently wins.
- Missing, unreadable, malformed, unsupported-version, duplicate-ID, invalid-status, or semantically invalid JSON fails closed and never yields false `0/0` or complete progress.
- JSON task completion is counted from `status`; only `completed` is complete. An empty task list is not archive- or merge-complete.
- Internal Acceptance findings remain virtual task-gate items: unclaimed findings block archive and merge exactly as unchecked Markdown follow-up boxes do. Narrative and external-blocker data remain outside ordinary task counts.
- Runtime-owned acceptance findings round-trip structurally in JSON. Apply may record remediation and evidence but cannot replace runtime-owned finding identity or actionable payload.
- Active and archived path discovery accepts either filename while preserving current location precedence and archive-entry identity checks.
- Prompts and embedded skills name the selected task path and describe format-specific safe updates rather than instructing every agent to edit `tasks.md`.

## Explicit Completion Conditions

- Production task-file consumers no longer construct a `tasks.md` path when they require task state; they use the shared resolver/format API.
- Focused tests cover Markdown-only, JSON-only, both-present ambiguity, malformed/unsupported JSON, duplicate IDs, progress parity, active/archive fallback, atomic JSON updates, acceptance follow-up round trips, archive authorization, and OpenSpec validation/list/show.
- `cargo test --lib` passes and executes parser, strict validation, prompts, archive layout, Apply/Acceptance, merge authorization, TUI and Web regressions.
- Archive-gate validation passes for this change.

## Out of Scope

- Converting existing `tasks.md` files to JSON.
- Supporting YAML or configurable arbitrary task filenames.
- Allowing one change to split tasks across Markdown and JSON.
- Changing proposal/spec formats or OpenSpec upstream behavior.
- Adding a separate external task database or out-of-worktree workflow state.

Repository-wide formatting and Clippy remain owned by the tracked path-scoped pre-commit hooks for Rust changes. Requirement-specific tests remain explicit above.
