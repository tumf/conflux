## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Parallel and serial execution SHALL run `acceptance_command` after a successful apply and before archive in each workspace. The acceptance loop SHALL parse stdout to determine pass/fail/continue/stalled-hold outcomes, and MUST NOT use exit code as the acceptance verdict when a canonical verdict is present.

A completed acceptance command that emits no canonical verdict MUST be classified as an explicit missing-verdict protocol failure, not as an intentional `CONTINUE`. The runtime MUST record bounded output evidence and emit an actionable operator-visible diagnostic. Missing-verdict failures MUST NOT consume or enter the configured retry path reserved for explicit canonical `CONTINUE` verdicts.

During an active run, serial and parallel execution MUST instead apply the same dedicated missing-verdict protocol-retry policy. While the dedicated budget remains, runtime MUST invoke the normal configured acceptance command again with bounded Conflux-managed prior acceptance output, attempt evidence, current workspace context, and a trusted corrective instruction requiring exactly one canonical verdict. This continuation MUST NOT depend on harness session resume, harness-specific CLI flags, provider events, or external managed-job identifiers.

The dedicated policy MUST allow no more than two retries after the initial missing-verdict attempt. Its counter MUST be independent from explicit-`CONTINUE` accounting and MUST reset after any canonical verdict. Exhaustion MUST produce a terminal missing-verdict protocol failure with bounded evidence and attempt-count diagnostics. Acceptance command launch or execution failure MUST retain command-failure routing and MUST NOT be reclassified as a missing-verdict retry.

Acceptance execution MUST NOT create a workspace-root `ACCEPTANCE_REPORT.json` artifact or another generated retry checkpoint for PASS, command failure, FAIL, CONTINUE, stalled-hold, or missing-verdict outcomes. Acceptance outcomes MAY be recorded in active-run memory, events, or non-authoritative observability logs, but archive and resume routing MUST remain derivable from workspace file/git state. After process restart, an applied but unarchived workspace MUST run acceptance again and MUST NOT infer acceptance from prior narrative output.

When acceptance returns a stalled-hold compatibility verdict for infrastructure, external dependency, missing non-mockable credential, or pending-verification evidence, parallel execution SHALL record a non-terminal stalled hold and SHALL NOT invoke terminal rejection flow solely because of that verdict.

#### Scenario: status-only exit continues through a dedicated protocol retry

- **GIVEN** an acceptance command reports that it is waiting for owned verification
- **AND** the command exits without emitting a canonical verdict
- **AND** the active run has missing-verdict retry budget remaining
- **WHEN** acceptance execution classifies the completed command
- **THEN** the result remains an explicit missing-verdict protocol failure and is not classified as `CONTINUE`
- **AND** runtime records bounded evidence and invokes the normal acceptance command again
- **AND** the new prompt includes bounded prior attempt context plus a corrective canonical-verdict instruction
- **AND** the configured explicit-`CONTINUE` counter is unchanged
- **AND** queue reconciliation does not classify the active retry as `terminal_error_retry_required`

#### Scenario: continuation is harness neutral

- **GIVEN** any configured acceptance command can receive the normal Conflux prompt
- **WHEN** runtime retries after a missing verdict
- **THEN** continuity is provided only through Conflux-managed prompt context and workspace evidence
- **AND** runtime does not require a harness session ID, resume flag, provider event, or external job ID

#### Scenario: missing-verdict retry budget is exhausted

- **GIVEN** an initial acceptance attempt and two consecutive protocol retries all emit no canonical verdict
- **WHEN** runtime classifies the third consecutive missing verdict
- **THEN** it emits the terminal missing-verdict protocol failure
- **AND** the diagnostic identifies the exhausted attempts and includes bounded evidence
- **AND** no fourth protocol retry starts

#### Scenario: canonical outcome resets protocol retry state

- **GIVEN** acceptance previously entered a missing-verdict protocol retry
- **WHEN** a later invocation emits canonical PASS, FAIL, CONTINUE, or stalled-hold output
- **THEN** that canonical outcome retains its existing routing semantics
- **AND** the consecutive missing-verdict retry count resets
- **AND** explicit `CONTINUE` retains its configured continuation policy

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the workspace remains applied but unarchived
- **WHEN** Conflux restarts without out-of-worktree runtime state
- **THEN** it runs acceptance again from workspace file/git state
- **AND** it does not infer PASS from prior missing-verdict output
- **AND** it does not require a generated acceptance retry checkpoint

#### Scenario: missing verdict does not create report artifact

- **GIVEN** an acceptance command exits without a canonical verdict
- **WHEN** runtime records or retries the missing-verdict failure
- **THEN** the workspace root does not contain `ACCEPTANCE_REPORT.json` or another generated acceptance retry checkpoint
- **AND** evidence is carried through existing prompt, history, event, and observability paths
