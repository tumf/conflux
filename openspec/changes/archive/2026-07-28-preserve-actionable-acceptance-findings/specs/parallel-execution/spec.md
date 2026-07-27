## ADDED Requirements

### Requirement: Acceptance repair state MUST separate actionable payload from retry identity

Serial and parallel runtime MUST keep the complete latest Acceptance finding payload separate from stable retry identities and semantic fingerprints. Updating comparison identities, semantic baselines, cycle counters, or retry checkpoints MUST NOT mutate or replace actionable evidence, required changes, or verification expectations.

Ordinary retry counters and semantic baselines MUST remain in memory. The runtime-owned current follow-up MUST preserve enough immutable actionable finding detail and Apply remediation evidence for an interrupted FAIL-to-Apply handoff using workspace-local evidence. If actionable workspace evidence is absent or invalid after restart, Conflux MUST rerun Acceptance before Apply and MUST NOT infer a repair target, closure, PASS, or archive readiness from hidden state.

#### Scenario: retry checkpoint cannot overwrite payload

- **GIVEN** Acceptance records a detailed finding and runtime derives `repository|path|verification` as comparison identity
- **WHEN** runtime updates retry identity and semantic baseline state
- **THEN** the complete detailed finding remains unchanged
- **AND** the next Apply receives its evidence, required changes, and verification expectations

#### Scenario: restart preserves constitutional routing

- **GIVEN** orchestration stops after FAIL and before repair Apply
- **WHEN** Conflux resumes the workspace
- **THEN** it uses valid workspace-local current finding evidence or reruns Acceptance
- **AND** missing out-of-worktree metadata cannot imply closure or PASS
- **AND** all archive and merge decisions still require repository-verifiable current-revision evidence

### Requirement: Acceptance repair diff MUST cover declared finding work

Before rerunning Acceptance after repair Apply, serial and parallel runtime MUST compare the workspace delta from the finding's FAIL revision through the repair result. For every structured finding, every declared `required_changes` file and every declared `verification` file MUST occur in that delta. Runtime MUST retain actual changed files, uncovered required files, unrelated changed files, and Apply remediation evidence as structured diagnostics.

Passing coverage authorizes only the next Acceptance review; it MUST NOT prove semantic resolution. Missing declared coverage MUST stop before Acceptance with an evidenced, resumable `acceptance_remediation_mismatch` hold. Changes outside the finding contract, including calibration-only or comment-only changes, MUST NOT satisfy missing coverage. Legacy findings without declared path sets MAY retain compatibility behavior.

#### Scenario: complete coverage permits semantic review

- **GIVEN** a structured finding declares an implementation file and a verification file
- **AND** repair Apply changes both files
- **WHEN** runtime validates the repair delta
- **THEN** coverage passes and Acceptance may run
- **AND** runtime does not claim the finding is resolved until Acceptance decides

#### Scenario: calibration-only change stops before Acceptance

- **GIVEN** a finding requires test-support observability and a value-based integration assertion
- **AND** repair Apply changes only a calibration test or unrelated comments
- **WHEN** runtime validates the delta
- **THEN** coverage fails with `acceptance_remediation_mismatch`
- **AND** Acceptance is not invoked
- **AND** diagnostics identify the missing implementation and verification files plus unrelated changes

#### Scenario: unrelated progress cannot satisfy coverage

- **GIVEN** broad semantic fingerprinting observes source, test, or spec changes
- **AND** none covers a finding's declared required file
- **WHEN** runtime evaluates remediation
- **THEN** semantic progress does not override the coverage failure
- **AND** the change enters the same evidenced hold

### Requirement: Repeated Acceptance finding IDs MUST stop automatic repair

Each stable finding ID MUST receive at most one automatic repair Apply after its first FAIL observation. If the next canonical Acceptance FAIL reports the same ID as still open, runtime MUST stop before another Apply with an evidenced, resumable `repeated_acceptance_finding` hold. Unrelated semantic progress, changed prose, changed line numbers, additional evidence, or different representative paths MUST NOT reset that ID's automatic repair budget.

A genuinely new ID receives one automatic repair opportunity. If a FAIL contains both a repeated ID and a new ID, runtime MUST stop atomically, retain every finding in diagnostics, and MUST NOT dispatch partial Apply work. An explicit operator retry MAY start another revision-bound attempt through the existing stalled retry contract, but MUST NOT erase prior occurrence or remediation evidence.

#### Scenario: same ID stops before second repair Apply

- **GIVEN** finding ID `acceptance-secret-value-scan` received one repair Apply
- **AND** the next Acceptance FAIL reports that ID again
- **WHEN** runtime computes the next action
- **THEN** it enters `repeated_acceptance_finding`
- **AND** it does not start a second automatic repair Apply
- **AND** unrelated changed files do not alter the decision

#### Scenario: changed detail does not create a new opportunity

- **GIVEN** a prior finding has a stable ID
- **AND** the next FAIL changes its summary, line numbers, evidence, or cited path while describing the same defect
- **WHEN** runtime reconciles the result
- **THEN** it recognizes the repeated ID
- **AND** it stops automatic repair rather than treating the prose change as progress

#### Scenario: new finding receives one repair opportunity

- **GIVEN** Acceptance no longer reports the prior ID
- **AND** it reports a genuinely new stable ID
- **WHEN** runtime computes the next action
- **THEN** the prior finding is Acceptance-closed
- **AND** the new finding may receive one automatic repair Apply

#### Scenario: mixed repeated and new findings stop atomically

- **GIVEN** a FAIL contains one ID that already consumed its repair opportunity and one new ID
- **WHEN** runtime computes the next action
- **THEN** it starts no Apply
- **AND** diagnostics retain both findings and identify the repeated ID as the stop reason

### Requirement: Acceptance repair-stop diagnostics MUST be actionable and mode-independent

Serial and parallel execution MUST produce equivalent structured diagnostics for `acceptance_remediation_mismatch` and `repeated_acceptance_finding`. Diagnostics MUST include the complete open findings, stable IDs, occurrence counts, relevant FAIL and Apply revisions, declared required and verification files, actual changed files, coverage results, unrelated files and relationship explanations, Apply remediation evidence, stop reason, resumability, and next action.

These temporary hold records MAY control stalled presentation, ordinary dispatch suppression, and explicit retry eligibility only through the revision-bound lifecycle established by `replace-acceptance-marker-stalls`. They MUST NOT prove implementation completion, finding closure, Acceptance PASS, archive readiness, or merge eligibility, and MUST NOT create an Acceptance-origin worktree marker.

#### Scenario: serial and parallel stop with equivalent evidence

- **GIVEN** serial and parallel observe equivalent detailed findings and repair diffs
- **WHEN** each detects remediation mismatch or a repeated ID
- **THEN** both choose the same stop reason and resumability
- **AND** both expose equivalent structured diagnostic fields
- **AND** neither writes Acceptance-origin workflow evidence into the worktree

#### Scenario: explicit retry remains reviewable

- **GIVEN** an operator explicitly retries an evidenced repair hold
- **WHEN** runtime resumes the current revision
- **THEN** prior finding occurrences and remediation diagnostics remain inspectable
- **AND** the retry resumes at the appropriate revision-bound phase
- **AND** runtime still requires a later current-revision Acceptance PASS before archive
