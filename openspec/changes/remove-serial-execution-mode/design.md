# Design: Single worktree orchestration path

## Decision

Conflux will have one runtime execution model: cumulative Git-worktree orchestration. A run with one change is the one-worker case of the same scheduler, not a separate serial mode.

## Current Split

`Orchestrator::run` currently checks a mode boolean. The worktree branch resolves targets, analyzes dependencies, creates managed workspaces, applies and accepts changes, archives them, then performs the configured post-archive action. The alternate branch initializes `SerialRunService`, repeatedly selects a change in the repository root, and treats archive as terminal.

The mode also leaks into configuration, CLI parsing, TUI state, remote-control DTOs, operator eligibility, reducer terminal transitions, hook context construction, documentation, and tests.

## Target Flow

1. CLI or TUI startup loads configuration and validates the repository and Git command.
2. Startup binds required local API listeners, but starts no lifecycle adapter, hook, AI process, or workspace mutation until Git preflight succeeds.
3. The frontend resolves selected targets and dispatches the cumulative worktree scheduler.
4. Every selected change, including a single change, executes in a managed worktree.
5. Archived changes proceed through the configured merge-to-base or push-to-remote action.
6. Reducer state reaches its terminal state through that post-archive outcome; there is no archive-is-terminal mode branch.

## Compatibility Decisions

### Removed CLI flag

`--parallel` is removed rather than retained as a no-op. A no-op flag would preserve a false mode distinction and conceal stale automation. Clap must reject it and help output must omit it.

### Removed configuration

Serde currently permits optional `parallel_mode`. Removal must detect this known retired key and return an actionable error. General unknown-key policy is unchanged; this migration check is limited to the retired key.

### Git-only execution

No serial fallback remains outside Git repositories. Validation must happen before observable orchestration side effects. Read-only commands that do not execute orchestration remain unaffected.

### Ineligible changes

Eligibility becomes unconditional input to execution selection. An ineligible change remains visible with its reason but cannot be marked or queued for execution and cannot be diverted to a legacy path.

## State Simplification

Remove `ExecutionMode` if no non-test consumer needs a mode after branch deletion. State constructors and reducers should model the sole archive-to-post-archive transition directly. Do not replace the enum with another one-value mode abstraction.

Hook contexts always have managed-worktree identity during change execution. Run-level hooks remain workspace-neutral where they already represent the whole run.

## Deletion Order

1. Add regression tests for default dispatch, Git preflight, removed inputs, and terminal transitions.
2. Route all frontends to the worktree scheduler.
3. Remove mode selectors and operator commands.
4. Remove reducer and hook mode branches.
5. Delete unreachable serial services and tests.
6. Update canonical specs and documentation.

This order keeps compiler errors useful while ensuring tests fail if the implementation merely changes the default and leaves the fallback reachable.

## Verification Strategy

- `unit`: CLI parser rejects `--parallel`; configuration loader reports retired `parallel_mode`; reducer has one archive transition.
- `integration`: default and explicit single-change runs emit the existing parallel-start marker/use managed workspace behavior; non-Git startup proves no hooks or agent commands ran.
- `integration`: TUI and remote API snapshots contain no toggle action; ineligible changes cannot be queued.
- `e2e`: existing worktree dry-run, merge, push, resume, retry, and stop tests continue to pass through `make test`.
- `unit`: source-level behavior is preferred over brittle string scans; compile failures and targeted tests establish deleted public/internal surfaces.
