## MODIFIED Requirements

### Requirement: Running Footer Progress Bar Display

The TUI Status panel SHALL display a progress bar for overall processing progress. Its aggregate target set SHALL be the unique union of successful completed work, work currently executing, and unfinished work carrying an execution mark. A row MUST contribute its stored `completed_tasks` and `total_tasks` at most once even when it belongs to more than one category. The progress calculation MUST NOT use execution-mark presence as the sole evidence that a change belongs to the aggregate.

Successful completed work SHALL include a change with reducer-observed archive completion or final success display `archived`, `merged`, or `pushed`. Current execution SHALL use the shared active-status vocabulary rather than a TUI-local phase list. Marked unfinished work SHALL include marked idle, queued, waiting, or retryable error rows. Rejected rows and rows that are unfinished, inactive, and unmarked SHALL NOT contribute.

The TUI SHALL sum the included rows' last known task counts and SHALL NOT synthesize full task completion solely from lifecycle status. A completion transition MUST NOT reduce the displayed progress by removing the completed row when its execution mark is revoked. An explicit change to the operator's marked target set MAY change the aggregate denominator and percentage.

#### Scenario: Completed task progress survives mark revocation

**Given**: A marked change contributes its last known task counts to the Status progress bar
**When**: The change reaches archive completion or final success and its execution mark is revoked
**Then**: The change remains in the aggregate as completed work
**And**: The completion transition alone does not reduce progress by dropping its task counts

#### Scenario: Archive-complete post-archive work remains included

**Given**: Reducer archive completion has been observed for a change
**When**: Its display status is `resolving`, `resolve pending`, or `merge wait`
**Then**: Its last known task counts remain in the aggregate
**And**: No execution mark is required for inclusion

#### Scenario: Active unmarked work is included

**Given**: A change has a display status recognized by the shared active-status vocabulary
**And**: The change has no execution mark
**When**: The Status panel is rendered
**Then**: Its stored task counts contribute to overall progress

#### Scenario: Marked unfinished work is included

**Given**: A non-rejected change is unfinished and not actively executing
**And**: The change carries an execution mark
**When**: The Status panel is rendered
**Then**: Its stored task counts contribute to the aggregate
**And**: This includes a marked retryable error row

#### Scenario: Overlapping categories count once

**Given**: A change satisfies more than one of completed, active, or execution-marked inclusion
**When**: Overall progress is calculated
**Then**: The change contributes its task counts exactly once

#### Scenario: Mixed lifecycle and mark states show overall progress

**Given**: An unmarked merged change has `3/3` tasks
**And**: An unmarked applying change has `1/4` tasks
**And**: A marked not-queued change has `0/2` tasks
**And**: An unmarked not-queued change has `0/5` tasks
**When**: The Status panel is rendered
**Then**: The aggregate is `4/9`
**And**: The displayed percentage is `44.4%`

#### Scenario: Unmarked idle and rejected rows are excluded

**Given**: A row is unfinished, inactive, and unmarked, or its final outcome is rejected
**When**: Overall progress is calculated
**Then**: The row contributes neither completed nor total tasks

#### Scenario: Included rows have zero total tasks

**Given**: Every included row has zero total tasks
**When**: The Status panel is rendered
**Then**: The TUI does not divide by zero
**And**: Existing no-task Status rendering remains unchanged
