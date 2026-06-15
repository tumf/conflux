## MODIFIED Requirements

### Requirement: Archived dependency references are explicitly classified

The system SHALL classify active proposal metadata dependency targets using repository-visible evidence that distinguishes queued, in-flight, archived, rejected, and missing targets.

Archived dependency references MUST NOT collapse into generic parse/json failures. Rejected dependency references MUST NOT collapse into generic missing dependency failures when `REJECTED.md` evidence exists.

Rejected and missing dependency targets SHALL remain fail-closed dispatch blockers. Archived dependency targets SHALL remain explicitly classified as archived, but archive evidence alone MUST NOT satisfy dependent dispatch. A dependent change whose dependency is archived but not merged into the scheduler's effective dependency base MUST remain blocked until repository-visible merge evidence shows the dependency is merged into that effective dependency base. The effective dependency base SHALL be the branch or tree context Conflux uses as the accumulated integration result for dispatch decisions; in ordinary runs this MAY be the original branch, while stacked orchestration MUST use the repository-visible integration context that contains completed dependency merge/archive commits.

<!-- Expected canonical result after archive: archived dependencies remain blocked until merge evidence exists, but the merge evidence is checked against the scheduler's effective dependency base instead of implicitly requiring the startup branch in all orchestration modes. -->

#### Scenario: Archived dependency is surfaced with dedicated diagnostics

- **GIVEN** active change `alpha` declares dependency `beta`
- **AND** `beta` exists only under `openspec/changes/archive/`
- **WHEN** analyze or validate checks the dependency target
- **THEN** diagnostics classify the target as an archived dependency reference
- **AND** diagnostics are not displayed as generic `Analysis returned invalid JSON`

#### Scenario: Archived dependency is not dispatch-satisfied until merged

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** `beta` exists under `openspec/changes/archive/`
- **AND** effective dependency base merge evidence does not show `beta` as merged
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** apply is not started for `alpha`

#### Scenario: Archived dependency becomes satisfied after merge

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** `beta` exists under `openspec/changes/archive/`
- **AND** effective dependency base merge evidence shows `beta` is merged
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` becomes eligible for dispatch if no other unresolved dependency blockers remain

#### Scenario: Stacked orchestration uses effective integration base

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** `beta` exists under `openspec/changes/archive/`
- **AND** the original startup branch does not yet contain the archived `beta` merge
- **AND** the scheduler's effective integration base does contain the archived `beta` merge
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` becomes eligible for dispatch if no other unresolved dependency blockers remain
- **AND** `alpha` is not kept blocked solely because the original startup branch lacks the dependency merge

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
