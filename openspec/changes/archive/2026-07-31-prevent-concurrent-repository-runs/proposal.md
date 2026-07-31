---
change_type: implementation
priority: high
dependencies: []
references:
  - src/main.rs
  - src/web/mod.rs
  - tests/run_exit_tests.rs
  - openspec/specs/cli/spec.md
verifications:
  - id: repository-lock-tests
    requirement: Repository-scoped orchestration locking and conflict diagnostics work across processes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests output
    rerun: cargo test --test run_exit_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent Concurrent Repository Runs

**Change Type**: implementation

## Problem / Context

Conflux currently has process-local guards for selected operations, but separate `cflx` processes can orchestrate the same Git repository concurrently. Concurrent `run`, local TUI, or server orchestration can race while creating or deleting worktrees, changing OpenSpec files, archiving changes, and merging or pushing branches.

Operators also lack a useful way to identify the process that already owns a repository. When that process exposes the Web/API server, the conflicting invocation should show its reachable endpoint so the operator can use the running instance instead of starting another one.

## Proposed Solution

Add one repository-scoped, non-blocking exclusive process lock shared by all local orchestration entrypoints.

- Resolve repository identity from the canonical Git common directory so linked worktrees and alternate paths share one lock.
- Store the lock file in the Git common directory and hold its open file descriptor for the full lifetime of the orchestration process.
- Use OS-managed advisory locking on supported platforms so process exit, including abnormal exit, releases ownership without stale-lock deletion logic.
- Acquire the lock before logging adapters, web listeners, AI subprocesses, or orchestration work begin.
- Record diagnostic metadata after acquisition: PID, start time, canonical workspace, and invocation mode.
- After an API listener binds and its actual address is known, atomically refresh the diagnostic metadata with the API base URL.
- If acquisition fails, exit before side effects and report all valid available owner metadata, including the API URL when present.

The metadata is observability-only. Lock ownership is determined solely by the OS lock, not by PID files or metadata contents, preserving workspace-derived workflow decisions required by the constitution.

## Acceptance Criteria

- A second local `cflx run`, local TUI, or `cflx server` invocation targeting the same Git common directory exits non-zero before starting orchestration or listeners.
- The conflict message identifies the existing process using valid available PID, invocation mode, start time, and workspace metadata.
- If the existing process has successfully bound an API listener, the conflict message includes the actual API base URL, including an OS-assigned port.
- If no API listener is active or binding has not completed, the conflict message remains valid and omits the API URL.
- Different Git repositories can run Conflux concurrently.
- Different linked worktrees belonging to the same Git common directory conflict with each other.
- Normal and abnormal process termination release the lock through OS file-descriptor semantics; stale metadata alone never blocks startup.
- Read-only and maintenance commands that do not start local orchestration remain usable while another process owns the lock.
- Remote-client TUI mode does not acquire a local repository orchestration lock because it does not own local orchestration.

## Explicit Completion Conditions

- A repository-lock module resolves the canonical Git common directory, acquires a non-blocking process lock, owns it through RAII, and writes diagnostic metadata without using metadata as the authority for exclusion.
- Default TUI, explicit local TUI, `run`, and `server` startup paths acquire and retain the same lock before side effects; remote TUI and non-orchestration commands bypass it.
- Web/API startup updates owner metadata only after successful bind, using the actual URL returned by the listener startup path.
- Process-level integration tests launch competing `cflx` processes and prove same-repository rejection, diagnostic output with and without API URL, linked-worktree exclusion, different-repository concurrency, and lock release after owner termination.
- `cargo test --test run_exit_tests`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo fmt --check` pass.

## Out of Scope

- Coordinating repositories across different machines or network filesystems without reliable local advisory-lock semantics.
- Adding a `--force` option to break or bypass a live lock.
- Serializing individual change execution inside one Conflux process; existing parallel scheduling remains unchanged.
- Treating lock metadata as durable workflow state or using it for resume, acceptance, archive, or merge decisions.
