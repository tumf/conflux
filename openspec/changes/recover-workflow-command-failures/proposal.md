---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/changes/archive/2026-01-16-add-agent-crash-recovery/
  - openspec/changes/archive/add-post-apply-cleanup-review/
  - src/execution/apply.rs
  - src/history.rs
  - src/orchestration/acceptance.rs
  - src/serial_run_service.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/agent/prompt.rs
  - skills/cflx-cleanup-review/SKILL.md
verifications:
  - id: workflow-command-recovery-tests
    requirement: "Apply, Acceptance, and cleanup-review recover bounded operational command failures without losing workspace evidence, consuming unrelated retry budgets, or bypassing truthful completion gates"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering Apply iteration continuation and unlimited mode, serial and parallel Acceptance-only command retry, cleanup-review corrective retry diagnostics and dual success gate, cancellation and permission routing, retry exhaustion, and absence of durable retry checkpoints"
    rerun: "cargo test --lib execution::apply && cargo test --lib serial_run_service && cargo test --lib parallel::tests::executor && cargo test --lib agent::prompt && cargo fmt --check && cargo clippy -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Recover workflow command failures without discarding workspace progress

**Change Type**: implementation

## Premise / Context

- Apply, Acceptance, and post-apply cleanup-review all run through the retrying command queue, but exhausting that transport-level retry currently becomes a terminal workspace error before the operation-level workflow can use repository progress to recover.
- `execute_apply_loop` already records exit code and bounded stdout/stderr in `ApplyHistory`; the next Apply prompt can consume that evidence, but ordinary non-zero exits currently return before another iteration.
- Canonical configuration names the outer budget `max_iterations` and defines `0` as unlimited, while the current Apply loop tests `iteration > max_iterations` unconditionally. The older crash-recovery requirement still names the removed `max_apply_iterations` setting.
- Parallel Acceptance turns `CommandFailed` into terminal `WorkspaceResult.error`. Serial routing returns `AcceptanceCommandFailed` to an outer loop that may run Apply again. A captured production run later recovered by rerunning Acceptance against the same applied workspace without repository repair.
- Cleanup-review requires both one standalone `CLEANUP_REVIEW: CLEAN` marker and a repository-verified clean worktree. Current command retries repeat the same prompt; after they finish, a command failure, missing marker, or remaining dirty state terminates without a corrective cleanup-review attempt carrying the observed diagnosis.
- The constitution permits active-run in-memory counters and prompt context but forbids durable out-of-worktree workflow-control state. Resume must remain derivable from workspace files and Git evidence.

## Problem / Context

Three operation layers currently confuse transport retry exhaustion with unrecoverable workflow failure. The command queue correctly retries short-lived command crashes, but the caller often still has enough repository-local evidence to continue safely:

1. Apply can preserve partial implementation and provide the failed attempt to the next agent iteration.
2. Acceptance can rerun review against the unchanged applied and clean workspace without dispatching Apply.
3. Cleanup-review can inspect its previous protocol failure and dirty status, repair the handoff, and re-prove both its marker and Git cleanliness.

Immediate terminal conversion wastes valid progress and requires manual operator retry. Reusing the wrong existing loop is also unsafe: Acceptance command failures must not consume missing-verdict, explicit-CONTINUE, or FAIL-to-Apply budgets; cleanup-review must not bypass its dual success gate or fall back to ordinary Apply. Recovery must remain bounded, cancellation-aware, permission-aware, and free of durable retry checkpoints.

## Proposed Solution

Introduce operation-level recovery after the existing command queue finishes its internal attempts:

- **Apply:** record every non-zero result through the existing `ApplyHistory`, run `on_error`, and continue the same `execute_apply_loop` unless cancellation, permission-stall, blocked/rejecting handoff, or completion-finalized routing owns the result. Count all dispatched Apply attempts against `max_iterations` when it is positive; treat `0` as unlimited.
- **Acceptance:** add one shared active-run command-failure policy used by serial and parallel execution. It permits two Acceptance-only retries after the initial command failure, carries only the latest bounded command diagnostics, and resets after a canonical Acceptance outcome. It does not rerun Apply or cleanup-review and does not consume other Acceptance counters.
- **Cleanup-review:** add an operation-level loop allowing two corrective attempts after the initial cleanup-review result. Each new prompt receives the latest structured failure kind, exit code, bounded stdout/stderr, standalone-marker observation, and bounded current porcelain status. Every attempt independently rechecks command completion, exactly one standalone marker, and repository cleanliness in that order.
- **Cancellation and policy:** make cleanup-review waiting observe the per-change cancellation token and terminate its child. Explicit cancellation never starts another attempt. Classified permission denial retains existing permission/stall ownership rather than becoming generic corrective retry.
- **Constitutional restart:** keep counters and diagnostic context in memory only. Do not create workspace-root reports, retry marker files, external job IDs, or other durable workflow-control state. A restarted process recomputes the next operation from workspace and Git evidence.

No new configuration key is introduced. The two corrective retries for Acceptance command failure and cleanup-review are fixed protocol safety bounds, parallel to the existing bounded Acceptance protocol correction policy but with independent counters.

## Acceptance Criteria

1. After command-queue retries finish, an ordinary non-zero Apply result is recorded with bounded diagnostics and dispatches the next Apply iteration instead of immediately producing a terminal workspace error.
2. `max_iterations > 0` bounds all Apply attempts, including command-failure and final-commit repair attempts; exhaustion reports the latest actionable diagnostics. `max_iterations = 0` permits continued iterations until completion, cancellation, stall, or another owned terminal outcome.
3. Apply cancellation, repeated unresolved permission denial, blocked/rejecting handoff, and completion-finalized termination retain their existing routing and are not converted into ordinary command-failure retries.
4. Serial and parallel Acceptance command failures rerun only the configured Acceptance command against the same applied, clean workspace, with at most two corrective retries after the initial failure.
5. Acceptance command-failure retries carry the latest bounded exit/error/stdout/stderr evidence and do not rerun Apply or cleanup-review, append FAIL repair tasks, or consume explicit-CONTINUE, missing-verdict/protocol, or Apply-repair cycle budgets.
6. Any canonical Acceptance result resets command-failure retry state and follows its existing PASS, FAIL, CONTINUE, stalled, permission-stalled, or protocol routing.
7. Three consecutive Acceptance command failures produce one terminal error containing attempt-count and latest bounded diagnostics; no fourth command-failure attempt starts.
8. A dirty managed worktree gets at most three cleanup-review operation attempts. A later attempt receives the latest structured failure kind, exit code when available, bounded stdout/stderr, marker observation, and bounded current `git status --porcelain` evidence.
9. Cleanup-review succeeds only when the command result is acceptable, output contains exactly one standalone `CLEANUP_REVIEW: CLEAN`, and a fresh repository query proves the worktree clean. Marker-only and clean-only outcomes both remain failures.
10. Cleanup-review cancellation terminates the active child and starts no additional attempt. Classified permission denial retains existing non-terminal permission/stall semantics rather than consuming generic cleanup repair budget.
11. Cleanup-review exhaustion produces a terminal error with attempt count and latest bounded diagnosis while preserving the dirty managed workspace for explicit retry.
12. Active recovery creates no durable retry checkpoint or out-of-worktree workflow-control input. Restart routing remains a function of workspace files, Git state, and base-tree comparison.

## Explicit Completion Conditions

- `src/execution/apply.rs` applies the positive-only `max_iterations` guard and continues ordinary non-zero command results through existing history-backed iterations while preserving all owned exception routes.
- `src/orchestration/acceptance.rs` owns a shared, fixed command-failure retry decision and bounded latest diagnostic contract; `src/serial_run_service.rs` and `src/parallel/dispatch.rs` both use it without returning to Apply between attempts.
- `src/parallel/executor.rs` owns cleanup-review corrective attempts, captures bounded latest diagnostics, observes per-change cancellation, classifies permission denial consistently, and performs the marker-plus-clean validation after every attempt.
- `src/agent/prompt.rs` and `skills/cflx-cleanup-review/SKILL.md` accept trusted corrective instructions plus clearly delimited untrusted prior diagnostics without allowing prior output to redefine success.
- Fast default tests cover successful second-attempt recovery, exact exhaustion bounds, budget independence, diagnostic injection, cancellation, permission routing, `max_iterations = 0`, and no durable checkpoint artifacts. Tests over one second use the repository heavy-test policy.
- `cargo test --lib execution::apply`, `cargo test --lib serial_run_service`, `cargo test --lib parallel::tests::executor`, `cargo test --lib agent::prompt`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Scope Rationale

The three paths share one correctness boundary: transport failure is not terminal while repository-local evidence permits a bounded operation-level recovery. They must ship together so serial and parallel routing, retry-budget ownership, prompt diagnostics, cancellation, and truthful completion use one coherent contract. Splitting would leave at least one frontend or handoff stage with the old terminal conversion and would make the canonical crash-recovery requirement internally inconsistent.

## Out of Scope

- Changing command queue retry count, retry patterns, stagger, or inactivity-timeout policy.
- Adding user-configurable Acceptance command-failure or cleanup-review retry limits.
- Changing Acceptance verdict parsing, missing-verdict limits, explicit-CONTINUE policy, FAIL repair semantics, or the ten-cycle Apply/Acceptance safety ceiling.
- Weakening `CLEANUP_REVIEW: CLEAN`, accepting narrative success, or treating a dirty worktree as handoff-ready.
- Automatically retrying archive failures, invalid archive layout, startup/configuration errors, explicit cancellation, or validated external blockers.
- Persisting retry counters, prior agent output, verdict reports, or workflow-control state outside the managed workspace evidence allowed by the constitution.
