## MODIFIED Requirements

### Requirement: Reducer Input Precedence and Idempotency

Workspace observations SHALL NOT regress a change from terminal `Merged` back to `MergeWait` when the change has already been integrated into the base branch, including fast-forward integration.

#### Scenario: Archived workspace observation does not regress fast-forward merged change

- **GIVEN** a change has already reached terminal `Merged`
- **AND** the integration happened via fast-forward rather than a merge commit
- **WHEN** a later `ChangesRefreshed` event observes the workspace as archived
- **THEN** the reducer keeps the terminal state as `Merged`
- **AND** the derived display status does not regress to `merge wait`
