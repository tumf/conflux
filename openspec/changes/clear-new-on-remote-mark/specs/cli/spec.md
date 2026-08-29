## MODIFIED Requirements

### Requirement: New Change Detection

When auto-refresh detects new changes, they SHALL be displayed appropriately. The `NEW` badge is ephemeral frontend attention state. A settled operator execution-mark interaction SHALL acknowledge that attention for its target regardless of whether the interaction originated from the local TUI, `/api/v2`, `cflx client`, or MCP. Passive synchronization and lifecycle/system reconciliation SHALL NOT acknowledge it.

#### Scenario: New change detection
- **WHEN** auto-refresh detects a new change
- **THEN** the new change is added to the change list
- **AND** a "NEW" badge is displayed
- **AND** "Discovered new change: <id>" is logged

#### Scenario: Default state of new changes
- **WHEN** a new change is detected
- **THEN** it is unselected by default (`[ ]`)
- **AND** the new count in the footer is updated

#### Scenario: NEW badge display
- **WHEN** a change is newly detected
- **THEN** a "NEW" badge is displayed next to the change name
- **AND** the badge is displayed in a visually prominent color

#### Scenario: NEW badge cleared on local selection
- **GIVEN** a change has a NEW badge
- **WHEN** the user toggles its execution mark from the TUI
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

#### Scenario: NEW badge cleared on remote execution-mark interaction
- **GIVEN** a live TUI displays a change with a NEW badge
- **WHEN** an operator execution-mark mutation for that change settles through `/api/v2`, `cflx client`, or MCP
- **THEN** the TUI removes that change's NEW badge
- **AND** the new count in the footer is decremented
- **AND** unrelated NEW changes remain unchanged
- **AND** no queue, retry, lifecycle, or admission mutation is synthesized

#### Scenario: Passive mark synchronization does not clear NEW
- **GIVEN** a live TUI displays a change with a NEW badge
- **WHEN** the TUI passively synchronizes the shared execution-mark store after refresh or lifecycle reconciliation
- **AND** no operator execution-mark interaction for that change settled
- **THEN** the NEW badge remains
- **AND** the new count is unchanged

#### Scenario: Unchanged remote request does not acknowledge NEW
- **GIVEN** a live TUI displays a change with a NEW badge
- **AND** the authoritative execution mark already equals the requested value
- **WHEN** a remote execution-mark request settles as unchanged without creating a new operator mutation
- **THEN** the NEW badge remains
- **AND** the new count is unchanged

<!-- Expected canonical result after archive: New Change Detection defines NEW acknowledgement consistently for local and remote operator mark mutations while excluding passive and system-origin synchronization. -->
