## MODIFIED Requirements

### Requirement: No Behavioral Changes

TUI refactoring SHALL NOT change existing keyboard shortcuts, display behavior, remote-client semantics, or state transitions except where explicitly required for shutdown safety.

When local TUI mode exits while local orchestration launched by that TUI is active, the TUI SHALL treat exit as a cancellation boundary for that local work. It MUST cancel the local orchestrator and its invocation-scoped run command scope, wait for bounded scheduler and owned-process cleanup, and prevent any local orchestrator task or owned AI command process group spawned by that run from continuing after TUI cleanup has completed. The local shutdown deadline MUST NOT expire before the scheduler's outer cancellation boundary has had its configured opportunity to complete command cleanup and pending merge/base-lane handling.

If the orchestrator does not finish within that bound, the TUI MUST use the scope retained outside the orchestrator task to forcefully clean and verify retained owned process identities before aborting and joining the orchestrator task. Task abort alone MUST NOT be treated as child-process cleanup evidence.

Remote TUI mode (`--server`) is excluded from local-work cancellation semantics: closing the remote TUI client MUST NOT implicitly stop remote server-side work.

#### Scenario: Local TUI quit cancels active local orchestration

- **GIVEN** `cflx tui` is running in local mode
- **AND** the TUI has started local orchestration work with an invocation-scoped command scope
- **WHEN** the user exits the TUI
- **THEN** the local orchestrator cancellation token and run command scope are cancelled immediately
- **AND** the TUI waits through the bounded scheduler cleanup boundary
- **AND** active local orchestration and its owned AI command process groups are not allowed to continue after TUI cleanup completes

#### Scenario: Timed-out local orchestrator cleanup force-cleans before task abort

- **GIVEN** local TUI cleanup has requested orchestrator and run-command-scope cancellation
- **AND** the orchestrator task does not finish within the bounded outer cleanup period
- **WHEN** the period expires
- **THEN** the TUI uses retained owned process identities to perform forceful cleanup and quiescence verification
- **AND** deterministic cleanable process groups have no surviving members before the TUI aborts and joins the orchestrator task
- **AND** the task is not silently detached by dropping its join handle
- **AND** an operator-visible diagnostic distinguishes forced abort, unconfirmed cleanup, and graceful completion

#### Scenario: Local shutdown does not undercut command cleanup

- **GIVEN** run-owned command cleanup has a bounded graceful-then-forceful deadline
- **AND** the scheduler has a larger outer cancellation boundary
- **WHEN** local TUI shutdown waits for the active run
- **THEN** its grace does not expire earlier than the scheduler outer boundary
- **AND** nested command and merge cleanup consume one bounded cancellation timeline rather than resetting additive full deadlines

#### Scenario: Remote TUI quit does not stop remote work

- **GIVEN** `cflx tui --server <endpoint>` is connected to a remote server
- **AND** remote work may be running on that server
- **WHEN** the user exits the TUI client
- **THEN** the TUI cancels local client tasks such as rendering, refresh, and WebSocket subscription
- **AND** the TUI does not send an implicit stop or force-stop command to the remote server
