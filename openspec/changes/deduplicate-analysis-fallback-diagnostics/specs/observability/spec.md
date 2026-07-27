## MODIFIED Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

Conflux observability MUST distinguish recoverable degraded execution paths from terminal workflow failures across tracing records and runtime events. Equivalent recoverable fallback diagnostics MUST be deduplicated consistently across both tracing records and runtime events during the existing deduplication lifetime. The bundled log mining helper MUST remain observability-only and MUST NOT influence scheduler decisions, resume routing, acceptance, archive, merge, or next-action behavior.

#### Scenario: recoverable analysis fallback is not presented as terminal failure

- **GIVEN** dependency analysis rejects an LLM-produced graph
- **AND** Conflux successfully constructs metadata-dependency-only fallback analysis
- **WHEN** runtime diagnostics and events are emitted
- **THEN** the fallback remains visible as a warning-level degraded analysis diagnostic
- **AND** the diagnostic identifies metadata dependency fallback and preserves the original analysis failure reason
- **AND** the successful fallback emits no error-level tracing record or terminal error event
- **AND** repeated equivalent tracing records and runtime events are each deduplicated by the same diagnostic signature
- **AND** missing or rejected dependency blockers remain visible as actionable diagnostics
- **AND** observability output does not affect workflow-control decisions

#### Scenario: changed fallback signature remains visible

- **GIVEN** a recoverable fallback diagnostic has already been emitted for one queued set, in-flight set, and normalized rejection reason
- **WHEN** a later fallback has a different rejection reason or queued/in-flight context
- **THEN** a new warning tracing record is emitted
- **AND** a new warning runtime event is emitted
- **AND** the changed context is preserved for operator diagnosis

#### Scenario: fallback preserves safe dependency execution

- **GIVEN** an LLM analysis response is invalid or omits queued change IDs
- **AND** queued changes declare proposal metadata dependencies
- **WHEN** Conflux rejects the LLM response and uses fallback analysis
- **THEN** every queued change remains represented exactly once in fallback order
- **AND** declared metadata dependencies remain present
- **AND** dispatch continues to fail closed for missing or rejected dependency targets
