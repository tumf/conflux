---
change_type: hybrid
priority: high
dependencies: []
references:
  - src/execution/apply.rs
  - src/ai_command_runner.rs
  - src/command_queue.rs
  - src/process_manager.rs
  - src/tui/run_supervisor.rs
  - src/main.rs
  - skills/cflx-apply/SKILL.md
  - skills/cflx-proposal/SKILL.md
verifications:
  - id: apply-interruption-tests
    requirement: Interrupted and runtime-limited Apply preserves repository progress without automatic redispatch
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/execution/apply.rs
    evidence: Apply-loop unit tests prove cleanup, WIP snapshot, terminal classification, and restart-visible progress
    rerun: cargo test --locked execution::apply::tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: command-runtime-tests
    requirement: Absolute runtime limits terminate owned process groups independently of output activity
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/process_cleanup_test.rs
    evidence: Process cleanup integration tests prove timeout, SIGTERM/SIGKILL escalation, and quiescence
    rerun: cargo test --locked --features heavy-tests --test process_cleanup_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: tui-shutdown-tests
    requirement: TUI external shutdown drains the run command scope and leaves no owned descendants
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/tui/run_supervisor.rs
    evidence: TUI supervisor tests prove SIGINT/SIGTERM share the bounded shutdown boundary
    rerun: cargo test --locked tui::run_supervisor::tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: skill-contract-tests
    requirement: Apply and proposal guidance prevent unbounded or duplicated verification work
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/install_skills_test.rs
    evidence: Embedded skill contract tests assert bounded verification retries, blocker handoff, and heavy-gate ownership guidance
    rerun: cargo test --locked --test install_skills_test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent runaway Apply execution

**Change Type**: hybrid

## Problem / Context

A managed Apply command can continue indefinitely while emitting output because Conflux only enforces inactivity timeout, not an absolute invocation deadline. If an operator interrupts that work, the Apply cancellation path can return before preserving dirty workspace progress, so a later run re-enters from workspace evidence that still looks like the first Apply attempt. The portable Apply skill also requires foreground verification and repeated repair without defining a finite retry boundary, which permits self-invented stability loops and multi-hour repository gates.

TUI internal stop already owns bounded process-group cleanup, but an external SIGINT or SIGTERM must reach the same run cancellation, process quiescence, and progress-preservation boundary. Otherwise the TUI process can exit while detached agent, shell, Cargo, or test descendants remain active.

## Proposed Solution

Create one atomic Apply safety boundary with four coordinated behaviors:

1. Preserve dirty managed-worktree progress as a Conflux WIP snapshot after owned process-group quiescence whenever Apply is cancelled, externally terminated, or stopped by its absolute runtime limit.
2. Add a configurable `command_max_runtime_secs` absolute deadline, defaulting to 3,600 seconds and disabled by `0`, independent of output activity and inactivity timeout.
3. Update `cflx-apply` and `cflx-proposal` guidance so verification is single-run by default, unchanged stability loops are prohibited, retries require new repair or environment-recovery evidence, and non-completing verification produces a structured blocker rather than indefinite work.
4. Route TUI SIGINT and SIGTERM through the same bounded shutdown boundary as operator stop, closing command admission, terminating and proving quiescence for owned process groups, preserving Apply progress, then exiting.

These behaviors ship together because a deadline without progress preservation loses work, preservation without process quiescence races repository mutation, and guidance without runtime enforcement cannot bound a misbehaving agent.

## Acceptance Criteria

- Cancelling a dirty managed Apply terminates its owned process group, proves quiescence, creates a WIP snapshot containing staged, unstaged, and untracked change-owned progress, and does not dispatch Acceptance.
- A fresh process derives the next action from the preserved workspace and Git state without consulting logs or other external durable state.
- `command_max_runtime_secs` defaults to 3,600 seconds, accepts `0` as disabled, follows normal configuration precedence, and is independent of output activity.
- An agent that continuously emits output is terminated when the absolute deadline expires.
- Runtime-limit termination is classified distinctly from an ordinary crash and is not automatically retried in the same run.
- TUI SIGINT and SIGTERM close run command admission, cancel the run, terminate owned process groups through the existing graceful-then-forceful path, prove quiescence, preserve dirty Apply progress, and leave no owned descendants.
- If process quiescence or WIP preservation cannot be proven, Conflux exits non-zero with actionable diagnostics and retains workspace contents without reporting successful cleanup.
- `cflx-apply` prohibits no-change stability loops, permits at most three executions of the same verification command only when each retry follows repository repair or concrete environment recovery, and records `verification_timeout` or `verification_unstable` blocker evidence when bounded verification cannot complete.
- `cflx-proposal` keeps Docker, database, heavy, credentialed, deployed, and long-running repository-wide gates out of Apply-blocking checkbox tasks unless a bounded repository-local verification path exists.

## Explicit Completion Conditions

- Apply cancellation and absolute-timeout branches share a tested helper that performs process-group cleanup before any WIP snapshot and returns a typed terminal outcome that suppresses same-run redispatch.
- Configuration types, merge behavior, defaults, generated examples, and command-runner wiring expose `command_max_runtime_secs` consistently.
- Timeout tests use deterministic paused time or controlled process fixtures rather than short wall-clock correctness assertions.
- TUI signal tests exercise the same supervisor shutdown boundary used by operator stop and verify no registered execution or process identity remains.
- Embedded skill source and installed-skill contract tests contain the bounded verification and heavy-gate ownership rules.
- `cargo test --locked execution::apply::tests`, `cargo test --locked --features heavy-tests --test process_cleanup_test`, `cargo test --locked tui::run_supervisor::tests`, and `cargo test --locked --test install_skills_test` pass. `tests/process_cleanup_test.rs` is gated behind the `heavy-tests` feature because it drives real process groups, so its rerun command carries that feature explicitly rather than silently running zero tests.
- Existing path-scoped pre-commit hooks remain responsible for repository-wide rustfmt and clippy when Rust paths are staged; this proposal does not duplicate them as Apply checkbox tasks.

## Out of Scope

- Changing Corvus Cargo profiles, build parallelism, or test implementation.
- Detecting semantic repetition by parsing arbitrary agent shell commands.
- Persisting process-local retry counters or execution state outside the workspace.
- Changing Acceptance, Archive, Resolve, or Analyze operation-specific runtime limits beyond their use of the common command deadline.
- Adding deployed-service or credentialed post-integration verification.
