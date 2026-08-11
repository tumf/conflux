## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution marks and queue intent SHALL remain distinct projections. A standalone accepted operator mutation through Space, bulk `x`, or an equivalent shared mark command SHALL update the process-local mark set immediately without directly calling these operations.

`ExecutionMarkStore` SHALL be the concrete notification point used by both frontend service paths. TUI `apply_execution_mark` and API/coordinator `set_execution_mark` and `set_all_execution_marks` SHALL notify mark settlement after accepted standalone operator writes; Space and bulk `x` MUST NOT be rerouted through the API coordinator solely for this behavior.

When a live scheduler capable of dynamic queue admission exists, each such operator-originated mark notification SHALL replace one pending snapshot and restart one 10-second stability deadline. System mark revocation, a refused or no-op command, and marks written as part of Start admission MUST NOT arm or restart the deadline. When the deadline expires, the system SHALL read current marks and one coherent current reducer/operator view, then add each marked, loadable, ordinary `not queued` change to the current run through the explicit queue service.

Settlement SHALL be additive-only. Unmarking MUST NOT implicitly remove, stop, dequeue, or reschedule work in the active run. Settlement MUST NOT add active, admitted, already queued, error, retry-scoped, resolve-scoped, waiting, terminal, unavailable, or otherwise ineligible work and MUST NOT emit retry, resolve, cancellation, or stop intent.

#### Scenario: TUI and API mark entry points share settlement notification

- **GIVEN** TUI Space or bulk `x` writes through `apply_execution_mark`
- **AND** API mark commands write through `set_execution_mark` or `set_all_execution_marks`
- **WHEN** either service entry point accepts a standalone operator mark change
- **THEN** it notifies the same process-local mark-settlement mechanism
- **AND** neither frontend owns a timer
- **AND** the TUI command is not rerouted through the API coordinator

#### Scenario: Stable operator mark adds to DynamicQueue

- **GIVEN** a live scheduler capable of dynamic queue admission exists
- **AND** a visible loadable ordinary change is `not queued` and unmarked
- **WHEN** the user marks the change with Space or bulk `x`
- **AND** no later operator mark mutation occurs for 10 seconds
- **THEN** its execution mark becomes true immediately
- **AND** the explicit queue service performs one DynamicQueue `push`
- **AND** the scheduler receives a queue notification

#### Scenario: Stable unmark does not remove admitted work

- **GIVEN** a change is already queued or active in the current run
- **AND** the change carries an execution mark
- **WHEN** the user unmarks it with Space or bulk `x`
- **AND** the mark set remains stable for 10 seconds
- **THEN** its execution mark becomes false immediately
- **AND** no DynamicQueue `remove`, cancellation, stop, or dequeue request occurs
- **AND** current-run execution continues unchanged

#### Scenario: Rapid operator mark changes settle only the final state

- **GIVEN** a live dynamic-queue scheduler exists
- **AND** an accepted standalone operator mark mutation has armed the stability deadline
- **WHEN** another accepted standalone operator mark mutation occurs before 10 seconds elapse
- **THEN** the pending snapshot is replaced
- **AND** the deadline restarts
- **AND** no superseded snapshot mutates queue intent

#### Scenario: System revocation does not starve settlement

- **GIVEN** an operator mark mutation has armed the stability deadline
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

Space and bulk `x` SHALL toggle only process-local execution marks for visible non-terminal rows and MUST NOT directly modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status. When a live dynamic-queue scheduler exists, the shared stability coordinator MAY later add eligible marked ordinary work according to `Dynamic Queue Management`; frontend display code MUST NOT synthesize that queue state.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Space on either row SHALL toggle only the execution mark and MUST NOT modify `queue_status` or DynamicQueue. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once the user explicitly queues a `NotQueued` change through admitted orchestration, including settled mark admission, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Error-mode retry SHALL route only marked retry-eligible error targets and report other marked rows as excluded. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect. Mark writes performed as part of this admission MUST NOT arm delayed mark settlement.

#### Scenario: Queue and mark projections remain independent

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other

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
- **THEN** the TUI SHALL NOT emit cursor-local `ResolveMerge`
- **AND** normal orchestration MAY proceed for marked runnable work

#### Scenario: Deadline survives persistent idle presentation

- **GIVEN** a live persistent scheduler is Running when an operator mark mutation arms the deadline
- **WHEN** the scheduler parks and the TUI presents Select before the deadline expires
- **THEN** the deadline remains armed because the scheduler is still live
- **AND** stable eligible marked work is admitted when the deadline expires

#### Scenario: Process without live scheduler remains mark-only

- **GIVEN** no live scheduler capable of dynamic queue admission exists
- **WHEN** the user changes one or all execution marks in Select, Stopping, Stopped, or Error mode
- **THEN** mark state changes according to existing mode eligibility
- **AND** no stability deadline, DynamicQueue mutation, or reducer queue intent is created

<!-- Expected canonical result after archive: both historical duplicate `Queue State Synchronization` requirements converge to this complete contract without losing Start, wait-state, or queued-display guarantees. -->

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、表示中の non-terminal change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select、Running、Stopping、Stopped、および Error の全 execution mode で有効でなければならない（SHALL）。warning popup、confirmation、QR、またはその他の overlay が input を所有する場合は overlay がキーを消費し、Changes view の bulk mark を実行してはならない（MUST NOT）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。bulk mark は execution mark のみを即時変更し、frontend 自身が DynamicQueue、reducer queue intent、retry、resolve、cancellation、scheduler、hook、または process mode を変更してはならない（MUST NOT）。live dynamic-queue scheduler が存在し、operator-originated bulk mark set が10秒安定した場合に限り、共有 stability coordinator が `Dynamic Queue Management` に従って eligible ordinary `not queued` work を current run へ追加しなければならない（SHALL）。Archived、merged、pushed、および rejected rows は対象外でなければならない（SHALL）。

#### Scenario: 全 execution mode で未マークを全マークする

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible non-terminal change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all visible non-terminal changes SHALL be marked immediately
- **AND** frontend code produces no direct queue or runtime side effect

#### Scenario: すべてマーク済みの場合は全アンマークする

- **GIVEN** all visible non-terminal changes are marked
- **WHEN** the user triggers the bulk toggle in any execution mode
- **THEN** all visible non-terminal changes SHALL be unmarked
- **AND** work already admitted to the current run remains unchanged

#### Scenario: terminal row は bulk 対象外

- **GIVEN** visible rows include archived, merged, pushed, or rejected changes
- **WHEN** the user triggers the bulk toggle
- **THEN** terminal rows are excluded without a mark refusal warning
- **AND** every visible non-terminal row receives the common target mark state

#### Scenario: Stable bulk mark creates one additive queue plan

- **GIVEN** a live dynamic-queue scheduler exists
- **AND** multiple visible loadable ordinary changes are unmarked and `not queued`
- **WHEN** the user triggers bulk `x` and the resulting mark set remains unchanged for 10 seconds
- **THEN** all visible non-terminal changes are marked immediately
- **AND** eligible ordinary `not queued` changes are added through the explicit queue service
- **AND** no unmarked, active, waiting, error, retry, resolve, terminal, or ineligible work is changed
