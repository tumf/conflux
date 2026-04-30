## ADDED Requirements

### Requirement: Self-modifying control-plane changes declare cross-phase hardening expectations

When a change modifies Conflux control-plane contracts such as prompts, verdict parsing, follow-up routing, runtime state semantics, or archive promotion behavior, the system SHALL treat that change as a self-modifying control-plane change for verification and execution-hardening purposes.

Such changes SHALL define verification coverage that spans all affected phases rather than validating each phase in isolation.

At minimum, the verification plan SHALL account for acceptance interpretation, follow-up persistence/routing, archive precondition feasibility, and the final user-visible diagnosis taxonomy.

#### Scenario: self-modifying change declares cross-phase verification
- **GIVEN** a change updates both acceptance contract and archive-related behavior
- **WHEN** the proposal is authored and validated
- **THEN** the change includes verification that spans acceptance, routing, and archive phases
- **AND** the change is not treated as a phase-local implementation-only change

### Requirement: Primary diagnosis survives secondary degradation across phase boundaries

When a phase-specific primary diagnosis has already been established, later secondary degradation in another phase MUST NOT replace that primary diagnosis with a less specific generic error.

Secondary degradation MAY be reported as supplemental context, warning, or retry metadata, but the user-visible and machine-readable outcome SHALL preserve the original primary diagnosis whenever it remains valid.

#### Scenario: archive no-op stall does not erase earlier archive prerequisite diagnosis
- **GIVEN** the system has already identified a concrete archive prerequisite failure for a change
- **AND** a later archive attempt makes no repository progress and hits empty-WIP stall protection
- **WHEN** the runtime reports the failure
- **THEN** the earlier archive prerequisite failure remains the primary diagnosis
- **AND** the stall condition is reported only as supplemental context or follow-on symptom

### Requirement: Self-modifying changes run archive-feasibility checks before archive stall loops

When a self-modifying control-plane change can affect canonical spec promotion or archive feasibility, the system SHALL run a pre-archive feasibility check before relying on archive retry/stall loops.

The feasibility check SHALL detect at least heading/promotion mismatches, canonical no-op archive conditions, or equivalent situations where archive cannot make progress without additional repository edits.

#### Scenario: self-modifying change detects heading mismatch before archive stall
- **GIVEN** a self-modifying change has a spec delta whose headings cannot promote cleanly into the canonical spec
- **WHEN** the archive preflight runs
- **THEN** the runtime reports the promotion mismatch before entering repeated archive no-op attempts
- **AND** the change does not first surface only as a generic archive empty-WIP stall
