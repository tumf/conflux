## ADDED Requirements

### Requirement: Confirmed merged-row dismissal is process-local presentation state

The local TUI SHALL let an operator dismiss either the focused row whose current display status is exactly `merged` or all rows in the current Changes-list projection whose current display status is exactly `merged`. "Visible" means present in that projection regardless of scroll position. Each action MUST require a typed confirmation before rows are removed. Individual confirmation MUST bind the exact change ID. Bulk confirmation MUST bind the exact projected target IDs when the action was requested.

Confirmation MUST recheck each bound row and remove only IDs that still have display status `merged`. If no bound ID remains eligible, confirmation MUST close without changing rows, dismissal state, or logs. Dismissed IDs MUST remain absent while refresh observes only retained terminal/merged presentation state for the remaining lifetime of that TUI process. If a later catalog refresh re-observes a dismissed ID as an active non-terminal change, the TUI MUST remove that ID from the dismissed set and render it normally. A new process MUST start without prior dismissal state. Confirmation keys MUST follow the existing case-insensitive modal pattern: `y`/`Y` confirms, while `n`/`N` or `Esc` cancels.

Dismissal MUST NOT mutate repository files, OpenSpec archives, Git state, worktrees, reducer state, API/Web projections, logs, queue membership, execution marks, or lifecycle routing.

#### Scenario: operator confirms one merged row

- **GIVEN** the Changes-view cursor is on `alpha` with display status `merged`
- **AND** other merged and non-merged rows are visible
- **WHEN** the operator requests individual dismissal and confirms it
- **THEN** `alpha` is removed from the local row projection
- **AND** every other row remains in its prior order and state
- **AND** no workflow or repository mutation is emitted

#### Scenario: operator confirms all visible merged rows

- **GIVEN** the Changes view contains merged rows `alpha` and `gamma` plus non-merged row `beta`
- **WHEN** the operator requests bulk dismissal and confirms it
- **THEN** `alpha` and `gamma` are removed
- **AND** `beta` remains unchanged
- **AND** the confirmation target cannot grow to include a row that became merged after the overlay opened

#### Scenario: cancellation is mutation free

- **GIVEN** an individual or bulk dismissal confirmation is open
- **WHEN** the operator presses `N` or `Esc`
- **THEN** the overlay closes
- **AND** no row or dismissal state changes

#### Scenario: stale confirmation cannot hide a changed row

- **GIVEN** a dismissal confirmation bound `alpha` while it was `merged`
- **AND** `alpha` no longer has display status `merged` before confirmation
- **WHEN** the operator confirms
- **THEN** `alpha` remains visible
- **AND** it is not added to the dismissed-ID set

#### Scenario: dismissed terminal row stays absent during the process

- **GIVEN** `alpha` was confirmed and dismissed
- **WHEN** later catalog or reducer refreshes contain only retained terminal/merged state for `alpha`
- **THEN** the local TUI does not recreate or render `alpha`
- **AND** workflow state remains available to non-TUI owners unchanged

#### Scenario: active reuse of a dismissed ID becomes visible

- **GIVEN** `alpha` was confirmed and dismissed as a merged row
- **WHEN** a later catalog refresh re-observes `alpha` as an active non-terminal change
- **THEN** `alpha` is removed from the dismissed-ID set
- **AND** the active row is rendered normally

#### Scenario: bulk dismissal includes off-viewport projected rows

- **GIVEN** merged row `gamma` is in the current Changes-list projection but outside the terminal viewport
- **WHEN** the operator requests bulk dismissal and confirms it
- **THEN** `gamma` is included in the bound target IDs and dismissed if it is still merged

#### Scenario: no bound row remains eligible at confirmation

- **GIVEN** every row bound by a dismissal confirmation has ceased to be `merged`
- **WHEN** the operator confirms with `y` or `Y`
- **THEN** the overlay closes
- **AND** rows, dismissal state, and logs remain unchanged

#### Scenario: dismissal repairs local navigation state

- **GIVEN** confirmed dismissal removes the first, middle, last, all, or only visible row
- **WHEN** the local row projection is updated
- **THEN** cursor and list selection point to a surviving row when one exists
- **AND** selection is cleared when no row remains
- **AND** a selected-proposal log filter targeting a removed row is disabled without deleting buffered logs

#### Scenario: non-merged requests are inert

- **GIVEN** the focused row is not `merged` or no visible merged row exists
- **WHEN** the operator invokes the corresponding individual or bulk dismissal key
- **THEN** no confirmation opens
- **AND** no state changes

#### Scenario: confirmation owns input

- **GIVEN** a merged-row dismissal confirmation is open over any execution mode
- **WHEN** the operator presses a row navigation, edit, mark, run, worktree, or unrelated action key
- **THEN** the underlying view does not act
- **AND** only `Y`, `N`, or `Esc` resolves the confirmation
