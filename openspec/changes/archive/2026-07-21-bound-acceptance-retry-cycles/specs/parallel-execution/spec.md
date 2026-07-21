## MODIFIED Requirements

### Requirement: Acceptance failure returns to apply loop

When acceptance returns FAIL, execution MUST permit at least one apply retry for repository-fixable findings regardless of whether the workspace started fresh or resumed. Before a later apply retry, runtime MUST compare the current normalized finding identity set and repository-visible semantic progress with the previous failed attempt. If the same findings recur after the permitted apply and no semantic progress exists, execution MUST enter a resumable stalled hold before invoking apply again.

Semantic progress MUST include substantive committed and uncommitted repository changes and MUST exclude runtime-managed acceptance follow-up content, blocker markers, attempt counters, logs, and observability-only state. Finding order, duplicates, and presentation-only whitespace MUST NOT create distinct identity sets. Previous finding identities, semantic baseline, and cycle count MUST be loaded from and updated through the workspace-local retry checkpoint so process restart cannot reset the retry decision.

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
- **AND** it does not invoke apply again or emit terminal Error solely for repetition

#### Scenario: real progress permits another bounded retry

- **GIVEN** acceptance returns the same finding identity after apply
- **AND** source, test, configuration, spec, or substantive task content changed
- **WHEN** runtime evaluates progress
- **THEN** the change remains eligible for another bounded apply retry
- **AND** runtime-owned bookkeeping alone would not produce this result

## ADDED Requirements

### Requirement: Acceptance retry safeguards are mode-independent

Serial and parallel execution MUST use equivalent finding normalization, semantic progress, retry, mixed-blocker, and stalled classification. The existing apply+acceptance ceiling of ten cycles MUST remain a safety ceiling, but exhaustion MUST produce a resumable `acceptance_cycle_limit_exhausted` stalled hold with workspace-local evidence instead of terminal Error.

#### Scenario: cycle ceiling preserves resumability

- **GIVEN** a change reaches the tenth apply+acceptance cycle without acceptance PASS
- **WHEN** runtime enforces the safety ceiling
- **THEN** it enters `acceptance_cycle_limit_exhausted` stalled
- **AND** it preserves the worktree and retry evidence
- **AND** it does not classify the ceiling as terminal implementation failure

#### Scenario: serial and parallel classify equivalent observations equally

- **GIVEN** serial and parallel observe equivalent prior findings, current findings, and workspace progress
- **WHEN** each computes its retry decision
- **THEN** both choose the same apply-retry or stalled outcome and reason

### Requirement: Acceptance findings retain repository and external scopes

Runtime MUST classify findings individually as repository-fixable or external/non-mockable. Repository-fixable findings MUST remain actionable repair input. External blockers MUST be retained and MUST NOT disappear when repository findings are present. If repository findings are resolved and only an unresolved external blocker remains, runtime MUST preserve it through the stalled path.

#### Scenario: mixed findings preserve both responsibilities

- **GIVEN** acceptance identifies a repository defect and an external prerequisite
- **WHEN** runtime evaluates the FAIL
- **THEN** the repository defect remains apply-repairable
- **AND** the external prerequisite remains blocker evidence
- **AND** apply is not instructed to satisfy the external prerequisite by repository edits

#### Scenario: external blocker remains after repository repair

- **GIVEN** apply resolves all repository-fixable findings
- **AND** an external non-mockable blocker remains
- **WHEN** acceptance runs again
- **THEN** the blocker is preserved in a resumable stalled hold
