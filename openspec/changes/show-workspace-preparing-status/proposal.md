---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - src/events.rs
  - src/orchestration/state.rs
  - src/parallel/
  - src/vcs/git/mod.rs
  - src/tui/
  - src/web/
verifications:
  - id: local-tests
    requirement: Workspace preparation is projected truthfully across the reducer, TUI, WebUI, and API without changing resume routing
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering preparation transitions, setup failure, active-state guards, and frontend projections
    rerun: cargo test --all-features preparing
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show Workspace Preparing Status

**Change Type**: implementation

## Problem / Context

After the scheduler dispatches a change, worktree creation and `.wt/setup` can take several minutes before the Apply agent starts. During that interval the shared reducer leaves the change displayed as `queued`, so TUI and monitoring clients look stalled even though repository preparation is actively progressing.

`applying` is also inaccurate during this interval because no Apply agent has started. The product needs a distinct active preparation phase that is observable but remains ephemeral and does not influence restart routing.

## Proposed Solution

Add a process-local `preparing` activity for a change that has been admitted to an execution slot and is creating, recreating, setting up, or inspecting its managed workspace before the next workflow operation starts.

Emit the preparation transition before potentially slow workspace work. Project it through the shared reducer to TUI, WebUI, and `/api/v2`, and treat it as active execution for cancellation and destructive-worktree guards. Transition from `preparing` to the repository-derived next operation, normally `applying`, and report preparation failures as errors with actionable setup diagnostics.

Add bounded observability around `.wt/setup`: a visible start diagnostic and a completion diagnostic containing elapsed time. These diagnostics are observability only and MUST NOT become workflow-control evidence.

This remains one proposal because the event contract, reducer state, active-operation guards, and frontend projections must ship together to avoid a state token that is emitted but not represented consistently.

## Acceptance Criteria

- A scheduler-dispatched change displays `preparing` before worktree creation or recreation begins and remains `preparing` while `.wt/setup` runs.
- `preparing` is replaced by `applying[:iteration]`, accepting, rejecting, archiving, or resolving when repository evidence routes execution to that phase.
- A preparation or `.wt/setup` failure produces an error state and a diagnostic that identifies the failed preparation step.
- TUI, WebUI, and `/api/v2` expose the same `preparing` status from the shared reducer.
- `preparing` counts as active execution for stop/dequeue classification and worktree mutation guards; while no preparation termination handle exists, dequeue is refused, the stop mark is retained, and execution stops after preparation before an operation agent starts.
- Logs expose `.wt/setup` start and completion with elapsed duration without using those logs as routing input.
- Restart and resume behavior remains derived only from workspace files, Git state, and base-tree comparison; `preparing` is not persisted as authoritative workflow state.

## Explicit Completion Conditions

- The execution event and reducer paths contain an explicit preparation transition emitted before slow workspace setup.
- Every exhaustive status/activity mapping, active-status classifier, frontend projection, and generated API contract affected by the new token handles `preparing`.
- Repository-local tests hold setup execution open and prove the externally projected status is `preparing`, then release it and prove the next real phase is shown.
- Repository-local tests cover setup failure, non-Apply resume routing, active-operation guards, and the absence of durable preparation-based routing.
- `cargo test --all-features preparing`, formatting, linting, and relevant full test suites pass.

## Out of Scope

- Optimizing or parallelizing commands inside project-owned `.wt/setup` scripts.
- Persisting setup progress, command-level percentages, or estimated completion times.
- Treating `preparing` as proof that Apply started or as repository evidence for resume decisions.
- Changing the configured `.wt/setup` command contract.
- Making the current parallel dispatch loop non-blocking while workspace setup runs.
- Adding this preparation status to obsolete serial execution paths.
