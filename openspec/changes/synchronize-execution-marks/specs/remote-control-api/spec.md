## ADDED Requirements

### Requirement: Event mark changes share the authoritative state revision

When a typed failure, rejection, rejected refresh, or dequeue event revokes an execution mark, `/api/v2` MUST publish the reconciled `execution_marked` value in the same authoritative state revision as that event's reducer/frontend transition. The projection MUST read the shared `ExecutionMarkStore` after event reconciliation and MUST NOT wait for an unrelated refresh or create a second mark-only revision.

Duplicate or late delivery that changes neither reducer state nor execution marks MUST NOT advance another state revision. Event reconciliation MUST preserve unrelated marks in the same snapshot.

#### Scenario: failure event and cleared mark are coherent

- **GIVEN** `alpha` and `beta` are marked in the authoritative operator snapshot
- **WHEN** a typed event transitions `alpha` into change-level Error
- **THEN** the event envelope's state revision identifies a snapshot where `alpha.execution_marked` is false
- **AND** `beta.execution_marked` remains true
- **AND** no intermediate revision exposes Error with the stale `alpha` mark

#### Scenario: rejected refresh clears mark in its refresh revision

- **GIVEN** a marked active change becomes a rejected marker in a catalog refresh
- **WHEN** the authoritative refresh dispatch is projected
- **THEN** the refresh revision reports the row as rejected and unmarked together
- **AND** the client does not need prior-event replay or log parsing to reconcile the decision

#### Scenario: duplicate revocation is revision-idempotent

- **GIVEN** the target mark is already false after a revoking event
- **WHEN** duplicate delivery produces no reducer or mark change
- **THEN** `/api/v2` does not advance another state revision
- **AND** unrelated execution marks remain unchanged

#### Scenario: process stop retains marked resume targets

- **GIVEN** the snapshot contains execution-marked changes
- **WHEN** a process-level `Stopped` transition is published
- **THEN** the stopped revision retains those `execution_marked` values
- **AND** queue intent and reducer stop reconciliation remain separate from mark ownership
