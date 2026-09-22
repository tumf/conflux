## MODIFIED Requirements

### Requirement: cflx-accept MUST define a portable acceptance skill interface

The dedicated `cflx-accept` skill MUST provide operation identity, scoped acceptance guidance, and the portable acceptance verdict output interface without requiring a specific agent runtime or command template. Runtime-specific entrypoints MAY mirror this interface as adapters, but they MUST NOT be the authoritative source that an acceptance skill must inspect to produce a correct verdict.

The primary acceptance verdict contract MUST be a strict JSON object emitted as the final machine-readable verdict payload. The runtime MAY continue to accept legacy plain-text standalone lines such as `ACCEPTANCE: PASS` as a backward-compatible fallback, but canonical guidance MUST prefer the JSON contract.

Repository-fixable findings MUST be atomic and SHOULD include a stable finding code. Implementation defects and missing verification evidence MUST be separate findings when they can be resolved or verified independently. Acceptance MUST evaluate the current worktree before re-reporting a prior finding and MUST NOT add a broad duplicate finding that merely aggregates test work already owned by specific findings.

Apply guidance MUST treat runtime-owned acceptance finding text as immutable task identity metadata. The general rule allowing task descriptions to be refined MUST NOT apply to runtime-owned acceptance findings; remediation and verification evidence MUST be recorded separately.

When the runtime supplies an Acceptance execution manifest, its eligible declared change-blocking verification set MUST be the complete blocking command boundary for both `cflx-accept` and `cflx-accept-with-speca`. The skill MUST evaluate the frozen candidate revision, review range, change artifacts, diff, and typed gate results. It MUST NOT start repository-wide/full-suite checks, base-worktree comparisons, sibling-change checks, or unrelated artifact investigations as prerequisites for a verdict. Already-available undeclared observations MAY inform a repository finding, but their absence, failure, or incompletion MUST remain non-blocking.

#### Scenario: cflx-accept defines JSON-primary verdict contract

- **GIVEN** the acceptance prompt loads `cflx-accept`
- **WHEN** the skill describes the final verdict format
- **THEN** it defines a strict JSON verdict object as the primary machine-readable contract
- **AND** it documents plain-text standalone verdict markers only as backward-compatible fallback guidance
- **AND** it does not require runtime-specific command files or a particular command invocation mechanism to describe the verdict interface

#### Scenario: repo-local acceptance skills follow the same contract

- **GIVEN** repo-local acceptance-related skills under `skills/` are reviewed
- **WHEN** they describe acceptance output expectations
- **THEN** they reference the same JSON-primary verdict contract
- **AND** they do not redefine a conflicting text-only canonical output rule
- **AND** they do not treat runtime-specific command templates as the authoritative source for the skill interface

#### Scenario: acceptance emits atomic current-state findings

- **GIVEN** a prior FAIL reported implementation and verification defects
- **WHEN** acceptance reviews a newer worktree state
- **THEN** it re-evaluates each defect against current repository evidence
- **AND** it reports implementation and verification defects separately when independently actionable
- **AND** it does not add an aggregate finding that duplicates their verification work
- **AND** each repository finding includes a stable code when the reviewer can provide one

#### Scenario: apply preserves runtime-owned finding text

- **GIVEN** runtime persisted an acceptance finding as a follow-up task
- **WHEN** apply fixes and verifies that finding
- **THEN** apply marks the existing task complete without rewriting its finding text
- **AND** remediation or verification evidence is recorded separately


#### Scenario: declared verification closes the blocking command set

- **GIVEN** the execution manifest names one eligible change-blocking verification
- **AND** its typed runtime-owned result is successful or exactly reused
- **WHEN** either acceptance skill performs semantic review
- **THEN** it does not start any undeclared verification as a completion prerequisite
- **AND** it emits a verdict from the frozen candidate and declared evidence

#### Scenario: broad observation is non-blocking

- **GIVEN** an undeclared repository-wide observation is already complete and available to the reviewer
- **WHEN** it reveals a concrete defect in the candidate implementation
- **THEN** the reviewer may cite that evidence in an atomic repository finding
- **AND** a missing, failed, running, or incomplete undeclared observation cannot delay or block the verdict by itself

### Requirement: Acceptance agents retain ownership of started verification

Acceptance guidance MUST require the parent acceptance agent to wait for the final result of every manifest-declared verification, sub-agent, job, or monitored operation it starts within the runtime-provided review boundary before emitting its final verdict. The agent MUST NOT start undeclared verification work. It MUST NOT terminate with only progress prose, a waiting message, or a promise to decide after a future completion notification. Work that remains active at the review deadline MUST be terminated and reaped by the runtime and MUST NOT enter the missing-verdict retry loop. This rule MUST remain portable and MUST NOT depend on a named runtime-specific monitoring tool.

#### Scenario: monitored verification completes before verdict

**Given**: the acceptance agent starts a verification operation whose result arrives asynchronously
**When**: the operation is still running or its completion notification has not been received
**Then**: the parent acceptance agent continues waiting and does not terminate the acceptance response
**And**: after the final result is received, the parent evaluates the evidence and emits the canonical acceptance verdict

#### Scenario: waiting prose is not a terminal acceptance response

**Given**: the acceptance agent has started a long-running verification
**When**: it reports that verification is still being monitored
**Then**: that status message is not treated by the guidance as completion
**And**: the agent remains responsible for obtaining the result and emitting the canonical verdict

#### Scenario: standard and SPECA skills share completion ownership

**Given**: either `cflx-accept` or `cflx-accept-with-speca` is selected
**When**: acceptance starts manifest-authorized verification or review work
**Then**: both skills require the same bounded wait-for-result and final-verdict behavior
**And**: neither skill depends on a provider-specific command or tool name


#### Scenario: review deadline owns unfinished work

**Given**: the acceptance agent has not completed manifest-authorized work by the review deadline
**When**: the reserved finalization window begins
**Then**: runtime terminates and reaps the owned process group
**And**: the invocation settles as `stalled` with category `acceptance_review_deadline_exhausted` rather than starting a missing-verdict retry
