## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts. Serial and parallel acceptance paths MUST preserve this distinction.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in in-memory runtime history, events, or non-authoritative observability logs, but the runtime MUST NOT write a new workspace-root report file as acceptance completion evidence.

Archive and resume routing MUST NOT depend on `ACCEPTANCE_REPORT.json`. The absence of that file MUST NOT prevent existing acceptance history recording, event emission, or subsequent routing behavior that is otherwise valid for the workspace file/git state.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit is a missing-verdict failure

- **GIVEN** an acceptance command reports that it is waiting for a verification completion notification
- **AND** the command exits without emitting a canonical verdict
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result is an explicit missing-verdict protocol failure
- **AND** the result is not classified as `CONTINUE`
- **AND** operator-visible diagnostics and attempt evidence identify the missing verdict
- **AND** the configured explicit-CONTINUE retry counter/path is not used

#### Scenario: explicit CONTINUE retains retry semantics

- **GIVEN** an acceptance command emits a canonical JSON or legacy `CONTINUE` verdict
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an intentional `CONTINUE`
- **AND** the configured continuation retry policy applies unchanged

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** the runtime records the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json`
- **AND** evidence is carried through existing result, history, event, and observability paths

#### Scenario: canonical outcomes remain unchanged

- **GIVEN** an acceptance command emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **WHEN** serial or parallel acceptance classifies the output
- **THEN** each canonical outcome retains its existing routing semantics
- **AND** missing-verdict handling does not override the canonical verdict
