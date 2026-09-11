## ADDED Requirements

### Requirement: Merged-row dismissal is immediate process-local presentation state

The local TUI SHALL let an operator immediately dismiss either the focused row whose current display status is exactly `merged` or all rows in the current Changes-list projection whose current display status is exactly `merged`. "Visible" means present in that projection regardless of scroll position. Dismissal MUST take effect from the `d` or `D` key press without opening a confirmation overlay or requiring a second key press.

Dismissed IDs MUST remain absent while refresh observes only retained terminal/merged presentation state for the remaining lifetime of that TUI process. If a later catalog refresh re-observes a dismissed ID as an active non-terminal change, the TUI MUST remove that ID from the dismissed set and render it normally. A new process MUST start without prior dismissal state.

Dismissal MUST NOT mutate repository files, OpenSpec archives, Git state, worktrees, reducer state, API/Web projections, logs, queue membership, execution marks, or lifecycle routing.

#### Scenario: operator immediately dismisses one merged row

- **GIVEN** the Changes-view cursor is on `alpha` with display status `merged`
- **AND** other merged and non-merged rows are visible
- **WHEN** the operator presses `d`
- **THEN** `alpha` is removed immediately from the local row projection
- **AND** every other row remains in its prior order and state
- **AND** no confirmation overlay opens
- **AND** no workflow or repository mutation is emitted

#### Scenario: operator immediately dismisses all projected merged rows

- **GIVEN** the Changes view contains merged rows `alpha` and `gamma` plus non-merged row `beta`
- **WHEN** the operator presses `D`
- **THEN** `alpha` and `gamma` are removed immediately
- **AND** `beta` remains unchanged
- **AND** no confirmation overlay opens

#### Scenario: bulk dismissal includes off-viewport projected rows

- **GIVEN** merged row `gamma` is in the current Changes-list projection but outside the terminal viewport
- **WHEN** the operator presses `D`
- **THEN** `gamma` is dismissed

#### Scenario: dismissed terminal row stays absent during the process

- **GIVEN** `alpha` was dismissed
- **WHEN** later catalog or reducer refreshes contain only retained terminal/merged state for `alpha`
- **THEN** the local TUI does not recreate or render `alpha`
- **AND** workflow state remains available to non-TUI owners unchanged

#### Scenario: active reuse of a dismissed ID becomes visible

- **GIVEN** `alpha` was dismissed as a merged row
- **WHEN** a later catalog refresh re-observes `alpha` as an active non-terminal change
- **THEN** `alpha` is removed from the dismissed-ID set
- **AND** the active row is rendered normally

#### Scenario: dismissal repairs local navigation state

- **GIVEN** dismissal removes the first, middle, last, all, or only visible row
- **WHEN** the local row projection is updated
- **THEN** cursor and list selection point to a surviving row when one exists
- **AND** selection is cleared when no row remains
- **AND** a selected-proposal log filter targeting a removed row is disabled without deleting buffered logs

#### Scenario: non-merged requests are inert

- **GIVEN** the focused row is not `merged` or no visible merged row exists
- **WHEN** the operator invokes the corresponding individual or bulk dismissal key
- **THEN** no confirmation opens
- **AND** no state changes
