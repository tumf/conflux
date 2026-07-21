## ADDED Requirements

### Requirement: Acceptance finding reconciliation uses stable identity and monotonic completion

Serial and parallel runtime MUST reconcile repository-fixable acceptance findings by stable identity rather than exact human-readable text. Explicit finding codes MUST be preferred when present. When a code is absent, runtime MUST generate a deterministic fallback identity from normalized structural finding fields and MUST NOT require summary or evidence text to remain unchanged.

A completed runtime-owned finding MUST remain completed during apply hydration and reconciliation. Runtime MAY reopen it only while ingesting a new acceptance FAIL payload that explicitly reports the same identity. Serial and parallel execution MUST apply equivalent identity and completion transition rules.

#### Scenario: partial completion survives apply reconciliation

- **GIVEN** a current acceptance follow-up contains multiple findings
- **AND** apply completed one finding while others remain unchecked
- **AND** remediation evidence or human-readable detail changed
- **WHEN** runtime hydrates or reconciles follow-up state during apply
- **THEN** the completed finding remains checked
- **AND** the remaining findings retain their prior state

#### Scenario: latest FAIL explicitly reopens a completed identity

- **GIVEN** a finding is completed in the current follow-up
- **WHEN** a later acceptance FAIL reports the same stable identity with current repository evidence
- **THEN** runtime reopens that finding as unchecked
- **AND** changed summary or evidence does not create a duplicate identity

#### Scenario: missing reviewer code uses runtime fallback identity

- **GIVEN** an acceptance finding has no explicit stable code
- **WHEN** runtime normalizes the finding in serial or parallel execution
- **THEN** both modes derive the same identity from normalized structural fields
- **AND** prose-only changes do not change the identity
- **AND** a distinct rule or repository location does not collide with it

#### Scenario: reconciliation cannot implicitly reopen completion

- **GIVEN** a completed finding exists
- **WHEN** runtime performs any follow-up update outside ingestion of a new FAIL payload
- **THEN** the update cannot transition the finding to unchecked
