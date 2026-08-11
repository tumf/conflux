## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST continue to accept only the closed command set, including `set_execution_mark`, `set_all_execution_marks`, explicit queue commands, start/retry, stop, dequeue, and resolve controls. Accepted commands MUST execute through the same process-local application transaction used by the TUI.

`set_execution_mark` and `set_all_execution_marks` MUST represent process-local next-run target intent only. They MUST accept visible non-terminal targets for which the reducer has not recorded archive completion, independent of app mode, active/retry/wait status, Apply iteration-limit evidence, and current parallel eligibility, and MUST NOT mutate queue intent, active execution, cancellation, retry/resolve state, hooks, scheduler state, or process mode. Targets with terminal display status (`archived`, `merged`, `pushed`, or `rejected`) or reducer-recorded archive completion MUST settle as unchanged no-op outcomes with a stable non-markable-target reason and no effects.

Start/retry MUST perform current reducer and worktree eligibility checks at final admission. A worktree-ineligible marked target MUST reject the complete request. Other non-startable statuses MUST be excluded with target-specific detail, and zero runnable targets MUST reject. Error-mode retry MUST route only marked retry-eligible error targets. Failed admission MUST NOT produce partial queue, scheduler, retry-edge, or projection effects.

#### Scenario: Single mark is lifecycle-independent and side-effect free

**Given**: A visible non-terminal change exists in any app mode or lifecycle status
**And**: The reducer has not recorded archive completion for that change
**When**: `set_execution_mark` changes its mark
**Then**: the shared mark store and coherent snapshot reflect the new value
**And**: queue intent, runtime, cancellation, retry, resolve, hooks, scheduler, and mode remain unchanged

#### Scenario: Non-markable single mark is a reasoned unchanged no-op

**Given**: A target has terminal display status or reducer-recorded archive completion
**When**: `set_execution_mark` is submitted
**Then**: the command settles successfully as unchanged
**And**: the outcome identifies a stable non-markable-target reason
**And**: no mark, queue, runtime, revision, or scheduler effect occurs

#### Scenario: Bulk mark changes only markable rows

**Given**: Visible markable and non-markable changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from rows without terminal display status or reducer-recorded archive completion only
**And**: It updates only execution marks atomically
**And**: It returns changed IDs without Running queue-intent effects

#### Scenario: Worktree-invalid Start is rejected atomically

**Given**: Marks include a worktree-ineligible target
**When**: Start is submitted
**Then**: the complete request is rejected
**And**: No scheduler is prepared, activated, or notified
**And**: no queue, mark, retry-edge, reservation, mode, hook, or projection effect survives
**And**: the command identifies the target and reason

#### Scenario: Mixed status Start admits runnable subset

**Given**: Marks include at least one runnable target and another currently non-startable status
**And**: no marked target violates the worktree eligibility fence
**When**: Start is submitted
**Then**: runnable targets are admitted
**And**: non-startable statuses are reported as excluded with target-specific detail

#### Scenario: Zero runnable targets is rejected

**Given**: Marks exist but no runnable target remains after status classification
**When**: Start or Retry is submitted
**Then**: no scheduler or queue effect occurs
**And**: the command rejects with actionable exclusion detail

<!-- Expected canonical result after archive: `remote-control-api` will keep mark commands lifecycle-independent for markable rows while making reducer-recorded archive completion a reasoned unchanged no-op without adding an archive field to state payloads. -->
