## MODIFIED Requirements

### Requirement: archived dependency references have explicit scheduler and validation semantics

Archived, active, queued, in-flight, missing, and rejected dependency targets MUST remain explicitly classified during analysis and scheduler dispatch. Fallback analysis after an LLM dependency-analysis failure MUST remain metadata-dependency-only rather than dependency-free. When fallback succeeds, the failed LLM attempt is a degraded analysis diagnostic, not a terminal workflow error.

#### Scenario: fallback analysis preserves metadata dependency

- **GIVEN** `route` has proposal metadata dependency `policy`
- **AND** LLM analysis fails or is disabled
- **WHEN** fallback analysis creates an order result
- **THEN** the fallback result includes `route -> policy`
- **AND** the fallback is metadata-dependency-only rather than dependency-free
- **AND** a successful fallback path is not reported as a terminal error-level workflow failure

#### Scenario: missing dependency fails closed

- **GIVEN** active change `route` references dependency `ghost`
- **AND** `ghost` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** analyzer validation or scheduler dispatch checks evaluate `route`
- **THEN** `ghost` is classified as missing
- **AND** `route` is not dispatched
- **AND** the diagnostic distinguishes missing dependency from archived dependency
- **AND** the unsafe dependency blocker remains visible as actionable operator evidence
