## MODIFIED Requirements

### Requirement: Acceptance failure returns to apply loop

When acceptance returns FAIL, execution MUST permit at least one apply retry for repository-fixable findings regardless of whether the workspace started fresh or resumed. Before a later apply retry, runtime MUST compare the current normalized finding identity set and repository-visible semantic progress with the previous failed attempt. If the same findings recur after the permitted apply and no semantic progress exists, execution MUST enter a resumable stalled hold before invoking apply again.

Semantic progress MUST include substantive repository changes and MUST exclude runtime-managed acceptance follow-up content, acceptance blocker markers, attempt counters, logs, and other observability-only state. String finding order, duplicate entries, and presentation-only whitespace MUST NOT create false progress or distinct finding sets.

#### Scenario: resumed workspace gets one repair attempt

- **GIVEN** a resumed Applied workspace runs acceptance
- **WHEN** acceptance returns repository-fixable FAIL findings for the first time
- **THEN** the next cycle runs apply before acceptance
- **AND** the change is not stalled solely because it resumed into acceptance

#### Scenario: repeated findings without progress stall before apply

- **GIVEN** acceptance returned FAIL and apply ran once
- **AND** the next acceptance returns the same normalized finding identity set
- **AND** the workspace has no semantic progress since the previous failed attempt
- **WHEN** runtime chooses the next action
- **THEN** it records `repeated_acceptance_findings` as a resumable stalled hold
- **AND** it does not invoke apply again
- **AND** it does not emit terminal Error solely for the repetition

#### Scenario: real repository progress permits another bounded retry

- **GIVEN** acceptance returns the same finding identity after apply
- **AND** source, test, configuration, spec, or substantive task content changed
- **WHEN** runtime evaluates progress
- **THEN** the change remains eligible for another bounded apply retry
- **AND** runtime-owned follow-up changes alone would not produce this result

## ADDED Requirements

### Requirement: Acceptance retry safeguards are mode-independent

Serial and parallel execution MUST use equivalent finding normalization, semantic progress, retry, mixed-blocker, and stalled classification. The existing apply+acceptance ceiling of ten cycles MUST remain a safety ceiling, but exhaustion MUST produce a resumable `acceptance_cycle_limit_exhausted` stalled hold with preserved workspace evidence instead of terminal Error.

#### Scenario: cycle ceiling preserves resumability

- **GIVEN** a change reaches the tenth apply+acceptance cycle without acceptance PASS
- **WHEN** the runtime enforces the safety ceiling
- **THEN** it enters a resumable stalled hold
- **AND** it preserves the worktree, current findings, retry count, and next action
- **AND** it does not classify the ceiling as terminal implementation failure

#### Scenario: serial and parallel classify the same observation equally

- **GIVEN** serial and parallel execution observe equivalent prior findings, current findings, and workspace progress
- **WHEN** each computes its retry decision
- **THEN** both choose the same apply-retry or stalled outcome
- **AND** both use the same stalled reason

### Requirement: Acceptance findings retain repository and external scopes

Runtime MUST classify findings individually as repository-fixable or external/non-mockable. Repository-fixable findings MUST remain actionable FAIL follow-up. External blockers MUST be retained as non-checkbox metadata and MUST NOT disappear when repository findings are present. If repository findings are resolved and only an unresolved external blocker remains, runtime MUST preserve it through the stalled-hold path.

#### Scenario: mixed findings preserve both responsibilities

- **GIVEN** acceptance identifies a repository defect and an external deployment prerequisite
- **WHEN** runtime persists the FAIL follow-up
- **THEN** the repository defect is an unchecked repair task
- **AND** the external prerequisite is non-checkbox blocker metadata
- **AND** apply is not instructed to satisfy the external prerequisite by repository edits

#### Scenario: external blocker remains after repository repair

- **GIVEN** apply resolves all repository-fixable findings
- **AND** an external non-mockable blocker remains
- **WHEN** acceptance runs again
- **THEN** the blocker is preserved in a resumable stalled hold
- **AND** it is not discarded or converted into a repository checkbox

### Requirement: Acceptance stalled retry evidence is workspace-local

A repeated-finding or cycle-limit stalled hold MUST be represented by workspace-local evidence using the existing apply-blocked marker contract or an equivalent workspace file. Ordinary dispatch MUST honor that evidence after restart. Explicit retry MAY consume a resumable acceptance-generated marker, but MUST NOT clear unrelated apply blockers.

#### Scenario: restart reconstructs acceptance stalled state

- **GIVEN** a workspace contains an acceptance-generated resumable blocker marker
- **AND** out-of-worktree Conflux state is absent
- **WHEN** Conflux detects the workspace after restart
- **THEN** it reconstructs the stalled hold and next action from workspace evidence
- **AND** ordinary dispatch does not start apply

#### Scenario: explicit retry clears only acceptance-generated marker

- **GIVEN** an operator explicitly retries a stalled acceptance change
- **WHEN** runtime prepares the workspace for retry
- **THEN** it consumes the resumable acceptance-generated marker
- **AND** an unrelated apply-generated blocker marker is not silently cleared
