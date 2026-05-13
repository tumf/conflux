## MODIFIED Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

Conflux observability MUST distinguish recoverable degraded execution paths from terminal workflow failures. The bundled log mining helper MUST remain observability-only and MUST NOT influence scheduler decisions, resume routing, acceptance, archive, merge, or next-action behavior.

#### Scenario: recoverable analysis fallback is not mined as terminal error

- **GIVEN** dependency analysis rejects an LLM-produced graph
- **AND** Conflux successfully falls back to metadata-dependency-only analysis
- **WHEN** runtime logs are emitted and later mined by `scripts/cflx-log-mine.py`
- **THEN** the fallback remains visible as degraded analysis evidence
- **AND** the recoverable fallback is not emitted as an ERROR-level terminal workflow failure
- **AND** missing or rejected dependency blockers remain visible as actionable diagnostics
- **AND** mined log output does not affect workflow-control decisions
