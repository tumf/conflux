## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, or stalled-hold outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

#### Scenario: acceptance pass does not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command emits a canonical PASS verdict
- **WHEN** acceptance finalizes the pass
- **THEN** the runtime records the acceptance attempt in existing acceptance history
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** the pass result does not require any workspace-root report file for later routing

#### Scenario: command failure does not create misleading pass report

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the acceptance command exits unsuccessfully without a finalized canonical verdict
- **WHEN** acceptance returns a command-failure result
- **THEN** the runtime records the failed attempt through existing history paths
- **AND** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** no JSON artifact is written that could describe the failed attempt as `pass`

#### Scenario: non-pass verdicts do not create report artifact

- **GIVEN** a parallel workspace runs acceptance for change `alpha`
- **AND** the final acceptance verdict is FAIL, CONTINUE, or a stalled-hold compatibility verdict
- **WHEN** acceptance finalizes that outcome
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** any required follow-up information is carried by the existing result, findings, events, or history rather than a workspace-root report file
