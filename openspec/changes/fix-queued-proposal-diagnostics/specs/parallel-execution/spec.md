## ADDED Requirements

### Requirement: Invalid queued proposal metadata is a visible change-local error

Conflux SHALL reconcile an ordinary admitted proposal's strict metadata read/decode failure before any blocked-only short circuit. It SHALL publish the existing change-local Error and bounded actionable error detail, without global Error, fake dependency or reviewer provenance. The detail SHALL identify the repository-relative proposal path and correction/commit/explicit Retry action without echoing raw declarations or environment values. Current state projection SHALL not depend on diagnostic dedup caches.

#### Scenario: Invalid-only queue cannot silently wait
- **GIVEN** an admitted proposal has unreadable, wrong-typed or unclosed metadata in LF or CRLF
- **WHEN** the scheduler evaluates ordinary candidates
- **THEN** the target becomes change-local error with safe detail before analysis or Apply
- **AND** its client wait returns existing change_requires_action
- **AND** process mode and unrelated work are preserved

#### Scenario: Local repair requires explicit Retry
- **GIVEN** the target is errored for invalid proposal metadata
- **WHEN** only its base proposal is corrected and committed
- **THEN** the target does not automatically resume
- **WHEN** the operator explicitly retries it
- **THEN** fresh valid metadata permits ordinary dispatch once, and still-invalid metadata returns to error without execution

#### Scenario: Repeated observations and genuine dependency controls
- **GIVEN** one invalid proposal and one valid unrelated or dependency-waiting proposal
- **WHEN** repeated scheduler observations or a projection reconstruction occur
- **THEN** invalid current state remains observable without repeated diagnostic floods
- **AND** genuine dependency resolution and valid empty/no-frontmatter reader behavior are unchanged
