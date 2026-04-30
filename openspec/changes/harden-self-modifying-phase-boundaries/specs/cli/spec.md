## ADDED Requirements

### Requirement: CLI/operator diagnostics distinguish primary diagnosis from cross-phase degradation

When a self-modifying control-plane change fails across multiple phases, CLI/operator-facing diagnostics SHALL distinguish the primary diagnosis from later cross-phase degradation.

The CLI MAY include supplemental warnings about persistence failure, retry exhaustion, or no-progress stall, but it MUST preserve the more specific earlier diagnosis when one has already been established.

#### Scenario: CLI preserves earlier diagnosis for self-modifying change
- **GIVEN** a self-modifying control-plane change first fails due to a concrete archive-feasibility mismatch
- **AND** later retries also encounter empty-progress stall protection
- **WHEN** the CLI reports the final outcome
- **THEN** the operator can still see the earlier archive-feasibility mismatch as the primary reason
- **AND** the later stall symptom is reported separately as secondary degradation
