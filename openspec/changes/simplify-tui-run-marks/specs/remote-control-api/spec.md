## MODIFIED Requirements

### Requirement: Closed shared command delegation

`POST /api/v2/commands` MUST continue to accept only the closed command set, including `set_execution_mark`, `set_all_execution_marks`, explicit queue commands, start/retry, stop, dequeue, and resolve controls. Accepted commands MUST execute through the same process-local application transaction used by the TUI.

`set_execution_mark` and `set_all_execution_marks` MUST represent process-local next-run target intent only. They MUST accept visible pre-archive targets independent of app mode, active/retry/wait status, Apply iteration-limit evidence, and current parallel eligibility, and MUST NOT mutate queue intent, active execution, cancellation, retry/resolve state, hooks, scheduler state, or process mode. Archived, merged, and pushed targets MUST settle as unchanged no-op or failed without effects.

Start/retry MUST perform current reducer and worktree eligibility checks at final admission. Invalid marked target sets MUST NOT produce partial queue, scheduler, retry-edge, or projection effects.

#### Scenario: Single mark is lifecycle-independent and side-effect free

**Given**: A visible pre-archive change exists in any app mode or lifecycle status
**When**: `set_execution_mark` changes its mark
**Then**: the shared mark store and coherent snapshot reflect the new value
**And**: queue intent, runtime, cancellation, retry, resolve, hooks, scheduler, and mode remain unchanged

#### Scenario: Bulk mark changes only pre-archive marks

**Given**: Visible pre-archive and post-archive changes exist at one state revision
**When**: `set_all_execution_marks` is accepted
**Then**: The service selects one target state from pre-archive rows only
**And**: It updates only execution marks atomically
**And**: It returns changed IDs without Running queue-intent effects

#### Scenario: Empty or invalid Start is not successful

**Given**: Marks exist but no valid run target set exists at final admission
**When**: Start is submitted
**Then**: No scheduler is prepared, activated, or notified
**And**: no queue, mark, retry-edge, reservation, mode, hook, or projection effect survives
**And**: the command settles with actionable target-specific detail

<!-- Expected canonical result after archive: `remote-control-api` will expose lifecycle-independent pure marks and move run eligibility to final Start/Retry admission. -->
