## MODIFIED Requirements

### Requirement: Typed frontend lifecycle emission

TUI and non-interactive frontends MUST publish lifecycle state from typed runtime state and actions rather than rendered-screen scraping. The TUI lifecycle snapshot MUST represent execution mode independently from modal interaction state. This lifecycle publication MUST preserve the existing `EventSink` and `ReducerCommand` ownership boundaries.

#### Scenario: Confirmation dialog reports blocked

- **GIVEN** the TUI enters a confirmation or retry interaction requiring user input
- **WHEN** the typed TUI state changes
- **THEN** the lifecycle dispatcher receives `blocked`
- **AND** confirmation context is derived from the typed modal payload when applicable
- **AND** no terminal buffer parsing is used

#### Scenario: QR overlay preserves underlying lifecycle

- **GIVEN** the TUI displays the QR overlay while execution is idle, working, stopping, stopped, or error
- **WHEN** the typed TUI state is projected to an external lifecycle event
- **THEN** the lifecycle state is derived from the underlying execution mode
- **AND** QR presentation alone does not report `blocked`

#### Scenario: Adapter cannot mutate core state

- **GIVEN** an external lifecycle adapter is active
- **WHEN** it receives events or exits with an error
- **THEN** Core state changes still occur only through existing Core command paths
- **AND** adapter behavior cannot select the next workflow action
