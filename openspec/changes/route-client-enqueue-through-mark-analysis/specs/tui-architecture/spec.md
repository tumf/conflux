## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution marks and queue intent SHALL remain distinct projections. An accepted human-target-selection mutation through TUI Space, bulk `x`, CLI/MCP enqueue, or an equivalent shared mark command SHALL update the process-local mark set immediately without directly calling these operations. CLI/MCP enqueue MUST use the same target-scoped mark settlement notification as TUI mark input and MUST NOT bypass analyze by submitting direct queue intent for ordinary work.

`ExecutionMarkStore` SHALL be the concrete notification point used by all frontend service paths. TUI `apply_execution_mark`, API/coordinator `set_execution_mark` and `set_all_execution_marks`, and ordinary CLI/MCP enqueue SHALL notify mark settlement after accepted standalone target-scoped writes; adapters MUST NOT reimplement settlement, analyze, capacity, or queue mutation.

When a live scheduler capable of dynamic queue admission exists, each such target-selection notification SHALL replace one pending snapshot and restart one 10-second stability deadline. System mark revocation, a refused or no-op command, and marks written as part of idle-owner Start admission MUST NOT arm or restart the deadline. When the deadline expires, the system SHALL read current marks and one coherent current reducer/operator view, analyze admissible work and capacity through the existing shared services, then add each selected, loadable, ordinary `not queued` change admitted by that analysis to the current run through the explicit queue service.

Settlement SHALL be additive-only. Unmarking MUST NOT implicitly remove, stop, dequeue, or reschedule work in the active run. Settlement MUST NOT add active, admitted, already queued, error, retry-scoped, resolve-scoped, waiting, terminal, unavailable, or otherwise ineligible work and MUST NOT emit retry, resolve, cancellation, or stop intent.

#### Scenario: TUI, API, CLI, and MCP mark entry points share settlement notification

- **GIVEN** TUI Space or bulk `x` writes through `apply_execution_mark`
- **AND** API mark commands write through `set_execution_mark` or `set_all_execution_marks`
- **AND** CLI or MCP enqueue requests ordinary live-owner work
- **WHEN** any service entry point accepts a standalone target-scoped execution-mark change
- **THEN** it notifies the same process-local mark-settlement/analyze mechanism
- **AND** the requested target is added without replacing unrelated marks
- **AND** no frontend or adapter owns a timer or analyze implementation
- **AND** no client path submits direct queue intent for that ordinary mark request

#### Scenario: Stable execution mark adds through analysis to DynamicQueue

- **GIVEN** a live scheduler capable of dynamic queue admission exists
- **AND** a visible loadable ordinary change is `not queued` and unmarked
- **WHEN** TUI, API, CLI, or MCP accepts a target-scoped mark request
- **AND** no later target-selection mutation occurs for 10 seconds
- **AND** existing analysis finds the change admissible within current capacity
- **THEN** its execution mark becomes true immediately
- **AND** the explicit queue service performs one DynamicQueue `push`
- **AND** the scheduler receives a queue notification

#### Scenario: Analysis refusal preserves selection without queue mutation

- **GIVEN** a live scheduler accepts an ordinary execution mark
- **WHEN** shared analysis finds no capacity or the target is not currently admissible
- **THEN** the execution mark remains visible unless another existing reconciliation rule revokes it
- **AND** no queue intent or DynamicQueue entry is synthesized by the client
- **AND** CLI/MCP enqueue does not report admission from the mark alone

#### Scenario: Stable unmark does not remove admitted work

- **GIVEN** a change is already queued or active in the current run
- **AND** the change carries an execution mark
- **WHEN** the user unmarks it with Space or bulk `x`
- **AND** the mark set remains stable for 10 seconds
- **THEN** its execution mark becomes false immediately
- **AND** no DynamicQueue `remove`, cancellation, stop, or dequeue request occurs
- **AND** current-run execution continues unchanged

#### Scenario: Rapid target-selection changes settle only the final state

- **GIVEN** a live dynamic-queue scheduler exists
- **AND** an accepted standalone target-selection mutation has armed the stability deadline
- **WHEN** another accepted target-selection mutation occurs before 10 seconds elapse
- **THEN** the pending snapshot is replaced
- **AND** the deadline restarts
- **AND** no superseded snapshot mutates queue intent

#### Scenario: System revocation does not starve settlement

- **GIVEN** an operator target-selection mutation has armed the stability deadline
- **WHEN** lifecycle reconciliation revokes another execution mark before the deadline
- **THEN** the current mark set reflects the revocation
- **AND** the existing deadline is not restarted
- **AND** settlement classifies the current mark set when that deadline expires

#### Scenario: Prevent duplicate additions

- **WHEN** settlement or another explicit queue service attempts to add an ID that already exists
- **THEN** the addition is rejected and queue state remains unchanged
- **AND** execution-mark state remains unchanged

#### Scenario: Remove non-existent ID

- **WHEN** an explicit queue service attempts to remove an ID that does not exist
- **THEN** no error occurs and queue state remains unchanged
- **AND** execution-mark state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space, bulk `x`, and ordinary CLI/MCP enqueue SHALL write only process-local execution marks for visible non-terminal rows and MUST NOT directly modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status. When a live dynamic-queue scheduler exists, the shared stability coordinator MAY later add eligible marked ordinary work according to `Dynamic Queue Management`; frontend display code and client adapters MUST NOT synthesize that queue state.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Space on either row SHALL toggle only the execution mark and MUST NOT modify `queue_status` or DynamicQueue. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once shared analysis explicitly queues a `NotQueued` change, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Error-mode retry SHALL route only marked retry-eligible error targets and report other marked rows as excluded. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect. Mark writes performed as part of idle-owner Start admission MUST NOT arm delayed mark settlement.

#### Scenario: Queue and mark projections remain independent

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked due to an explicit lower-level queue-control caller or existing admitted state
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other
- **AND** ordinary CLI/MCP enqueue cannot create this state by bypassing mark analysis

#### Scenario: Ordinary client enqueue preserves human selection

- **GIVEN** unrelated change `beta` is execution-marked and ordinary change `alpha` is unmarked
- **WHEN** CLI or MCP enqueue requests `alpha`
- **THEN** `alpha` and `beta` are both marked in the shared store
- **AND** no existing mark is cleared or replaced
- **AND** queue presentation changes only after shared analysis/admission

#### Scenario: Marking wait and error rows has no workflow side effect

- **GIVEN** a visible non-terminal row is in error, merge wait, resolve pending, or another non-active wait state
- **WHEN** the user toggles its execution mark
- **THEN** only the process-local mark changes
- **AND** no retry, resolve, or queue intent is created

#### Scenario: Worktree-ineligible mark rejects atomically

- **GIVEN** marked targets include a worktree-ineligible change
- **WHEN** the configured start control reaches final admission
- **THEN** the complete request is rejected
- **AND** no scheduler, queue, retry-edge, mode, or delayed-settlement effect survives
- **AND** the diagnostic identifies that target and reason

#### Scenario: Non-startable status is excluded without blocking runnable work

- **GIVEN** marked targets include one runnable change and one currently non-startable status
- **AND** neither target violates the worktree eligibility fence
- **WHEN** Start reaches final admission
- **THEN** the runnable change is admitted
- **AND** the other target is excluded with target-specific diagnostic detail
- **AND** Start-admission mark writes do not create a second delayed admission

#### Scenario: No runnable target rejects

- **GIVEN** every marked target is currently non-startable
- **WHEN** Start reaches final admission
- **THEN** admission is rejected before queue or scheduler effects
- **AND** the diagnostics identify the exclusions
- **AND** no delayed mark settlement is armed by the rejected request

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** cursor change `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work
- **WHEN** a configured start key is pressed
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration MAY admit `beta`

#### Scenario: Default start keys remain app-level controls

- **GIVEN** default start keys are `F5` and `!`
- **AND** the cursor is on a `MergeWait` row
- **WHEN** the user presses `F5` or `!`
- **THEN** the key SHALL be handled by normal app-level start orchestration
- **AND** it SHALL NOT invoke cursor-local resolve behavior
