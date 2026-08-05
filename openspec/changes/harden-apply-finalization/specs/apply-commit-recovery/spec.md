## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Task-complete Apply finalization MUST require explicit staging

Before a task-complete Apply iteration creates its final WIP snapshot, Conflux MUST require the managed workspace to contain no unstaged changes and no untracked files. The Apply agent MUST select change-owned files by staging them and MUST NOT create the final commit. Conflux MUST provide bounded repair feedback when the gate fails and MUST preserve work through the existing WIP snapshot without dispatching Acceptance.

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
**When**: Conflux evaluates finalization eligibility after process-group quiescence and before the WIP snapshot
**Then**: final commit does not start
**And**: bounded `incomplete_stage` feedback identifies affected paths and required repair
**And**: the existing WIP snapshot preserves the workspace
**And**: the next Apply iteration runs within existing iteration and stall limits

#### Scenario: Successful hook leaves workspace changes

**Given**: the hook-enabled final commit exits successfully
**But**: a hook leaves unstaged or untracked workspace content
**When**: Conflux checks post-commit cleanliness
**Then**: Acceptance is not dispatched
**And**: Conflux returns to bounded Apply repair with actionable stage diagnostics

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
