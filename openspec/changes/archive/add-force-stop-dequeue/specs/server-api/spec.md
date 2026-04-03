## ADDED Requirements

### Requirement: Per-change stop-and-dequeue API for running changes

The server SHALL provide a per-change control operation that force-stops a running change and returns it to display state `not queued` instead of terminal `stopped`.

The API MUST target an individual change within a project and MUST trigger the same runtime semantics used by the local TUI stop-and-dequeue flow.

#### Scenario: API stops running change and clears queue state

- **GIVEN** project `proj-1` has change `foo` currently running
- **WHEN** the client calls the per-change stop-and-dequeue API for `foo`
- **THEN** the server requests cancellation for `foo`
- **AND** once cancellation is confirmed the change state becomes `not queued`
- **AND** subsequent REST or WebSocket state payloads report `foo` as not queued rather than `stopped`

#### Scenario: API does not stop unrelated changes

- **GIVEN** project `proj-1` has running changes `foo` and `bar`
- **WHEN** the client calls the per-change stop-and-dequeue API for `foo`
- **THEN** only `foo` is cancelled and dequeued
- **AND** `bar` continues running unchanged

### Requirement: Dashboard can invoke stop-and-dequeue for active changes

The dashboard SHALL expose a per-change control for active changes that invokes the server stop-and-dequeue operation.

#### Scenario: Dashboard action updates visible change status

- **GIVEN** the dashboard shows a change row for an active change
- **WHEN** the user invokes the stop-and-dequeue action from that row
- **THEN** the dashboard calls the server per-change stop-and-dequeue API
- **AND** the row eventually updates to `not queued`
- **AND** the row does not remain in `stopped` terminal display
