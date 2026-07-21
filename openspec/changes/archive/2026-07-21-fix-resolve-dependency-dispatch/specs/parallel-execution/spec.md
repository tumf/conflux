## MODIFIED Requirements

### Requirement: Archived dependency references are explicitly classified

The system SHALL classify active proposal metadata dependency targets using repository-visible evidence that distinguishes queued, in-flight, resolving, archived, rejected, and missing targets.

Archived dependency references MUST NOT collapse into generic parse/json failures. Rejected dependency references MUST NOT collapse into generic missing dependency failures when `REJECTED.md` evidence exists.

Rejected and missing dependency targets SHALL remain fail-closed dispatch blockers. Archived dependency targets SHALL remain explicitly classified as archived, but archive evidence alone MUST NOT satisfy dependent dispatch. A dependent change whose dependency is resolving, awaiting resolve integration, or archived but not merged into the scheduler's effective dependency base MUST remain blocked until repository-visible merge evidence shows the dependency is merged into that effective dependency base. Resolve completion signaling without matching repository-visible integration evidence MUST NOT unblock the dependent. The effective dependency base SHALL be the branch or tree context Conflux uses as the accumulated integration result for dispatch decisions; in ordinary runs this MAY be the original branch, while stacked orchestration MUST use the repository-visible integration context that contains completed dependency merge/archive commits.

<!-- Expected canonical result after archive: resolving dependencies remain blocked until repository-visible integration evidence exists, while unrelated changes retain parallel dispatch eligibility. -->

#### Scenario: Resolving dependency blocks dependent dispatch

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** `beta` is in active resolve or resolve-wait state
- **AND** the effective dependency base does not contain repository-visible merge evidence for `beta`
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** apply is not started for `alpha`

#### Scenario: Resolve completion without integration evidence remains blocked

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** the resolve command for `beta` has completed
- **AND** the effective dependency base does not yet contain repository-visible merge evidence for `beta`
- **WHEN** scheduler reanalysis evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** resolve completion signaling alone does not satisfy the dependency

#### Scenario: Integrated resolved dependency unblocks dependent

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** resolve integration for `beta` is complete
- **AND** the effective dependency base contains repository-visible merge evidence for `beta`
- **WHEN** resolve completion triggers scheduler reanalysis
- **THEN** `alpha` becomes eligible for dispatch if no other blockers remain

#### Scenario: Unrelated work remains parallel during resolve

- **GIVEN** change `beta` is resolving
- **AND** queued change `gamma` does not depend on `beta`
- **AND** execution capacity is available
- **WHEN** scheduler dispatch selection evaluates `gamma`
- **THEN** `gamma` remains eligible for dispatch
- **AND** the active resolve does not act as a global scheduler barrier

#### Scenario: Dependency evidence failure is fail-closed

- **GIVEN** queued change `alpha` declares dependency `beta`
- **AND** Conflux cannot determine whether `beta` is merged into the effective dependency base
- **WHEN** scheduler dispatch selection evaluates `alpha`
- **THEN** `alpha` remains dependency-blocked
- **AND** apply is not started for `alpha`
