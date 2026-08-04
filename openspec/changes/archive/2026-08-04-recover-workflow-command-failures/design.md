## Context

Conflux has two retry layers with different responsibilities. `AiCommandRunner::execute_streaming_with_retry` and `CommandQueue` retry transport-level crashes of one configured command. Above them, Apply, Acceptance, and cleanup-review own workflow semantics and repository evidence. Today, exhaustion of the lower layer is often immediately promoted to terminal `WorkspaceResult.error`, even when the operation layer can safely retry using the same managed worktree.

Apply already has the required continuation mechanism: `ApplyHistory` records exit status and bounded stream tails, and the next prompt includes it. Acceptance already has protocol-specific active-run counters, but command failure is deliberately distinct from missing verdict and canonical CONTINUE. Cleanup-review already has a strict success marker and Git cleanliness check, but lacks an operation-level corrective loop and structured latest failure context.

The constitution requires restart routing to be derived from workspace files, Git state, and base-tree comparison. Retry counters and prompt diagnostics may exist only in active-run memory; logs and external state cannot authorize the next operation.

## Goals

- Recover ordinary Apply command failures through the existing history-backed outer iteration.
- Make `max_iterations = 0` truly unlimited and use positive values as one total Apply-attempt ceiling.
- Give serial and parallel execution identical bounded Acceptance-only command recovery.
- Give cleanup-review bounded corrective attempts without weakening marker or clean-worktree proof.
- Keep cancellation, permission denial, validated blockers, and owned protocol outcomes out of generic retry.
- Keep all retry-control state process-local and restart routing repository-derived.

## Non-Goals

- Change command queue retry behavior or configuration.
- Add configurable operation-retry settings.
- Change Acceptance verdict parsing, FAIL repair, CONTINUE, missing-verdict, or blocker semantics.
- Retry Archive or repository invariant failures under this policy.
- Make cleanup-review success depend on agent narrative rather than marker plus Git state.

## Retry Layer Contract

| Layer | Purpose | Budget owner | Context passed forward |
|---|---|---|---|
| Command queue | Retry one command process after crash/inactivity according to existing queue policy | `CommandQueueConfig` | Same configured command/prompt; streamed diagnostics remain observable |
| Apply iteration | Continue implementation using repository progress | One per-change active-run counter shared across every Apply entry; positive `max_iterations` total or zero unlimited | Existing `ApplyHistory` with exit code and bounded stdout/stderr |
| Acceptance command recovery | Re-run review when the configured command cannot complete | Fixed initial plus two retries | Latest bounded command failure only |
| Acceptance protocol correction | Correct missing/malformed canonical output | Existing protocol counter | Existing bounded protocol context |
| Acceptance FAIL repair | Return actionable findings to Apply | Existing Apply/Acceptance cycle ceiling and finding ledger | Repository-backed findings and repair context |
| Cleanup-review correction | Repair handoff after command/protocol/dirty failure | Fixed initial plus two retries | Latest structured cleanup failure and fresh dirty evidence only |

No counter in one row consumes another row's budget. Command-queue attempts are internal to one operation attempt. Any Acceptance invocation that completes as a non-command-failure result ends consecutive command-failure recovery before its normal canonical or protocol routing. Cleanup-review never enters the ordinary Apply loop.

## Apply Budget Ownership

`max_iterations` is a per-change, active-run cumulative Apply-dispatch budget. One counter is created when a change first enters Apply in a process and is passed through serial CLI, TUI, and parallel orchestration into every later `execute_apply_loop` call for that change. It survives Acceptance FAIL-to-Apply cycles, ordinary command-failure recovery, task-format repair, escalation, and final-commit rejection repair. It resets only when the process starts a fresh run; it is not reconstructed from logs or persisted state.

The sole counter owner reserves/increments immediately before dispatching an Apply agent. Command-queue transport retries remain inside that reservation. Existing independent CLI/TUI workflow-loop counters must not impose a second Apply ceiling, and parallel `_initial_iteration` plumbing must either carry the shared counter or be removed. At 80% of a positive limit, the owner emits the configured warning once for the threshold crossing. Before reserving beyond the exact positive ceiling, it returns a typed `iteration_limit { change_id, attempts, latest_diagnostic }` outcome. CLI, TUI, and parallel run boundaries use that outcome to stop consistently and invoke existing `on_finish(status = iteration_limit, iteration = attempts)` ownership exactly once rather than converting it to an untyped command error.

## Apply Control Flow

For each reserved Apply attempt:

1. Check cancellation and the positive budget before dispatch.
2. Inspect current task/Git state, task-format state, blocked/rejecting handoff, and pending final-commit repair before choosing the command.
3. Run `pre_apply` in its existing position, execute through the retrying command queue, collect bounded stdout/stderr, and record `ApplyAttempt` regardless of status.
4. Preserve completion-finalized and cancellation routing, then classify permission denial.
5. For an ordinary non-zero result, run `on_error` once but do not return before repository evaluation.
6. Re-read tasks and Git state, apply completion and blocked/rejecting handoff, emit progress, and run the same WIP/progress/stall accounting used after a successful command. An ordinary non-zero result cannot be considered progress merely because output was produced.
7. Run `post_apply` only under its existing successful/owned-handoff eligibility; ordinary command failure does not newly authorize a success-style post hook.
8. Dispatch another attempt only if fresh routing says Apply remains eligible, stall policy has not stopped/escalated it, and positive budget remains.

This ordering ensures `max_iterations = 0` disables only the numeric ceiling. Repeated no-progress non-zero results still advance existing empty-WIP/no-progress tracking and reach diagnosis, escalation, or stalled termination. Prior narrative output never substitutes for fresh repository evidence.

## Acceptance Command Recovery

Add an active-run value equivalent to:

```rust
struct AcceptanceCommandRetryCounter {
    consecutive_failures: u32,
}

enum AcceptanceCommandRetryDecision {
    Retry { attempt: u32, max_retries: u32, diagnostic: CommandDiagnostic },
    Exhausted { attempts: u32, max_retries: u32, diagnostic: CommandDiagnostic },
}
```

Exact names may follow local style. Required semantics:

- `max_retries` is two, yielding at most three consecutive command invocations.
- `CommandDiagnostic` contains an error summary, exit code when available, and bounded latest stdout/stderr. It is Conflux-managed context, not an Acceptance verdict.
- `CommandFailed` records one consecutive failure. `Retry` invokes the normal configured Acceptance command again without entering Apply or cleanup-review.
- Every invocation that completes as a non-command-failure result resets command-failure state before its existing routing. This includes PASS, FAIL, CONTINUE, validated stalled/permission-stalled, missing-verdict, malformed-finding, bare-blocker correction, and other completed protocol-bearing output.
- Missing-verdict, malformed-finding, and bare-blocker correction retain their existing independent protocol counters after reset. They do not become command failures. Therefore `CommandFailed → MissingVerdict → CommandFailed` records the final command failure as the first consecutive failure of a new sequence.
- Cancellation is not `CommandFailed` and starts no retry.
- Exhaustion returns the existing terminal error shape with attempt count and latest bounded diagnostics.

### Parallel placement

The retry loop belongs around `execute_acceptance_in_workspace`, inside one applied-workspace cycle. It must not increment `cycle_count`, set up another Apply cycle, or use `skip_apply_once` as an indirect retry mechanism. This ensures `MAX_ACCEPTANCE_RETRY_CYCLES` remains reserved for FAIL-to-Apply repair cycles.

### Serial placement

The retry loop belongs inside `SerialRunService::run_acceptance_loop`, before returning `ChangeProcessResult`. A recoverable command failure must not escape as `AcceptanceCommandFailed` to the outer orchestrator, because that route currently advertises and performs Apply retry. Only exhausted command recovery returns the terminal result.

## Cleanup-Review Corrective Loop

One cleanup-review operation attempt produces a structured observation equivalent to:

```rust
enum CleanupReviewFailureKind {
    CommandFailed,
    MarkerMissing,
    MarkerDuplicate,
    DirtyRemains,
    StatusInspectionFailed,
    PermissionDenied,
    Cancelled,
}

struct CleanupReviewDiagnostic {
    kind: CleanupReviewFailureKind,
    exit_code: Option<i32>,
    stdout_tail: Option<String>,
    stderr_tail: Option<String>,
    marker_count: usize,
    status_tail: Option<String>,
}
```

Exact names may differ. All free-form fields use the existing bounded output policy. The next prompt contains only the latest diagnostic, enclosed as untrusted tool/repository output, followed by a trusted fixed action: inspect the actual worktree, repair only relevant changes, commit intended work, prove clean status, then emit exactly one standalone marker.

### Ordered validation

After every attempt, Conflux validates:

1. **Command ownership:** distinguish cancellation and classified permission denial before generic command failure. An ordinary unsuccessful command cannot pass.
2. **Protocol marker:** parse all output and require exactly one standalone `CLEANUP_REVIEW: CLEAN` line. Missing or duplicate markers fail.
3. **Repository truth:** run a fresh repository status query and require no tracked, staged, unstaged, or untracked changes. Status-query failure is not clean.

Success requires all three. A marker never overrides Git state, and clean Git state never invents the marker. For a failure with retry budget remaining, the next operation attempt receives the diagnostic. On the third failure, return terminal error and leave the worktree untouched beyond changes the cleanup agent already made.

### Cancellation

`run_post_apply_cleanup_review` must receive the per-change cancellation token. Both stream receive and child-status wait select on cancellation, terminate the managed child, drain/close owned output safely, and return the existing intentional-stop routing without another attempt.

### Permission denial

Classify bounded stdout/stderr with the shared permission classifier before generic cleanup failure. A classified cleanup permission denial immediately returns the existing non-terminal permission hold for the change. It starts no corrective cleanup attempt and does not increment the generic cleanup operation-failure counter. Cleanup-review does not create a new first/changed/repeated permission tracker; explicit operator retry after the permission condition changes re-enters cleanup from workspace/Git evidence with a fresh active-run cleanup budget.

## Prompt Trust Boundary

The cleanup-review retry prompt has two sources:

- trusted Conflux instructions defining required action and immutable success criteria;
- untrusted prior command/repository diagnostics, clearly delimited and bounded.

Agent output inside the diagnostic cannot add instructions, relax the marker count, authorize blind staging, or declare the repository clean. The skill continues to prohibit `git add -A` and indiscriminate commits.

Acceptance command diagnostics use the existing Acceptance prompt trust model: `src/agent/runner.rs` stores only the latest bounded command diagnostic separately from canonical `AcceptanceHistory`, `src/agent/prompt.rs` renders it as untrusted evidence, and serial/parallel execution clears it after any completed non-command-failure invocation. Prior failed output cannot be interpreted as a canonical current verdict or merged into missing-verdict protocol history.

## Restart and Workspace Evidence

All new counters and latest-diagnostic values live in the active execution future or service instance. No `ACCEPTANCE_REPORT.json`, cleanup report, retry marker, provider session ID, managed-job ID, cache, or log becomes a workflow-control input.

After restart:

- an applied, clean, unarchived workspace runs Acceptance again;
- a task-complete dirty managed workspace runs cleanup-review again before Acceptance;
- an incomplete workspace runs Apply based on tasks and Git evidence;
- no prior narrative output authorizes PASS, CLEAN, archive, or merge.

## Verification Strategy

Use deterministic shell fixtures and in-memory counters rather than external providers:

- Apply fixture exits non-zero after writing partial progress and diagnostics, then verifies the next invocation receives bounded history and completes.
- Apply budget fixtures cross serial/TUI/parallel and FAIL-to-Apply re-entry, proving one per-change count, exact positive bound, one 80% warning, typed `iteration_limit`, exact `on_finish` count/status, restart reset, and zero-unlimited multi-attempt behavior.
- Apply failure fixtures prove fresh progress/handoff/permission/stall evaluation, `on_error` cardinality, no success-style `post_apply`, and no-progress stall termination with `max_iterations = 0`.
- Shared Acceptance policy unit tests prove two retries, third-failure exhaustion, latest-only bounded context, and reset after every completed non-command-failure result, including `CommandFailed → MissingVerdict → CommandFailed`.
- Agent runner/prompt tests prove failed Acceptance output remains latest-only untrusted command evidence and cannot become a verdict or protocol history.
- Serial and parallel fixtures count Apply, cleanup, and Acceptance invocations to prove command retry is Acceptance-only and budget-independent.
- Cleanup fixtures retain canonical dirty-trigger, clean-skip, completion-grace, blocked-handoff-grace, and incomplete-Apply scenarios, then add all command/marker/status classification rows and second-attempt recovery.
- Cancellation fixture keeps a cleanup child alive, cancels it, and proves termination plus no next invocation.
- Permission fixture proves classified denial immediately enters non-terminal hold with zero corrective invocations and zero generic counter consumption.
- Restart fixtures create fresh service/driver instances against unchanged workspace state and prove no durable retry artifacts are consulted or created.

All default tests must complete under one second. Any unavoidable process-boundary test exceeding that limit must use the repository `heavy-tests` feature and retain fast default state-machine coverage of the same routing.

## Risks and Mitigations

- **Infinite Apply loop with zero limit:** zero intentionally disables only the numeric ceiling; cancellation, stall detection, permission holds, and completion checks remain active.
- **Accidental duplicate implementation after Acceptance failure:** place command retry inside Acceptance and assert Apply invocation count remains unchanged.
- **Budget coupling:** use separate counter types and tests that exhaust one policy while asserting the others remain zero.
- **False cleanup success:** preserve ordered marker-plus-fresh-Git validation and test marker-only/clean-only outcomes.
- **Prompt injection from prior output:** bound and delimit diagnostics as untrusted, with trusted fixed corrective instructions after them.
- **Cancellation leaks:** select on the same per-change token during both streaming and wait, and verify the child terminates before returning.
- **Constitution violation:** keep all recovery control state process-local and re-derive restart routing solely from workspace/Git evidence.
