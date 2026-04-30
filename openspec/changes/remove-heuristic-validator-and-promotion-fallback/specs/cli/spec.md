## ADDED Requirements

### Requirement: Native OpenSpec validator must not infer proposal quality from free-text wording

The native OpenSpec validator SHALL validate proposal/task quality only from explicit, parseable structure such as declared metadata fields, verification ownership markers, or other machine-readable syntax defined by the canonical specs.

The validator MUST NOT classify proposal intent or implementation adequacy solely from keyword matches in free-text task or proposal prose.

#### Scenario: wording variation does not change validator outcome without structural change
- **GIVEN** two proposals have the same explicit metadata and verification markers
- **AND** they differ only in free-text phrasing or synonymous wording
- **WHEN** the native validator evaluates them
- **THEN** the validator returns the same structural validation outcome for both
- **AND** it does not emit different quality warnings based only on keyword wording

### Requirement: Canonical spec promotion must fail closed on malformed delta structure

When a spec delta cannot be parsed into the canonical requirement block structure required for promotion, canonicalization SHALL fail with a deterministic parse/promotion error.

The promotion engine MUST NOT rewrite malformed delta text into a best-effort canonical spec as a fallback.

#### Scenario: malformed delta does not fallback-rewrite into canonical text
- **GIVEN** a change spec delta lacks parseable requirement blocks for canonical promotion
- **WHEN** the promotion engine attempts canonicalization
- **THEN** the engine returns a deterministic parse or promotion error
- **AND** it does not rewrite section markers to synthesize a best-effort canonical spec
