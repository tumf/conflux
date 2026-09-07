## ADDED Requirements

### Requirement: Invalid Acceptance results MAY use a bounded alternate reviewer

Managed-worktree execution MUST classify completed Acceptance results before selecting the next reviewer command. `MissingVerdict`, `BareBlocker`, `MalformedFinding`, and FAIL with no actionable finding payload are eligible invalid results. When configured threshold and use budget permit, the next existing Acceptance-only retry MUST use `acceptance_escalation_command` with the same generated prompt and placeholder contract as `acceptance_command`.

The escalation command MUST NOT create an additional retry opportunity beyond the existing result-specific retry budget. A canonical PASS, actionable FAIL, CONTINUE, validated external blocker, permission hold, command failure, runtime limit, or cancellation MUST NOT trigger Acceptance escalation. Missing escalation command MUST preserve existing normal-command routing. Equivalent CLI, TUI, and remote-controlled execution MUST use the same classification, reset, command-selection, and cap policy.

For FAIL with no actionable findings, the Acceptance-only retry MUST be bounded solely by the escalation use budget. If the configured threshold has not been reached, the use cap is exhausted, or the escalation command is absent, runtime MUST follow the existing generic FAIL-to-Apply fallback. The invalid-result sequence MUST remain active across that Apply round until a completed non-invalid Acceptance result, revision change, or terminal outcome resets it.

All escalation accounting MUST remain active-run memory only. Any completed non-invalid result MUST reset the invalid-result sequence. Restart MUST recompute Acceptance from workspace file and Git state without a persisted escalation checkpoint.

#### Scenario: malformed finding selects alternate reviewer

- **GIVEN** the Acceptance escalation command is configured with default policy
- **AND** Acceptance emits a malformed structured FAIL finding
- **WHEN** the existing protocol policy permits the next Acceptance-only retry
- **THEN** that retry uses `acceptance_escalation_command`
- **AND** it retains the bounded corrective context for the malformed finding contract
- **AND** it does not rerun Apply or cleanup-review

#### Scenario: empty FAIL does not dispatch a generic repair

- **GIVEN** Acceptance emits FAIL with no actionable findings
- **AND** the runtime would otherwise create only `Investigate acceptance failure and apply the required fix`
- **WHEN** the escalation policy permits a retry
- **THEN** the next Acceptance-only retry uses the alternate reviewer
- **AND** no Apply repair is dispatched from the generic fallback alone

#### Scenario: empty FAIL below threshold preserves existing routing and sequence

- **GIVEN** Acceptance emits FAIL with no actionable findings
- **AND** the configured invalid-result threshold has not been reached
- **WHEN** runtime routes the result
- **THEN** it dispatches the existing generic FAIL-to-Apply fallback
- **AND** it retains the invalid-result count across that Apply round
- **AND** a later invalid Acceptance result on the same revision may reach the escalation threshold

#### Scenario: actionable FAIL remains repair work

- **GIVEN** Acceptance emits FAIL with at least one actionable finding
- **WHEN** runtime routes the result
- **THEN** it records the complete finding and returns to Apply
- **AND** it does not increment or select Acceptance escalation

#### Scenario: escalation consumes existing retry budget

- **GIVEN** an eligible invalid-result sequence reaches the escalation threshold
- **AND** only one result-specific protocol retry remains
- **WHEN** the escalation command runs and again emits the same invalid result
- **THEN** runtime follows the existing terminal protocol-exhaustion path
- **AND** it does not add another escalation retry

#### Scenario: non-invalid result resets the sequence

- **GIVEN** an invalid result has made escalation eligible
- **WHEN** a later Acceptance invocation completes with any non-invalid result
- **THEN** the invalid-result count and escalation-use count reset
- **AND** a future invalid result starts a new sequence

#### Scenario: restart has no escalation checkpoint

- **GIVEN** an escalation retry was pending before owner termination
- **AND** the workspace remains applied and unarchived
- **WHEN** Conflux restarts
- **THEN** it runs ordinary Acceptance from workspace and Git evidence
- **AND** it does not read an escalation counter or command mode from persistent state
