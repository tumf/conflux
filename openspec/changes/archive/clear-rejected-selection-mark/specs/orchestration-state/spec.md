## MODIFIED Requirements

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL continue to treat `openspec/changes/<change_id>/REJECTED.md` as the durable rejection marker and exclude marker-bearing changes from the execution-oriented active listing returned by `list_changes_native()`.

In addition, when a change transitions into `TerminalState::Rejected`, any frontend-visible execution mark associated with that change SHALL be cleared so the rejected change is not represented as an execution candidate. This clear SHALL restore the UI-visible selection state for that change to `selected = false` while preserving the `rejected` terminal display status.

This execution-mark clear applies only to the rejected change. It MUST NOT clear execution marks for unrelated changes.

#### Scenario: Rejected transition clears execution mark for that change only

- **GIVEN** change `fix-auth` is execution-marked (`selected = true`)
- **AND** another change `add-feature` is also execution-marked
- **WHEN** `fix-auth` transitions into `TerminalState::Rejected`
- **THEN** `fix-auth` is represented as `selected = false`
- **AND** the display status for `fix-auth` remains `rejected`
- **AND** `add-feature` keeps its existing execution mark

#### Scenario: Reactivated rejected change stays unselected after marker removal

- **GIVEN** change `fix-auth` was previously rejected and its execution mark was cleared
- **AND** the user deletes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** `ChangesRefreshed` fires with `fix-auth` present in the active change list
- **THEN** the runtime clears `TerminalState::Rejected` for `fix-auth`
- **AND** the display status for `fix-auth` becomes `not queued`
- **AND** `fix-auth` remains `selected = false` until the user explicitly marks it again
