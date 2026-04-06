## MODIFIED Requirements

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL treat `openspec/changes/<change_id>/REJECTED.md` as the durable rejection marker and SHALL exclude marker-bearing changes from the active listing returned by `list_changes_native()`.

When a previously rejected change reappears in the active listing because its `REJECTED.md` marker has been removed from the base branch, the runtime SHALL clear the in-memory `Rejected` terminal state for that change and restore it to the default non-terminal state (`terminal = None`, `activity = Idle`, `wait_state = None`, `queue_intent = NotQueued`). A reactivated change SHALL be eligible for `AddToQueue` and SHALL display as `not queued` after refresh.

#### Scenario: Rejected marker excludes change from active list

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** `list_changes_native()` is called
- **THEN** `fix-auth` is NOT included in the returned change list

#### Scenario: Non-rejected change with proposal is included

- **GIVEN** `openspec/changes/add-feature/proposal.md` exists
- **AND** `openspec/changes/add-feature/REJECTED.md` does NOT exist
- **WHEN** `list_changes_native()` is called
- **THEN** `add-feature` IS included in the returned change list

#### Scenario: Removal of REJECTED marker reactivates change in reducer

- **GIVEN** change `fix-auth` was previously rejected and the runtime holds `TerminalState::Rejected` for it
- **AND** the user deletes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** `ChangesRefreshed` fires with `fix-auth` present in the active change list
- **THEN** the runtime clears `TerminalState::Rejected` for `fix-auth`
- **AND** the display status for `fix-auth` becomes `not queued`
- **AND** `AddToQueue("fix-auth")` succeeds (not NoOp)

#### Scenario: Reactivated change can be queued again

- **GIVEN** change `fix-auth` has been reactivated after `REJECTED.md` removal and refresh
- **WHEN** the user queues `fix-auth` via `AddToQueue`
- **THEN** the display status becomes `queued`
- **AND** the change is eligible for execution dispatch

#### Scenario: Marker still present keeps change excluded

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` still exists on the base branch
- **WHEN** `ChangesRefreshed` fires
- **THEN** `fix-auth` is NOT in the active change list
- **AND** the runtime does NOT clear `TerminalState::Rejected`
