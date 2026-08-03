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
  - src/agent/runner.rs
  - src/agent/prompt.rs
  - src/ai_command_runner.rs
  - src/permission.rs
  - src/orchestration/acceptance.rs
  - src/orchestrator.rs
  - src/tui/orchestrator.rs
  - src/serial_run_service.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - skills/cflx-cleanup-review/SKILL.md
verifications:
  - id: workflow-command-recovery-tests
    requirement: "Apply, Acceptance, and cleanup-review recover bounded operational command failures without losing workspace evidence, consuming unrelated retry budgets, or bypassing truthful completion gates"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering per-change cross-cycle Apply budget ownership, warning and iteration_limit finish propagation, non-zero progress/stall routing and unlimited mode, shared Acceptance command policy and retry prompts, serial and parallel Acceptance-only control flow, cleanup-review corrective diagnostics and dual success gate, cancellation and immediate permission hold, retry exhaustion, preserved legacy cleanup handoffs, and absence of durable retry checkpoints"
    rerun: "cargo test --lib execution::apply && cargo test --lib orchestration::acceptance && cargo test --lib orchestrator && cargo test --lib tui::orchestrator && cargo test --lib serial_run_service && cargo test --lib parallel::executor::tests && cargo test --lib parallel::tests::executor && cargo test --lib agent::prompt && cargo fmt --check && cargo clippy -- -D warnings"
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

- **Apply budget ownership:** make one active-run per-change counter the sole `max_iterations` owner across every Apply dispatch for that change, including later FAIL-to-Apply cycles. Serial CLI, TUI, and parallel execution pass the same counter into each `execute_apply_loop` entry; command-queue attempts do not increment it; process restart resets it. The owner emits the 80% warning once per threshold crossing and returns typed `iteration_limit` exhaustion so the run boundary invokes `on_finish(status = iteration_limit)` once.
- **Apply command recovery:** record every non-zero result through the existing `ApplyHistory`, run `on_error`, then perform fresh task/Git inspection, completion and blocked/rejecting routing, permission classification, and existing progress/WIP/stall accounting before deciding whether to dispatch another Apply attempt. This keeps no-progress failures subject to stall policy even when `max_iterations = 0`.
- **Acceptance:** add one shared active-run command-failure policy used by serial and parallel execution. It permits two Acceptance-only retries after the initial command failure, carries only the latest bounded command diagnostics through `AgentRunner`/prompt plumbing, and resets whenever an invocation completes as any non-command-failure result before that result follows existing canonical or protocol routing. It does not rerun Apply or cleanup-review and does not consume other Acceptance counters.
- **Cleanup-review:** add an operation-level loop allowing two corrective attempts after the initial cleanup-review result. Each new prompt receives the latest structured failure kind, exit code, bounded stdout/stderr, standalone-marker observation, and bounded current porcelain status. Every attempt independently rechecks command completion, exactly one standalone marker, and repository cleanliness in that order.
- **Cancellation and policy:** make cleanup-review waiting observe the per-change cancellation token and terminate its child. Explicit cancellation never starts another attempt. Classified cleanup permission denial returns immediately to the existing non-terminal permission hold, starts no corrective attempt, and does not consume the generic cleanup failure counter.
- **Constitutional restart:** keep counters and diagnostic context in memory only. Do not create workspace-root reports, retry marker files, external job IDs, or other durable workflow-control state. A restarted process recomputes the next operation from workspace and Git evidence.

No new configuration key is introduced. The two corrective retries for Acceptance command failure and cleanup-review are fixed protocol safety bounds, parallel to the existing bounded Acceptance protocol correction policy but with independent counters.

## Acceptance Criteria

1. After command-queue retries finish, an ordinary non-zero Apply result is recorded with bounded diagnostics and then passes through fresh repository/handoff/progress/stall evaluation; it dispatches another Apply attempt only when that evaluation permits continuation.
2. One per-change active-run counter owns `max_iterations` across serial CLI, TUI, and parallel Apply entries, including later Acceptance FAIL repair, command-failure repair, escalation, task-format repair, and final-commit repair. Command-queue attempts do not increment it, and restart resets it.
3. `max_iterations > 0` starts no Apply dispatch beyond its exact ceiling, emits the 80% warning from the sole owner, and propagates typed `iteration_limit` so `on_finish` receives that status once with the exact count. `max_iterations = 0` disables only the numeric ceiling; no-progress command failures still reach existing stall policy.
4. Apply cancellation, repeated unresolved permission denial, blocked/rejecting handoff, completion-finalized termination, pre/post hooks, and progress accounting retain their defined ordering and are not converted into ordinary command-failure retries.
5. Serial and parallel Acceptance command failures rerun only the configured Acceptance command against the same applied, clean workspace, with at most two corrective retries after the initial failure.
6. Acceptance command-failure retries carry the latest bounded exit/error/stdout/stderr evidence through the normal runner/prompt boundary and do not rerun Apply or cleanup-review, append FAIL repair tasks, or consume explicit-CONTINUE, missing-verdict/protocol, or Apply-repair cycle budgets.
7. Any Acceptance invocation that completes as a non-command-failure result resets consecutive command-failure state before following its existing canonical, missing/malformed protocol, stalled, permission-stalled, or blocker routing.
8. Three command failures uninterrupted by a completed non-command-failure invocation produce one terminal error containing attempt-count and latest bounded diagnostics; no fourth command-failure attempt starts.
9. A dirty managed worktree gets at most three cleanup-review operation attempts. A later attempt receives the latest structured failure kind, exit code when available, bounded stdout/stderr, marker observation, and bounded current `git status --porcelain` evidence.
10. Cleanup-review succeeds only when the command result is acceptable, output contains exactly one standalone `CLEANUP_REVIEW: CLEAN`, and a fresh repository query proves the worktree clean. Marker-only and clean-only outcomes both remain failures.
11. Cleanup-review cancellation terminates the active child and starts no additional attempt. Classified cleanup permission denial immediately enters the existing non-terminal permission hold, starts no corrective attempt, and does not consume the generic cleanup failure counter.
12. Cleanup-review exhaustion produces a terminal error with attempt count and latest bounded diagnosis while preserving the dirty managed workspace for explicit retry; existing clean-skip and Apply completion/blocked-handoff grace semantics remain unchanged.
13. Active recovery creates no durable retry checkpoint or out-of-worktree workflow-control input. Restart routing remains a function of workspace files, Git state, and base-tree comparison.

## Explicit Completion Conditions

- One per-change active-run Apply budget owner is shared across `src/orchestrator.rs`, `src/tui/orchestrator.rs`, `src/serial_run_service.rs`, `src/parallel/dispatch.rs`, and `src/execution/apply.rs`; obsolete independent run-loop counting and unused initial-iteration plumbing are removed or delegated to that owner.
- Apply command failure handling records history, preserves hook ordering, and reaches the same fresh task/Git, handoff, progress/WIP, permission, and stall state machine as a completed Apply attempt before redispatch.
- Positive budget exhaustion produces one typed `iteration_limit` outcome with latest actionable failure; CLI, TUI, and parallel boundaries stop that change/run consistently, emit the warning at the configured threshold, and invoke the existing `on_finish` contract exactly once with the cumulative per-change count.
- `src/orchestration/acceptance.rs` owns a shared, fixed command-failure retry decision; `src/agent/runner.rs`, `src/agent/prompt.rs`, `src/serial_run_service.rs`, `src/parallel/dispatch.rs`, and `src/parallel/executor.rs` carry latest-only diagnostics and reset on every completed non-command-failure invocation without returning to Apply between command attempts.
- `src/parallel/executor.rs` owns cleanup-review corrective attempts, captures bounded latest diagnostics, observes per-change cancellation, immediately routes classified permission denial to non-terminal hold without retry/counter consumption, and performs marker-plus-clean validation after every ordinary attempt.
- `src/agent/prompt.rs` and `skills/cflx-cleanup-review/SKILL.md` accept trusted corrective instructions plus clearly delimited untrusted prior diagnostics without allowing prior output to redefine success.
- Fast default tests cover cross-cycle budget ownership, warning/finish propagation, no-progress stall with zero limit, all owned Apply outcomes and hooks, Acceptance reset interleavings and retry prompts, cleanup legacy scenarios, successful second-attempt recovery, exact exhaustion bounds, budget independence, diagnostic injection, cancellation, immediate permission hold, and no durable checkpoint artifacts. Process-boundary tests over one second use `heavy-tests` and retain fast state-machine coverage.
- `cargo test --lib execution::apply`, `cargo test --lib orchestration::acceptance`, `cargo test --lib orchestrator`, `cargo test --lib tui::orchestrator`, `cargo test --lib serial_run_service`, `cargo test --lib parallel::executor::tests`, `cargo test --lib parallel::tests::executor`, `cargo test --lib agent::prompt`, `cargo fmt --check`, and `cargo clippy -- -D warnings` pass.

## Scope Rationale

The three paths share one correctness boundary: transport failure is not terminal while repository-local evidence permits a bounded operation-level recovery. They must ship together so serial and parallel routing, retry-budget ownership, prompt diagnostics, cancellation, and truthful completion use one coherent contract. Splitting would leave at least one frontend or handoff stage with the old terminal conversion and would make the canonical crash-recovery requirement internally inconsistent.

## Out of Scope

- Changing command queue retry count, retry patterns, stagger, or inactivity-timeout policy.
- Adding user-configurable Acceptance command-failure or cleanup-review retry limits.
- Changing Acceptance verdict parsing, missing-verdict limits, explicit-CONTINUE policy, FAIL repair semantics, or the ten-cycle Apply/Acceptance safety ceiling.
- Weakening `CLEANUP_REVIEW: CLEAN`, accepting narrative success, or treating a dirty worktree as handoff-ready.
- Automatically retrying archive failures, invalid archive layout, startup/configuration errors, explicit cancellation, or validated external blockers.
- Persisting retry counters, prior agent output, verdict reports, or workflow-control state outside the managed workspace evidence allowed by the constitution.
