## MODIFIED Requirements

### Requirement: Dashboard UI - Real-time Updates

The web dashboard SHALL automatically update when orchestrator state changes.
The web dashboard SHALL render a fresh initial state snapshot on page load.
The web dashboard SHALL fall back to polling when WebSocket updates are unavailable.

Explicit user-driven selection toggles MUST NOT feel deferred until the next background `full_state` or polling cycle. When the user toggles a change checkbox, the dashboard SHALL reflect the intended selection state immediately and then reconcile against the server-confirmed value.

#### Scenario: live-selection-toggle-does-not-wait-for-full-state
- **GIVEN** the dashboard is connected and displays a change row with `selected = false`
- **WHEN** the user toggles the checkbox for that row
- **THEN** the dashboard immediately updates the row's visible checkbox state
- **AND** the row does not wait for the next periodic refresh or unrelated `full_state` push before showing the intended selection change

#### Scenario: error-row-reselect-is-visible-immediately
- **GIVEN** the dashboard displays a change row whose status is `error` and `selected = false`
- **WHEN** the user re-selects that row for retry
- **THEN** the checkbox becomes visibly checked immediately
- **AND** the row continues to display error status treatment rather than changing to a non-error status prematurely

#### Scenario: failed-selection-toggle-rolls-back-ui
- **GIVEN** the dashboard has optimistically updated a row checkbox after a user toggle
- **AND** the server rejects or fails the toggle request
- **WHEN** the failure is observed by the dashboard
- **THEN** the dashboard restores the prior confirmed checkbox state
- **AND** the dashboard surfaces an error indication to the user

#### Scenario: bulk-selection-toggle-updates-visible-rows-immediately
- **GIVEN** the dashboard displays multiple change rows including previously unselected error rows
- **WHEN** the user invokes a bulk selection toggle
- **THEN** the visible row checkbox states update immediately to the intended post-toggle values
- **AND** the dashboard later reconciles to the server-confirmed values without leaving stale pre-toggle selections on screen
