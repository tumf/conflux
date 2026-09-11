## ADDED Requirements

### Requirement: Merged-row dismissal hints are context-aware

The local TUI Changes panel SHALL render `d: dismiss` only when the focused row's current display status is exactly `merged`. It SHALL render `D: dismiss all merged` whenever at least one row in the current Changes-list projection has display status exactly `merged`, regardless of the focused row's status or scroll position. In this requirement, "visible" means present in the current Changes-list projection, not limited to the terminal viewport. The TUI SHALL NOT advertise either action when its corresponding target set is empty.

The hints SHALL use the existing Changes-panel title composition, text styling, border, and clipping behavior. They MUST remain distinct from the Worktrees-view `d`/`D` delete action and MUST NOT alter existing mark, run, resolve, kill, edit, log, QR, or app-level controls.

#### Scenario: focused merged row advertises individual dismissal

- **GIVEN** the cursor is on a visible `merged` row in the Changes view
- **WHEN** key hints are rendered
- **THEN** the Changes panel shows `d: dismiss`

#### Scenario: any visible merged row advertises bulk dismissal

- **GIVEN** at least one visible row has status `merged`
- **WHEN** key hints are rendered
- **THEN** the Changes panel shows `D: dismiss all merged`
- **AND** the hint remains visible when the cursor is on another status

#### Scenario: no merged target omits dismissal hints

- **GIVEN** no visible row has status `merged`
- **WHEN** key hints are rendered
- **THEN** neither merged-row dismissal hint is shown

#### Scenario: running mode preserves existing controls

- **GIVEN** the TUI is in Running mode with at least one visible `merged` row
- **WHEN** dismissal hints are rendered
- **THEN** the same target-dependent hints are shown as in Select mode
- **AND** existing app-level run and stop controls are unchanged

#### Scenario: worktree deletion remains distinct

- **GIVEN** the TUI is in the Worktrees view
- **WHEN** the operator uses `d` or `D`
- **THEN** the existing worktree-delete confirmation behavior remains authoritative
- **AND** no merged-row dismissal action is advertised or opened
