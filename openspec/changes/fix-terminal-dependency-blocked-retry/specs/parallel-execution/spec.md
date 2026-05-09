## MODIFIED Requirements

### Requirement: Archived dependency references are explicitly classified

The system SHALL classify active proposal metadata dependency targets using repository-visible evidence that distinguishes queued, in-flight, archived, rejected, and missing targets.

Archived dependency references MUST NOT collapse into generic parse/json failures. Rejected dependency references MUST NOT collapse into generic missing dependency failures when `REJECTED.md` evidence exists.

Rejected and missing dependency targets SHALL remain fail-closed dispatch blockers, while archived dependency targets SHALL be treated as satisfied.

#### Scenario: Archived dependency is surfaced with dedicated diagnostics

- **GIVEN** active change `alpha` declares dependency `beta`
- **AND** `beta` exists only under `openspec/changes/archive/`
- **WHEN** analyze or validate checks the dependency target
- **THEN** diagnostics classify the target as an archived dependency reference
- **AND** diagnostics are not displayed as generic `Analysis returned invalid JSON`

#### Scenario: Missing dependency remains an invalid dependency failure

- **GIVEN** active change `alpha` declares dependency `gamma`
- **AND** `gamma` is not queued, in-flight, archived, or rejected
- **WHEN** analyze or validate checks the dependency target
- **THEN** diagnostics classify the target as missing
- **AND** the message is distinguishable from archived and rejected dependency cases

#### Scenario: Rejected dependency remains a terminal dispatch blocker

- **GIVEN** active change `alpha` declares dependency `beta`
- **AND** `openspec/changes/beta/proposal.md` exists
- **AND** `openspec/changes/beta/REJECTED.md` exists
- **WHEN** analyze or scheduler dispatch checks the dependency target
- **THEN** diagnostics classify the target as rejected
- **AND** `alpha` is not dispatched
- **AND** the message is distinguishable from a missing dependency

### Requirement: Dependency-blocked diagnostics are stable and non-spamming

The scheduler SHALL preserve dependency-blocked state for queued changes that cannot dispatch, but it MUST NOT repeatedly emit identical operator-visible blocked/error diagnostics while the blocked change has the same repository-visible dependency blocker signature.

A blocker signature SHALL include at least the blocked change id, dependency ids, and dependency target classes. When the signature changes, the scheduler SHALL emit a fresh diagnostic and re-evaluate dispatch using the updated dependency evidence.

#### Scenario: Repeated rejected dependency blocker does not spam logs

- **GIVEN** queued change `alpha` depends on rejected dependency `beta`
- **AND** the scheduler has already emitted an operator-visible diagnostic for blocker signature `alpha -> beta [rejected]`
- **WHEN** later scheduler loops observe the same blocker signature
- **THEN** `alpha` remains dependency-blocked
- **AND** no duplicate operator-visible warn/error diagnostic for the same signature is appended

#### Scenario: Changed blocker signature emits a fresh diagnostic

- **GIVEN** queued change `alpha` was previously blocked by dependency `beta [missing]`
- **WHEN** repository-visible evidence changes so `beta` is now `rejected`
- **THEN** the scheduler emits a fresh diagnostic for `beta [rejected]`
- **AND** dispatch remains blocked

#### Scenario: Archived blocker re-evaluates dispatch eligibility

- **GIVEN** queued change `alpha` was previously blocked by dependency `beta [queued]`
- **WHEN** repository-visible evidence changes so `beta` is archived
- **THEN** the scheduler treats `beta` as satisfied
- **AND** `alpha` becomes eligible for dispatch if no other unresolved dependency blockers remain
