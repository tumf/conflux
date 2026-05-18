## MODIFIED Requirements

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, or resume controls. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, MUST NOT label the row with the `NEW` badge, and MUST visibly present the row's terminal status as `rejected` in both Select and Running mode.

Rejected row discovery during local TUI auto-refresh MUST use the same captured repository root as active change discovery. It MUST NOT depend on ambient process current working directory after TUI startup.

<!-- Expected canonical result after archive: `tui-state` will state that rejected marker row discovery is repo-root based during local TUI refresh and still never produces NEW badges or queue intent. -->

#### Scenario: Rejected change is shown in TUI list

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the TUI refreshes its change list
- **THEN** `fix-auth` is displayed in the list
- **AND** its display status is `rejected`

#### Scenario: Rejected row cannot gain an execution mark

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user presses Space on that row
- **THEN** the row remains `selected = false`
- **AND** no x mark is shown for `fix-auth`
- **AND** the display status remains `rejected`

#### Scenario: Rejected row is ignored by queue-oriented actions

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user invokes queue or resume-oriented actions such as `@` or `F5`
- **THEN** `fix-auth` is not added to the execution queue
- **AND** no execution start is requested for `fix-auth`

#### Scenario: Select mode shows rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Select mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`
- **AND** the row does NOT show the `NEW` badge

#### Scenario: Running mode keeps rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Running mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

#### Scenario: Rejected row refresh uses captured repository root

- **GIVEN** local TUI mode started from repository root `/repo`
- **AND** the process current working directory later differs from `/repo`
- **AND** `/repo/openspec/changes/rejected-visible/proposal.md` exists
- **AND** `/repo/openspec/changes/rejected-visible/REJECTED.md` exists
- **WHEN** the TUI refreshes rejected marker rows
- **THEN** `rejected-visible` is displayed as a rejected row from `/repo/openspec/changes`
- **AND** the row does NOT show the `NEW` badge
- **AND** no queue or selection intent is created for `rejected-visible`
