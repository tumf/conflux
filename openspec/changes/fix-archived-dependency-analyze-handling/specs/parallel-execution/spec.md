## MODIFIED Requirements

### Requirement: archived dependency references have explicit scheduler and validation semantics

The system SHALL classify dependency targets referenced from active change metadata into at least four categories: queued, in-flight, archived, and missing.

Queued and in-flight dependency targets MAY participate in analyze ordering as dependency edges. Archived dependency targets MUST NOT be surfaced as generic JSON parse failures. During analyze dependency validation, archived dependency targets SHALL be treated as already satisfied/non-queued references and MUST NOT make the analyze result fail solely because they are absent from the queued order list. Missing dependency targets SHALL remain invalid dependency references with dedicated diagnostics.

#### Scenario: archived dependency reference is normalized during analyze

- **GIVEN** an active change references dependency `beta`
- **AND** `beta` exists only under `openspec/changes/archive/`
- **WHEN** analyze-order parsing evaluates a dependency edge to `beta`
- **THEN** the analyzer treats `beta` as an archived satisfied/non-queued reference
- **AND** the returned executable dependency graph does not require `beta` to be queued
- **AND** the outcome is not reported as generic invalid JSON or malformed analysis output

#### Scenario: missing dependency remains a true invalid reference

- **GIVEN** an active change references dependency `gamma`
- **AND** `gamma` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** analyze-order parsing evaluates a dependency edge to `gamma`
- **THEN** the analyzer returns a dedicated invalid dependency reference failure
- **AND** the diagnostics identify `gamma` as missing rather than archived
