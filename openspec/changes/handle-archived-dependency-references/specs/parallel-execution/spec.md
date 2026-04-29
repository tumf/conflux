## ADDED Requirements

### Requirement: archived dependency references have explicit scheduler and validation semantics

The system SHALL classify dependency targets referenced from active change metadata into at least four categories: queued, in-flight, archived, and missing.

Queued and in-flight dependency targets MAY participate in analyze ordering as dependency edges. Archived dependency targets MUST NOT be surfaced as generic JSON parse failures. The runtime and validation layers MUST either treat archived dependencies as explicitly satisfied/non-queued references or reject them with dedicated archived-dependency diagnostics, but in either case they MUST distinguish this condition from malformed JSON and from truly missing change IDs.

#### Scenario: archived dependency reference is not reported as invalid JSON

- **GIVEN** an active change references dependency `beta`
- **AND** `beta` exists only under `openspec/changes/archive/`
- **WHEN** dependency validation or analyze-order parsing evaluates the reference
- **THEN** the reported outcome identifies the archived-dependency condition explicitly
- **AND** user-visible diagnostics do not collapse the condition into generic `Analysis returned invalid JSON`

#### Scenario: missing dependency remains a true invalid reference

- **GIVEN** an active change references dependency `gamma`
- **AND** `gamma` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** dependency validation evaluates the reference
- **THEN** the system reports a dedicated invalid dependency reference failure
- **AND** the diagnostics include enough context to distinguish it from the archived-dependency case
