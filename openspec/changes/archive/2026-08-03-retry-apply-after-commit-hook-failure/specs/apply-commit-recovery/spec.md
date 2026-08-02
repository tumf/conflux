## ADDED Requirements

### Requirement: Final Apply commit hook rejection MUST return to Apply with diagnostics

When the hook-enabled final Apply commit is rejected by a repository commit hook, Conflux MUST treat the rejection as repository-fixable Apply feedback and MUST re-enter Apply in the same workspace before Acceptance. The next Apply prompt MUST include bounded, untrusted diagnostic context containing the failed Git command and available exit status, stdout, and stderr. The prompt MUST require repair of the reported repository failure and rerunning the relevant validation.

#### Scenario: Pre-commit validation rejects the final Apply commit

**Given**: all implementation tasks are complete and WIP snapshots exist
**And**: the normal final Apply commit runs with repository hooks enabled
**When**: a commit hook rejects the final commit with actionable diagnostics
**Then**: Conflux records the structured diagnostics for the same change
**And**: Conflux starts another Apply iteration in the same workspace
**And**: the next Apply prompt includes the actionable failure context
**And**: Acceptance is not dispatched

#### Scenario: Clean-tree amend hook rejection is not treated as success

**Given**: a WIP snapshot leaves the Apply workspace clean
**And**: finalization uses the hook-enabled amend path
**When**: a repository commit hook rejects the amend
**Then**: Conflux propagates a typed repository-rejection outcome with captured diagnostics
**And**: the unchanged WIP commit is not reported as a completed Apply result
**And**: Acceptance is not dispatched

#### Scenario: Completed tasks do not suppress the repair agent

**Given**: all implementation tasks are marked complete
**And**: final commit rejection diagnostics are pending
**When**: Conflux starts the recovery iteration
**Then**: Conflux bypasses the task-complete short circuit
**And**: one Apply agent command runs before final commit is retried

#### Scenario: Repair succeeds and final commit is verified

**Given**: Apply was retried with final commit rejection diagnostics
**When**: the agent repairs the reported failure and the normal final commit succeeds
**Then**: Conflux returns a completed Apply result with the committed revision
**And**: Acceptance may be dispatched

### Requirement: Final Apply commit recovery MUST preserve verification hooks

Conflux MUST NOT bypass repository verification hooks when retrying the final Apply commit. Recovery guidance MUST distinguish the verified final commit from WIP snapshots and MUST NOT direct the agent or runtime to use `--no-verify` for the final commit.

#### Scenario: Final commit is retried without bypass

**Given**: a prior final Apply commit was rejected by a commit hook
**When**: Conflux retries the final Apply commit after a repair iteration
**Then**: the final commit executes repository hooks again
**And**: the final commit command does not include `--no-verify`

### Requirement: Final Apply commit recovery MUST be bounded and structurally classified

Conflux MUST count commit-hook repair iterations against the existing maximum Apply iteration budget. It MUST trigger repair only from a typed outcome produced at the final-commit call site, with the actual process exit code preserved, and MUST leave unrelated VCS failures terminal. Failures in pre-commit setup steps and fatal Git failures MUST NOT be classified as repository rejection. Generic substring matching over rendered error text MUST NOT determine recovery eligibility.

#### Scenario: Repeated hook rejection exhausts Apply iterations

**Given**: the final Apply commit continues to be rejected by an eligible commit hook failure
**When**: no Apply iteration budget remains
**Then**: Conflux stops retrying
**And**: the terminal Apply error contains the latest bounded actionable diagnostics
**And**: Acceptance is not dispatched

#### Scenario: Non-hook VCS failure remains terminal

**Given**: finalization encounters a VCS failure not structurally classified as commit-hook rejection
**When**: the failure reaches the shared Apply loop
**Then**: Conflux returns the VCS failure without dispatching another Apply agent iteration
**And**: Acceptance is not dispatched

### Requirement: Final Apply commit recovery MUST remain workspace-local

Commit-hook recovery MUST NOT introduce out-of-worktree durable workflow-control state. Immediate retry context MAY use process-local Apply history, but restart routing and eligibility MUST remain derivable from workspace files and Git state.

#### Scenario: Restart discards process-local diagnostics

**Given**: a process stops after a final commit hook rejection
**When**: Conflux resumes from the preserved workspace in a new process
**Then**: the next action is derived from workspace files and Git state
**And**: deletion of external Conflux state does not change that routing decision
