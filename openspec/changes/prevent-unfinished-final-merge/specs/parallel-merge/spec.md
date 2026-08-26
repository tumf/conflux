## ADDED Requirements

### Requirement: Sequential final merge requires repository-visible task completion

Sequential resolve MUST establish from the selected change's workspace and Git-visible active or archived `tasks.md` that every implementation task is complete before it emits final-merge guidance or permits a final merge mutation. Missing, unreadable, ambiguous, or incomplete task evidence MUST fail closed.

When final merge is not authorized, Conflux MUST expose a typed non-agent-actionable outcome, MUST emit no imperative merge or final-commit command, and MUST terminate through the existing evidence-withheld or manual-action path without mutating the base or change worktree.

A typed resolver safety refusal MAY latch merge authorization off for the remainder of the current in-process batch. The latch MUST be monotonic, MUST NOT authorize any action, MUST NOT depend on narrative output parsing, and MUST disappear on process restart so workflow state remains workspace-derived.

<!-- Expected canonical result after archive: `parallel-merge` will require complete active or archived task evidence before final merge guidance, and unchanged unsafe evidence will not be retried through another agent. -->

#### Scenario: Archived change has an incomplete task

**Given**: A sequential change has been archived in its worktree
**And**: Its archived `tasks.md` records six of seven tasks complete
**When**: Resolve reaches the phase that would otherwise emit final-merge guidance
**Then**: Resolve reports that merge is not authorized
**And**: The diagnosis contains no imperative `git merge` or final-commit instruction
**And**: No resolve agent is started for that final merge
**And**: The base and change worktree remain unchanged

#### Scenario: Active change has an incomplete task

**Given**: A sequential change still has an active `tasks.md`
**And**: At least one implementation task is incomplete
**When**: Resolve evaluates final-merge eligibility
**Then**: Resolve reports that merge is not authorized
**And**: It withholds final-merge guidance without mutating repository state

#### Scenario: Conflict is resolved but merge is not authorized

**Given**: Pre-sync and index conflicts have been resolved
**And**: Repository-visible task evidence remains incomplete
**When**: Resolve reclassifies the batch
**Then**: Conflict resolution completion does not imply final-merge authorization
**And**: Resolve terminates through the non-agent-actionable evidence-withheld path
**And**: It does not launch another attempt with the same final-merge instruction

#### Scenario: Typed safety refusal stops repeated attempts

**Given**: A resolve agent returns a typed safety refusal for a final merge
**And**: Repository evidence does not change
**When**: The current batch considers another resolve attempt
**Then**: The refusal remains latched as merge not authorized for that batch
**And**: Another agent does not receive the same required action
**And**: Narrative text is not parsed as workflow authority

#### Scenario: Complete tasks preserve the existing merge path

**Given**: The selected change's repository-visible task evidence is unambiguous and complete
**And**: Existing ownership, topology, pre-sync, conflict, cleanup, and terminal predicates are valid
**When**: Resolve evaluates final-merge eligibility
**Then**: Task completion does not block the existing exact per-change final merge path
**And**: Existing post-merge verification remains mandatory

#### Scenario: One blocked item does not authorize unrelated mutation

**Given**: A sequential batch contains an item whose final merge is not authorized
**And**: The batch also contains another item
**When**: Resolve terminates the blocked item's attempt
**Then**: It does not mutate either item as a consequence of retrying the blocked instruction
**And**: Any later processing of the other item requires its own existing ordered repository evidence and authorization
