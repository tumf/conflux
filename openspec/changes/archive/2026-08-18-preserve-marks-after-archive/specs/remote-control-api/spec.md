## MODIFIED Requirements

### Requirement: Event mark changes share the authoritative state revision

When an existing typed failure, rejection, rejected or parallel-ineligible refresh, dequeue, target-scoped stop, or first `on_merged` hook-recovery event revokes an execution mark, `/api/v2` MUST publish the reconciled `execution_marked` value in the same authoritative state revision as that event's reducer/frontend transition. A `ChangeArchived` transition MUST preserve its target mark in that archive revision. The projection MUST read the shared `ExecutionMarkStore` after pre/post event reconciliation and MUST NOT wait for an unrelated refresh or create a second mark-only revision.

Duplicate or late delivery that changes neither reducer state nor execution marks MUST NOT advance another state revision. Event reconciliation MUST preserve unrelated marks in the same snapshot. A duplicate failure delivered after an explicit re-mark MUST preserve that fresh mark when it creates no new reducer transition. Process-level Stopped transitions MUST retain marked resume targets.

#### Scenario: failure event and cleared mark are coherent

- **GIVEN** `alpha` and `beta` are marked in the authoritative operator snapshot
- **WHEN** a typed event transitions `alpha` into change-level Error
- **THEN** the event revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true
- **AND** no intermediate revision exposes Error with the stale mark

#### Scenario: rejected or ineligible refresh clears mark in its refresh revision

- **GIVEN** `alpha` and `beta` are marked active changes
- **WHEN** one authoritative refresh introduces `alpha` as rejected or parallel-ineligible
- **THEN** that refresh revision reports `alpha.execution_marked` as false
- **AND** `beta.execution_marked` remains true

#### Scenario: archive transition preserves target and unrelated marks

- **GIVEN** `alpha` and `beta` are marked
- **WHEN** the first effective `ChangeArchived(alpha)` transition is projected
- **THEN** that archive revision reports `alpha.execution_marked` as true
- **AND** `beta.execution_marked` remains true
- **AND** later merged or pushed projection continues to retain both marks

#### Scenario: on_merged recovery and cleared mark are coherent

- **GIVEN** a marked change is in active merge handling
- **WHEN** its first `on_merged` hook failure enters reducer merge-wait recovery
- **THEN** that event revision reports the recovery row and `execution_marked: false` together

#### Scenario: duplicate revocation is revision-idempotent

- **GIVEN** the target mark is already false after a revoking event
- **WHEN** duplicate delivery produces no reducer or mark change
- **THEN** `/api/v2` does not advance another state revision
- **AND** unrelated marks remain unchanged

#### Scenario: duplicate failure preserves a fresh re-mark

- **GIVEN** a revoking event cleared a target and an operator explicitly re-marked its non-terminal steady recovery row
- **WHEN** the same failure event is delivered again without a reducer transition
- **THEN** the event revision retains `execution_marked: true`
- **AND** no mark-only correction revision is needed

#### Scenario: process stop retains marked resume targets

- **GIVEN** the snapshot contains execution-marked changes
- **WHEN** a process-level `Stopped` transition is published
- **THEN** the stopped revision retains those `execution_marked` values
- **AND** queue intent and reducer stop reconciliation remain separate from mark ownership

<!-- Expected canonical result after archive: `remote-control-api` will publish archive, merge, and push transitions without revoking lifecycle-independent execution marks. -->
