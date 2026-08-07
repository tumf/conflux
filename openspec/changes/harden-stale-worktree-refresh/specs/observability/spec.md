## MODIFIED Requirements

### Requirement: REQ-OBS-001 Command Execution Logging

Conflux observability MUST distinguish recoverable degraded execution paths from terminal workflow failures across tracing records and runtime events. Equivalent recoverable fallback diagnostics MUST be deduplicated consistently across both tracing records and runtime events during the existing deduplication lifetime. The bundled log mining helper MUST remain observability-only and MUST NOT influence scheduler decisions, resume routing, acceptance, archive, merge, or next-action behavior.

VCS simulation diagnostics whose child output size is not intrinsically bounded MUST record structured summaries instead of complete raw stdout/stderr. A summary SHALL retain command outcome, output byte counts, worktree or branch identity when available, conflict count, at most 20 deterministic conflict paths, and at most 4096 bytes of each stdout/stderr prefix. Known merge conflicts SHALL remain ordinary conflict observations and SHALL NOT emit unbounded fallback output on each refresh.

<!-- Expected canonical result after archive: observability retains actionable merge-simulation evidence without allowing repeated child output to grow persistent logs without bound. -->

#### Scenario: Large merge conflict output is bounded

- **GIVEN** `git merge-tree` returns conflict output larger than the diagnostic sample limit
- **WHEN** Conflux records the conflict observation
- **THEN** the diagnostic contains the exit status, total stdout/stderr byte counts, conflict count, worktree identity, and deterministic bounded sample
- **AND** the diagnostic does not contain the complete raw stdout or stderr

#### Scenario: Repeated unchanged conflict does not flood logs

- **GIVEN** an eligible worktree conflict has already been observed for one unchanged revision tuple
- **WHEN** periodic refresh repeats without branch identity, base HEAD, worktree HEAD, or merge-base changes
- **THEN** no duplicate merge simulation output is logged
- **AND** the retained observation remains available to the Worktrees view
