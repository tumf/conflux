---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-error-handling/spec.md
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
verifications:
  - id: queued-proposal-diagnostic
    requirement: Invalid queued proposal metadata is a visible change-local error before analyzer or Apply and remains retryable after repair
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/queue_state.rs
    evidence: cargo test --locked --offline --lib --features web-monitoring queued_proposal_diagnostic -- --nocapture
    rerun: cargo test --locked --offline --lib --features web-monitoring queued_proposal_diagnostic -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Expose invalid queued proposal metadata as a change-local error

**Change Type**: implementation

## Problem and evidence

At f618a3e7, `classify_queued_work_with_snapshot` in `src/parallel/queue_state.rs` catches `parse_proposal_dependencies_strict_from_file` errors and puts the ID only in its local `dependency_blocked` vector. `before_dependency_analysis` then returns early for blocked-only work, never reaching the later dispatch guard that publishes a blocker. The real-owner CRLF wrong-type fixture remains `queued` with `blocker: null` and performs no work. The later guard incorrectly calls this a dependency wait on the change itself and emits process-wide `Error`, despite this being repairable local proposal data.

## Selected solution and observable outcome

Before the blocked-only short circuit, reconcile a strict proposal-read failure for an otherwise ordinary, admitted candidate through the existing change-local `ProcessingError` path. The source is the base proposal being considered for admission, not an Acceptance reviewer. Publish `error` plus actionable `error_detail` consistently to the reducer and its API/TUI projections. Preserve existing error-intent revocation and explicit Retry semantics. Do not classify malformed data as an external prerequisite or self-dependency, and do not emit global `Error`.

Diagnostics name the change and repository-relative `openspec/changes/<id>/proposal.md`, identify unreadable/invalid proposal metadata and tell the operator to correct/commit that proposal and explicitly Retry. They must not copy raw YAML/scalars/command/environment values; a fixed bounded category (`unreadable proposal` or `invalid proposal metadata`) and safe location suffice. The new scheduler diagnostic and its scheduler warning use this same bounded text, not `%error` or the raw parser diagnostic. This does not claim redaction of every pre-existing parser log. A candidate that never started has no Acceptance provenance or reviewer transcript. Use the same local error contract if the existing later dispatch check observes a file race after the early check.

Current-state reconciliation is independent of log/event deduplication. Retain a process-local per-target error epoch/fingerprint to prevent duplicate ProcessingError emission while an earlier event awaits reducer adoption; clear that epoch on explicit Retry and removal. Reconcile authoritative state before suppressing the duplicate; the epoch is never admission authority. Repeated unchanged observations must not create event floods or re-admission. An invalid row stays errored after a file edit alone; a fresh authorized Retry re-reads the proposal and proceeds only if now valid. Another still-invalid Retry returns to the same error without launching an analyzer for that target, Apply, a gate or reviewer. Valid unrelated work continues.

## Boundary and preserved contracts

- Limit product edits to `src/parallel/queue_state.rs` and directly necessary scheduler/reducer consumers; add focused tests in existing parallel, reducer, API/client and TUI test modules. No new CLI flag, API command, lifecycle status, configuration option or persistent state.
- Reuse `ProcessingError`, existing `error_detail`, existing Retry and existing Changes/Error Details rendering. Preserve genuine dependency waits (including automatic dependency recovery), active/terminal/held candidate exclusions, accepted stop semantics, candidate refresh, native Acceptance declaration validation and cache identities.
- Do not broaden this into a full strict proposal validator at admission; the failure set is the existing strict proposal/dependency reader's read/decode errors. Valid empty/no-frontmatter inputs retain their current dependency-reader behavior.
- No changes to renderer geometry, colors, keys or visual design. Visual authority is the existing f618a3e7 `src/tui` error row/details implementation and `openspec/specs/tui-error-handling/spec.md`. API and Web consume existing error projections rather than parsing diagnostic text.
- No deployment, installation, live owner replacement, user candidate repair, push/release, shell parsing or workspace pollution.

## Acceptance and explicit completion

1. Through the real scheduler pre-analysis path, wrong-typed metadata, unreadable/missing proposal and opened/unclosed frontmatter settle to a change-local error before blocked-only early exit. Cover LF and CRLF. Assert zero analyzer-target/Apply/gate/reviewer invocations, readable relative location/remedy, no self-dependency, no process error and unchanged unrelated marks/status.
2. Mixed queue control: a valid unrelated change remains eligible; an invalid-only queue also publishes the error. Unchanged observations produce one diagnostic transition, not a spin/flood; a rebuilt projection is reconciled from repository evidence, not a dedup cache.
3. Fix-only does not silently restart an errored row. Explicit Retry after valid repair dispatches once; retry of still-invalid data returns to error with no execution. Genuine dependency blockers still auto-clear, and valid absent/empty-frontmatter reader controls are not newly rejected.
4. API status and `client wait` projection see `error_detail` and release as existing `change_requires_action` rather than waiting forever on `queued`; no fake reviewer provenance. TUI deterministic TestBackend renders the existing error row and details/remedy at normal 120x30 and narrow 80x24 sizes with standard scrolling, and leaves unrelated rows and keys unchanged. These are projection tests, not a claim of deployed Web acceptance.
5. Place all change-blocking tests, including client-wait and TUI projection controls, in `src/` module tests selected by `--lib`. Every new focused test name includes `queued_proposal_diagnostic`; the declared command must select nonzero tests, and assertions must fail on the old local-vector-only behavior. Tests use deterministic events/state, not small sleep thresholds. The existing dependency/retry tests remain green. Rust fmt/clippy and integration-wide tests are checked separately; path-filtered pre-commit does not replace this gate.

## Rejected alternatives

A log-only warning, displaying a fake dependency on self, converting parse errors to empty declarations, treating local data as an external prerequisite, or requiring a new user setting leaves the behavior misleading or unsafe. A broad new blocker taxonomy is unnecessary because a repairable local error and Retry already exist.

## Verification ownership

Bounded repository-local scheduler/reducer/API/render tests above block this change. Parent-owned disposable real-owner/TUI/API and later Web observation are independent operational checks, not archive prerequisites. The real user scope-drill worktree is never a test fixture.
