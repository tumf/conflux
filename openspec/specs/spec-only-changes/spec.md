## Requirements

### Requirement: Proposal tooling MUST classify change type

Proposal tooling MUST classify each change as `spec-only`, `implementation`, or `hybrid`. The classification MUST be explicit in proposal metadata so proposal scaffolding, validation, and downstream review can apply the correct rules.

#### Scenario: Spec-only proposal declares its type
- **GIVEN** a proposal changes only OpenSpec proposal/spec content and does not introduce runtime code work
- **WHEN** the proposal is scaffolded or validated
- **THEN** the proposal metadata records `Change Type: spec-only`
- **AND** validation rejects the proposal if the change type is missing or outside the supported set

### Requirement: Spec-only proposals MUST declare canonical archive expectations

When a proposal is classified as `spec-only`, it MUST use specification-focused task sections and MUST describe the expected canonical result of each spec delta so archive-readiness can be checked before promotion.

#### Scenario: Spec-only proposal uses specification tasks and canonical expectations
- **GIVEN** a proposal is classified as `spec-only`
- **WHEN** the proposal authoring workflow produces `tasks.md` and delta specs
- **THEN** `tasks.md` uses a `Specification Tasks` section instead of `Implementation Tasks`
- **AND** each spec delta includes a short note describing how the canonical spec should change after archive

### Requirement: Proposal tooling MUST warn on high-risk spec-only deltas

Proposal tooling MUST emit an archive-risk warning when a `spec-only` proposal depends primarily on `MODIFIED` or `REMOVED` deltas, because those deltas require successful canonical promotion instead of simple append behavior.

#### Scenario: MODIFIED-only spec-only proposal gets archive-risk warning
- **GIVEN** a proposal is classified as `spec-only`
- **AND** its deltas are `MODIFIED` and `REMOVED` only
- **WHEN** the proposal is scaffolded or validated
- **THEN** the workflow emits an archive-risk warning
- **AND** the warning instructs the operator to verify canonical promotion behavior before acceptance
