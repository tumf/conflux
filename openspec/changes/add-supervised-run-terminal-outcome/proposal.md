---
change_type: implementation
priority: high
dependencies:
  - integrate-upstream-during-run
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/changes/integrate-upstream-during-run/proposal.md"
  - "openspec/specs/cli/spec.md"
  - "openspec/specs/external-lifecycle-integrations/spec.md"
  - "src/cli.rs"
  - "src/main.rs"
  - "src/orchestrator.rs"
  - "src/parallel/orchestration.rs"
  - "src/lifecycle_integration.rs"
  - "tests/run_exit_tests.rs"
verifications:
  - id: supervised-run-tests
    requirement: Supervised run mode emits exactly one schema-versioned terminal JSON record, exits promptly with the matching classified status, and preserves ordinary run behavior when disabled.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test output for supervised_run process and serialization cases
    rerun: cargo test supervised_run
    prerequisites:
      - integrate-upstream-during-run
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: add supervised run terminal outcomes

**Change Type**: implementation

## Problem / Context

`cflx run` currently exposes only the coarse lifecycle states `idle`, `working`, and `blocked`. `AllCompleted` and `Stopped` both project to `idle`, and an orchestrator error enters an unbounded operator-retry wait in `src/main.rs`. A one-container/one-job supervisor therefore cannot distinguish remote-confirmed completion, a resumable hold, cancellation, fatal failure, or process crash from lifecycle state alone.

The existing external lifecycle adapter cannot become the authoritative terminal-result transport. It is observability-only, may drop messages under backpressure, and must not affect cflx completion. A supervised job needs a one-shot process contract whose structured result and OS exit status agree while workflow and resume routing remain repository-derived.

This change consumes the explicit parallel scheduler/finalization outcome introduced by `integrate-upstream-during-run`. That repository-integrated output is required to distinguish remote-confirmed completion from blocked, stalled, cancelled, verification-failed, and push-failed finalization.

## Proposed Solution

Add `--supervised` to non-interactive `cflx run`.

In supervised mode:

- execution is one-shot and never waits for a web/operator retry after the orchestrator returns a terminal outcome;
- stdout is reserved for exactly one compact JSONL `run_terminal` record on every controlled completion after supervised-mode initialization;
- human progress, tracing, warnings, and diagnostics are written to stderr;
- the terminal record uses `schema_version: 1`, a stable outcome vocabulary, privacy-limited reason data, requested/change-result classifications, and optional upstream identity only when observed;
- the process exits `0` for `completed`, `2` for resumable `blocked` or `stalled`, `3` for `cancelled` including graceful SIGTERM/SIGINT handling, and `1` for `failed`;
- a crash, SIGKILL, abort before supervised-mode initialization, or inability to emit the record is identified by absence of a valid terminal record plus process status;
- terminal output remains observability for workflow routing: a later invocation recomputes its next action from workspace and Git evidence.

Ordinary `cflx run`, TUI, server mode, web-control retry behavior, and the lossy external lifecycle stream retain their existing contracts.

## Acceptance Criteria

1. `cflx run --supervised` is non-interactive, one-shot, and exits promptly after a terminal outcome instead of entering the ordinary outer retry wait.
2. Every controlled supervised exit after initialization writes exactly one compact JSON object followed by one newline to stdout; stdout contains no startup, progress, tracing, warning, or error text.
3. The terminal record contains `schema_version`, `type`, `outcome`, `resumable`, `selected_changes`, `processed_changes`, `already_completed_changes`, and `pending_changes`; optional remote/branch/head fields are present only when repository observation establishes them.
4. Outcomes are limited to `completed`, `blocked`, `stalled`, `cancelled`, and `failed`, and map deterministically to exit codes `0`, `2`, `2`, `3`, and `1` respectively.
5. Upstream-enabled completion is reported only after native push and remote confirmation; verification, push, auth, config, and unrecoverable command failures cannot report `completed`.
6. Graceful SIGTERM/SIGINT produces `cancelled` and exit code `3` after bounded cleanup; SIGKILL/process crash is distinguishable by a missing valid terminal record.
7. The record exposes only typed reason codes and bounded sanitized details; it contains no credentials, environment values, prompts, unrestricted command output, or config dump.
8. The existing lifecycle adapter remains lossy and observability-only; its success or failure cannot change the terminal record, exit code, or repository-derived resume action.
9. Without `--supervised`, existing stdout logging, outer retry/web controls, lifecycle messages, and process exit behavior remain unchanged.

## Explicit Completion Conditions

- `RunArgs` parses `--supervised` only for `cflx run`, with parser tests proving default-off behavior.
- A typed `RunTerminalOutcome` and versioned serializable record model cover all outcome/exit-code mappings without parsing human-readable errors.
- `Orchestrator` and parallel finalization return typed outcome/reason data sufficient to classify completed, blocked, stalled, cancelled, and failed paths.
- `src/main.rs` separates ordinary retry-loop handling from supervised one-shot handling and centralizes exactly-once terminal emission before selecting the matching process exit code.
- Supervised logging routes all non-result output to stderr, while ordinary run logging remains unchanged.
- Process tests exercise success, blocked/stalled, controlled failure, graceful cancellation, no-record crash simulation, output-channel separation, exactly-once emission, and default-off regression paths.
- `cargo test supervised_run`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- Making the external lifecycle adapter authoritative or lossless.
- Adding a supervisor daemon, container runtime, lease, retry API, or database schema to this repository.
- Persisting terminal records as workflow-control state.
- Automatically retrying a supervised job inside the cflx process.
- Treating SIGKILL or a process crash as a controlled cflx terminal outcome.
