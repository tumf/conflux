## ADDED Requirements

### Requirement: Archive promotion MUST merge deltas by requirement identity

Archive promotion MUST parse canonical and change-local spec files into full requirement blocks keyed by normalized `### Requirement:` headings. It MUST append `ADDED` requirements, replace matching `MODIFIED` requirements, and delete matching `REMOVED` requirements before writing canonical specs.

#### Scenario: MODIFIED requirement replaces canonical content
- **GIVEN** a canonical spec contains `### Requirement: Reliable Archive Tracking`
- **AND** the change delta contains `## MODIFIED Requirements` with a full updated `### Requirement: Reliable Archive Tracking` block
- **WHEN** archive promotion runs
- **THEN** the canonical requirement block is replaced with the updated block
- **AND** stale scenarios from the previous block are not retained outside the replacement

#### Scenario: REMOVED requirement deletes canonical content
- **GIVEN** a canonical spec contains `### Requirement: Legacy Archive Behavior`
- **AND** the change delta contains `## REMOVED Requirements` for `Legacy Archive Behavior`
- **WHEN** archive promotion runs
- **THEN** the canonical requirement block is removed
- **AND** the remaining canonical spec stays well-formed

### Requirement: Archive promotion MUST reject silent no-op updates

Archive promotion MUST fail instead of reporting success when a `MODIFIED` or `REMOVED` delta targets a requirement that does not exist in the canonical spec, or when promotion completes without changing the touched canonical spec content.

#### Scenario: Missing MODIFIED target fails promotion
- **GIVEN** a change delta contains `## MODIFIED Requirements` for `### Requirement: Missing Requirement`
- **AND** the canonical spec does not contain `Missing Requirement`
- **WHEN** archive promotion runs
- **THEN** promotion fails with a missing-target error
- **AND** the canonical spec file is not rewritten

#### Scenario: No canonical diff fails promotion
- **GIVEN** a change delta is promoted against a canonical spec
- **AND** the promotion result would leave the canonical spec byte-for-byte unchanged
- **WHEN** archive promotion runs for a touched spec
- **THEN** promotion fails with a no-op archive error
- **AND** archive does not report the spec as successfully updated

### Requirement: Archive guidance MUST verify canonical spec diffs

Archive guidance MUST instruct operators to verify the resulting canonical diff for each touched `openspec/specs/**` file instead of relying only on archive command exit status or helper summary output.

#### Scenario: Archive review checks canonical diff
- **GIVEN** an archive command reports that specs were updated
- **WHEN** the operator performs the archive verification checklist
- **THEN** the checklist requires reviewing the canonical diff for each touched spec file
- **AND** the checklist does not treat `Specs updated: [...]` as sufficient evidence by itself
