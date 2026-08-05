---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-editor/spec.md
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/runtime-state/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestrator.rs
  - src/orchestration/state.rs
  - src/orchestration/operator_command.rs
verifications:
  - id: serial-removal-tests
    requirement: Default CLI, TUI, and API execution uses the worktree orchestration path and exposes no serial-mode selector or fallback
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering CLI parsing, startup validation, reducer transitions, TUI controls, remote-control DTOs, and single-change execution
    rerun: make test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: serial-removal-lint
    requirement: Removing serial-only types and branches leaves no dead or warning-producing Rust code
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: clippy output with warnings denied
    rerun: make lint
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Remove obsolete serial execution mode

**Change Type**: implementation

## Problem / Context

Conflux still carries an executable serial/sequential orchestration path even though the runtime-state specification identifies serial mode as obsolete. The legacy path is selected through `--parallel`, `parallel_mode`, TUI controls, and `/api/v2` operator commands. It also forces mode-specific terminal semantics, hook contexts, tests, and documentation to remain synchronized with the cumulative worktree orchestrator.

This creates two competing workflow authorities and allows current behavior to diverge depending on a legacy toggle. The removal must update runtime wiring and canonical contracts together; changing only defaults would leave the obsolete path reachable.

## Proposed Solution

Make cumulative Git-worktree orchestration the only execution path for local TUI and `cflx run`.

- Remove the serial branch, `SerialRunService`, serial-only state transitions, hook constructors, and fallback behavior.
- Remove `--parallel`, `parallel_mode`, `resolve_parallel_mode`, the TUI `=` toggle/badge, and the `/api/v2` `set_parallel_mode` command.
- Require a usable Git repository before orchestration, hooks, lifecycle adapters, AI commands, or workspace mutation starts.
- Keep concurrency, dry-run, VCS selection, resume, merge, push, retry, and stop controls as ordinary options of the sole execution path.
- Reject removed `parallel_mode` configuration with a migration-oriented error instead of silently selecting another behavior.
- Treat a parallel-ineligible change as ineligible; never route it through a serial fallback.
- Update canonical specs, help, guides, and tests to describe one execution model.

## Atomic Scope Rationale

CLI/config selection, TUI/API controls, reducer terminal semantics, and orchestration dispatch are tightly coupled. Splitting them would temporarily expose controls with no valid target or preserve a hidden serial fallback, so they must ship as one change.

## Acceptance Criteria

- `cflx run` and local TUI always dispatch through cumulative worktree orchestration, including a single selected change.
- No CLI, config, TUI, or `/api/v2` surface can enable or disable an execution mode.
- Startup outside a usable Git repository fails before any hook, lifecycle adapter, AI subprocess, or managed-worktree mutation starts.
- Existing configuration containing `parallel_mode` fails with an actionable removal message.
- Parallel-ineligible changes remain excluded and cannot fall back to serial execution.
- Archive completion always follows the configured worktree post-archive action and no reducer branch treats archive as terminal solely because execution is serial.
- Concurrency, dry-run, VCS backend, resume, merge, push, stop, and retry behavior remain available on the sole path.
- Canonical specs, CLI help, user guides, and tests no longer describe serial/sequential as a selectable execution mode.
- Uses of “sequential” that describe ordered merge integration rather than an execution mode remain valid and are not mechanically removed.

## Explicit Completion Conditions

- `src/orchestrator.rs` contains no serial dispatch loop and no `SerialRunService` runtime dependency.
- `ExecutionMode::Serial`, serial hook-context constructors, `parallel_mode`, `resolve_parallel_mode`, CLI `--parallel`, TUI mode-toggle handling, and remote `set_parallel_mode` DTO/executor wiring are absent.
- Repository-local tests prove default and single-change runs reach the worktree path, removed selectors are rejected, Git preflight is side-effect-free, and ineligible changes do not execute.
- `make test` and `make lint` pass.
- `cflx openspec validate remove-serial-execution-mode --archive-gate` passes when implementation is complete.

## Migration

- Remove `parallel_mode` from `.cflx.jsonc` and other Conflux configuration files.
- Remove `--parallel` from scripts and aliases; no replacement flag is needed.
- Remove clients that send `/api/v2` `set_parallel_mode`; execution mode is no longer mutable.
- Run Conflux only inside a Git repository with the `git` command available.

## Out of Scope

- Replacing Git worktrees with another workspace backend.
- Changing dependency analysis, concurrency scheduling, merge ordering, or push semantics beyond removal of serial fallback.
- Renaming “sequential resolve” where it accurately means ordered integration within the worktree merge algorithm.
- Amending the Constitution; this change preserves workspace-local workflow authority.
