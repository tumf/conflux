## MODIFIED Requirements

### Requirement: No Behavioral Changes

TUI refactoring SHALL NOT change existing keyboard shortcuts, display behavior, or state transitions except where explicitly required for shutdown safety.

When local TUI mode exits while local orchestration launched by that TUI is active, the TUI SHALL treat exit as a cancellation boundary for that local work. It MUST cancel the local orchestrator, wait for bounded cleanup, and prevent any local orchestrator task spawned by that TUI from continuing after TUI cleanup has completed.

Remote TUI mode (`--server`) is excluded from local-work cancellation semantics: closing the remote TUI client MUST NOT implicitly stop remote server-side work.

<!-- Expected canonical result after archive: `tui-architecture` will require local TUI exit to cancel and stop local orchestrator tasks decisively while preserving remote client quit semantics. -->

#### Scenario: Local TUI quit cancels active local orchestration

- **GIVEN** `cflx tui` is running in local mode
- **AND** the TUI has started local orchestration work
- **WHEN** the user exits the TUI
- **THEN** the local orchestrator cancellation token is cancelled
- **AND** active local orchestration is not allowed to continue after TUI cleanup completes

#### Scenario: Timed-out local orchestrator cleanup aborts the task

- **GIVEN** local TUI cleanup has requested orchestrator cancellation
- **AND** the orchestrator task does not finish within the bounded cleanup grace period
- **WHEN** the grace period expires
- **THEN** the TUI aborts the local orchestrator task
- **AND** the task is not silently detached by dropping its join handle
- **AND** an operator-visible diagnostic distinguishes forced abort from graceful completion

#### Scenario: Remote TUI quit does not stop remote work

- **GIVEN** `cflx tui --server <endpoint>` is connected to a remote server
- **AND** remote work may be running on that server
- **WHEN** the user exits the TUI client
- **THEN** the TUI cancels local client tasks such as rendering, refresh, and WebSocket subscription
- **AND** the TUI does not send an implicit stop or force-stop command to the remote server
