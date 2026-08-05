### Requirement: Final Apply commit hook rejection MUST return to Apply with diagnostics

When the hook-enabled final Apply commit is rejected by a repository commit hook, Conflux MUST treat the rejection as repository-fixable Apply feedback and MUST re-enter Apply in the same workspace before Acceptance. The next Apply prompt MUST include bounded, untrusted diagnostic context containing the failed Git command and available exit status, stdout, and stderr. The prompt MUST require repair of the reported repository failure and rerunning the relevant validation. Complete hook output MUST remain available in persistent logs even when prompt diagnostics are bounded.

#### Scenario: Pre-commit validation rejects the final Apply commit

**Given**: all implementation tasks are complete and WIP snapshots exist
**And**: the normal final Apply commit runs with repository hooks enabled
**When**: a commit hook rejects the final commit with actionable diagnostics
**Then**: Conflux records the structured diagnostics for the same change
**And**: Conflux starts another Apply iteration in the same workspace
**And**: the next Apply prompt includes bounded actionable failure context
**And**: persistent logs retain the complete captured hook output
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
**Then**: Conflux returns a completed Apply result with the committed revision only when the workspace remains clean
**And**: Acceptance may be dispatched

### Requirement: Final Apply commit recovery MUST preserve verification hooks

Conflux MUST NOT bypass repository verification hooks when retrying the final Apply commit. Recovery guidance MUST distinguish the verified final commit from WIP snapshots and MUST NOT direct the agent or runtime to use `--no-verify` for the final commit. Final commit hook execution MUST stream operator-visible progress without weakening typed result classification.

#### Scenario: Final commit is retried without bypass

**Given**: a prior final Apply commit was rejected by a commit hook
**When**: Conflux retries the final Apply commit after a repair iteration
**Then**: the final commit executes repository hooks again
**And**: the final commit command does not include `--no-verify`

#### Scenario: Streamed hook execution preserves classification

**Given**: a final commit hook writes to stdout and stderr
**When**: Conflux streams those lines to operator-visible sinks
**Then**: Conflux also preserves the complete raw stdout, stderr, command, and exit status
**And**: hook rejection and index-lock classification use the preserved structured result rather than rendered log text

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

### Requirement: Final Apply commit retries narrowly classified index-lock contention

When final Apply finalization fails because a structured finalization Git command cannot create the current managed worktree's existing `index.lock`, Conflux MUST retry the complete finalization sequence at most three total attempts with a fixed 200 millisecond delay and no backoff. Conflux MUST preserve repository verification hooks, MUST NOT delete or bypass the lock, and MUST NOT apply this policy to hook rejection or unrelated VCS failures. Completion MUST be proven from repository state so ambiguous command reporting cannot create duplicate final commits.

#### Scenario: transient amend lock clears

- **GIVEN** a WIP snapshot leaves the Apply worktree clean
- **AND** final `git commit --amend --allow-empty` reports the current managed worktree's existing `index.lock`
- **WHEN** the lock clears within the bounded retry budget
- **THEN** Conflux retries normal hook-enabled finalization
- **AND** exactly one final `Apply: <change-id>` commit exists
- **AND** Acceptance may start only after repository state proves that commit succeeded

#### Scenario: transient add-and-commit lock clears

- **GIVEN** final Apply finalization must stage dirty workspace content
- **AND** finalization `git add -A` or the subsequent verified commit reports the current managed worktree's existing `index.lock`
- **WHEN** the lock clears within three total attempts
- **THEN** Conflux repeats complete finalization preparation from current repository state
- **AND** the final hook-enabled commit contains the expected workspace tree exactly once

#### Scenario: persistent lock exhausts bounded retries

- **GIVEN** eligible managed-worktree `index.lock` contention persists for all three attempts
- **WHEN** the retry budget is exhausted
- **THEN** final Apply fails with structured command, workspace, lock, stderr, and attempt diagnostics
- **AND** Conflux does not delete the lock
- **AND** workspace contents remain available for explicit recovery

#### Scenario: repository hook rejection is not lock contention

- **GIVEN** a hook-enabled final Apply commit runs
- **WHEN** a repository hook rejects that commit
- **THEN** Conflux does not consume the index-lock retry budget
- **AND** it routes the rejection through the existing bounded Apply commit-repair behavior

#### Scenario: unrelated VCS failures remain terminal

- **GIVEN** finalization fails for another worktree's lock, malformed lock output, permission or configuration failure, conflict, non-Git backend, or another Git command
- **WHEN** Conflux classifies the failure
- **THEN** it does not retry under the final Apply index-lock policy
- **AND** it returns the original terminal failure context

### Requirement: Task-complete Apply finalization MUST require explicit staging

Before every task-complete final-commit entry point, Conflux MUST require the managed workspace to contain no unstaged changes and no untracked files. The gate applies to post-agent finalization and to a task-complete loop entry or resume where no agent iteration or WIP snapshot precedes the attempt. The Apply agent MUST select change-owned files by staging them and MUST NOT create the final commit. When the gate fails, Conflux MUST provide bounded repair feedback, retain the complete captured porcelain status in persistent logs, leave the workspace and index untouched as restart-visible repair evidence, and MUST NOT create a WIP snapshot, final commit, or Acceptance dispatch.

#### Scenario: Fully staged task-complete workspace reaches final commit

**Given**: all tasks are complete
**And**: every intended change is staged
**And**: `git status --porcelain` reports no unstaged or untracked entries
**When**: Conflux finalizes Apply
**Then**: the existing WIP snapshot and hook-enabled final commit paths run
**And**: WIP `git add -A` adds no new workspace content

#### Scenario: Unstaged or untracked content returns to Apply

**Given**: all tasks are complete
**And**: the managed workspace contains unstaged or untracked entries
**When**: Conflux evaluates any final-commit entry point before WIP snapshot or finalization staging
**Then**: final commit does not start
**And**: bounded `incomplete_stage` feedback identifies affected paths and required repair
**And**: persistent logs retain the complete captured porcelain status
**And**: Conflux leaves the workspace and index unchanged and creates no WIP snapshot
**And**: the next Apply iteration runs within existing iteration and stall limits

#### Scenario: Successful hook leaves workspace changes

**Given**: the hook-enabled final commit exits successfully
**But**: a hook leaves unstaged or untracked workspace content
**When**: Conflux checks post-commit cleanliness
**Then**: Acceptance is not dispatched
**And**: Conflux returns to bounded Apply repair with `incomplete_stage` diagnostics

#### Scenario: Restart after a dirty successful commit resumes into Apply repair

**Given**: a verified final Apply commit exists for the change
**And**: the managed workspace still contains unstaged or untracked entries
**When**: Conflux restarts and derives the next action from the workspace alone
**Then**: the change resumes into Apply repair rather than Acceptance

#### Scenario: Task-complete loop entry applies the same gate

**Given**: all tasks are already complete when the Apply loop starts or resumes
**And**: no agent iteration or WIP snapshot precedes the final-commit attempt
**When**: the workspace contains unstaged or untracked entries
**Then**: Conflux applies the same stage gate before finalization staging
**And**: final commit and Acceptance remain undispatched

### Requirement: Empty successful Apply iterations MUST receive structured retry feedback

When an eligible successful Apply iteration produces neither task progress nor workspace progress, Conflux MUST record structured process-local feedback for the next Apply prompt. The feedback MUST direct the agent to inspect unchecked tasks and existing attempt history, inspect stage and hook diagnostics, and avoid returning while background verification remains active. It MUST NOT duplicate the existing bounded output tail or replace existing stall, escalation, handoff, denial, blocker, rejection, or iteration-budget behavior.

#### Scenario: Empty successful iteration informs the next attempt

**Given**: an Apply iteration exits successfully
**And**: task progress is unchanged
**And**: its WIP snapshot is empty
**And**: no handoff, denial, blocker, or rejection outcome applies
**When**: Conflux prepares another Apply attempt
**Then**: the prompt includes structured `empty_apply_iteration` feedback
**And**: it directs the agent to use the existing prior-attempt output and finish foreground work before returning
**And**: existing stall accounting remains authoritative

#### Scenario: Handoff and terminal outcomes do not receive empty feedback

**Given**: an Apply iteration produces handoff, permission denial, blocker, rejection, cancellation, or terminal failure semantics
**When**: Conflux classifies the result
**Then**: `empty_apply_iteration` feedback does not override that classification
