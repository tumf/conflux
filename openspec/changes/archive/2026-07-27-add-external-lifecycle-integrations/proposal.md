---
change_type: implementation
priority: high
dependencies: []
references:
  - src/events.rs
  - src/main.rs
  - src/tui/orchestrator.rs
  - src/tui/runner.rs
  - openspec/specs/frontend-abstraction/spec.md
verifications:
  - id: lifecycle-integration-tests
    requirement: External lifecycle adapters receive real cflx process and TUI state transitions without controlling workflow state
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: scripts/test-time-top10.sh
    evidence: cargo test output covering adapter protocol, TUI transitions, failure isolation, and shutdown
    rerun: cargo test --all-targets
    prerequisites: []
---

# Add External Lifecycle Integrations

**Change Type**: implementation

## Problem / Context

Herdr can automatically track tools such as OpenCode when the tool exposes lifecycle events to an installed integration. Conflux currently exposes orchestration events internally through `EventSink`, but stock `cflx` cannot load an external lifecycle adapter. Existing workflow hooks start after orchestration begins and cannot reliably represent TUI startup, ready state, confirmation dialogs, or process shutdown.

As a result, a normal `cflx` or `cflx tui` invocation inside a Herdr pane cannot provide complete lifecycle state without replacing the `cflx` command or requiring a plugin-owned pane. Command replacement is not an acceptable integration mechanism.

## Proposed Solution

Add an opt-in external lifecycle integration contract to cflx. Configuration identifies an adapter as an argv command. cflx starts the adapter as a child process, sends versioned newline-delimited JSON lifecycle events over stdin, and closes stdin during shutdown. The adapter receives the inherited process environment, allowing a Herdr adapter to use `HERDR_ENV`, `HERDR_SOCKET_PATH`, and `HERDR_PANE_ID` without cflx depending on Herdr.

The lifecycle stream covers process start, semantic `idle`, `working`, and `blocked` transitions, optional session identity, and process shutdown. TUI state changes and confirmation dialogs feed the same integration path as orchestration events. Adapter startup, write, timeout, or exit failures are non-fatal observability failures and never influence workflow routing.

Keep this as one proposal because the protocol, runtime wiring, and state transition coverage must ship together; a protocol without live TUI/process wiring would not satisfy the required behavior.

## Acceptance Criteria

- A user can configure an external lifecycle adapter without replacing or wrapping the `cflx` executable.
- Bare `cflx` and `cflx tui` start the configured adapter before presenting the interactive TUI.
- The adapter receives versioned JSON events for process start, deduplicated semantic state changes, and shutdown.
- Ready/selection UI reports `idle`; active orchestration and stopping report `working`; confirmation or retry decisions requiring user input report `blocked`.
- Non-interactive `cflx run` reports process and orchestration lifecycle through the same contract.
- Missing, disabled, crashing, slow, or malformed adapters do not prevent cflx startup, execution, or shutdown and do not modify workflow decisions.
- Adapter events contain no secrets or full terminal contents by default.
- Tests exercise a real fixture adapter and fail if event delivery is stubbed or disconnected from runtime state changes.
- Documentation describes installation, configuration, protocol versioning, environment inheritance, and a Herdr adapter implementation path.

## Explicit Completion Conditions

- Configuration parsing and validation accept an argv-based lifecycle integration and reject empty commands with actionable diagnostics.
- A reusable lifecycle dispatcher is connected to TUI startup/state/shutdown and non-interactive run startup/state/shutdown.
- The dispatcher has bounded buffering and bounded shutdown behavior; adapter backpressure cannot hang cflx.
- Repository tests assert event ordering and payloads from executable fixture adapters, including disabled configuration and adapter failure cases.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets` pass.

## Out of Scope

- Adding `cflx` process recognition to Herdr core.
- Replacing `cflx` on `PATH`, shell aliases, or wrapper binaries.
- A dynamic-library ABI or arbitrary in-process third-party code loading.
- Allowing lifecycle adapters to issue workflow commands or become authoritative workflow state.
- Persisting lifecycle integration state outside the workspace for resume or routing decisions.
