## Context
- When merge progress to the base branch stops, orchestration stalls even after apply/archive operations complete successfully, but this is difficult to notice.
- In **parallel mode**, merges from worktrees to the base branch create `Merge change: <change_id>` commits.
- By monitoring the progress of these commits, merge stalls can be detected and the system can stop immediately.
- In **serial mode**, `Merge change:` commits are not created, so this monitoring feature does not apply.

## Goals / Non-Goals
- Goals:
  - Detect orchestration stall when no merge progress occurs for 30 minutes and stop immediately.
  - Apply **only to parallel mode** (serial mode is out of scope).
  - Propagate stop reason to events and logs.
  - Make threshold and monitoring interval configurable.
- Non-Goals:
  - Analyze or auto-recover from merge stall root causes.
  - Stop individual changes (only global stop).
  - Monitor serial mode (no `Merge change:` commits exist).

## Decisions
- Implement monitoring as a Tokio task with periodic checks using `tokio::time::interval`.
- Determine merge progress based on the last timestamp of `Merge change: <change_id>` commits on the base branch.
- On stall detection, trigger `CancellationToken` immediately and integrate with existing stop flow.
- Launch monitoring task **only in parallel mode** (not in serial mode).

## Risks / Trade-offs
- If monitoring interval is too short, git log execution frequency increases; default is set to avoid increasing current run loop load.
- Stall detection is based only on merge progress, so long-running apply operations may cause false positives.
- Serial mode is out of scope; if similar monitoring is needed for serial mode, a different approach is required.

## Why Parallel Mode Only
- **Serial mode architecture**: Serial mode executes apply/archive directly in the main repository without worktree isolation.
- **No merge commits**: Serial mode does not create `Merge change: <change_id>` commits, so the concept of merge progress does not exist.
- **Monitoring dependency**: MergeStallMonitor relies on `Merge change:` commit timestamps, so it can only operate in parallel mode.
- **Alternative for serial**: Serial mode continues to use existing error detection circuit breakers.

## Migration Plan
- Add configuration options with default values for unset cases.
- Operate independently from existing stall/circuit-breaker logic.

## Open Questions
- None (requirements defined)
