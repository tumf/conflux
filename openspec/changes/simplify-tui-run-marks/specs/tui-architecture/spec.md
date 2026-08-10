## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution-mark mutation through Space or bulk `x` SHALL NOT implicitly call these operations. A mark expresses future run-target intent; it does not add, remove, stop, dequeue, or reschedule work in the active run.

#### Scenario: Marking during execution does not add to DynamicQueue

- **GIVEN** a run is active
- **AND** a visible non-terminal change is not marked
- **WHEN** the user marks the change with Space or bulk `x`
- **THEN** its execution mark becomes true
- **AND** no DynamicQueue `push` occurs
- **AND** no scheduler wake or run dispatch occurs

#### Scenario: Unmarking during execution does not remove admitted work

- **GIVEN** a change is already queued or active in the current run
- **AND** the change carries an execution mark
- **WHEN** the user unmarks it with Space or bulk `x`
- **THEN** its execution mark becomes false
- **AND** no DynamicQueue `remove`, cancellation, stop, or dequeue request occurs
- **AND** current-run execution continues unchanged

#### Scenario: Prevent duplicate additions

- **WHEN** an explicit queue service attempts to add an ID that already exists
- **THEN** the addition is rejected and queue state remains unchanged
- **AND** execution-mark state remains unchanged

#### Scenario: Remove non-existent ID

- **WHEN** an explicit queue service attempts to remove an ID that does not exist
- **THEN** no error occurs and queue state remains unchanged
- **AND** execution-mark state remains unchanged

### Requirement: Queue State Synchronization

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space and bulk `x` SHALL toggle only process-local execution marks for visible non-terminal rows and MUST NOT modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Space on either row SHALL toggle only the execution mark and MUST NOT modify `queue_status` or DynamicQueue. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once the user explicitly queues a `NotQueued` change through admitted orchestration, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Configured Start/F5 SHALL classify marked retry-eligible recovery rows from Ready/Select, Stopped, and process-wide Error rather than requiring process-wide Error mode. When retry and ordinary-start routes coexist, the invocation SHALL dispatch only retry routes with explicit-retry semantics, report ordinary rows as deferred, and preserve their marks for a later ordinary Start. When no retry route exists, ordinary startable rows SHALL use the existing Start route. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect.

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

#### Scenario: Ready re-mark and F5 retries a change-scoped error

- **GIVEN** `ProcessingError` moved change `alpha` to change-level `error`
- **AND** Core later projects Ready/Select without entering process-wide Error
- **AND** the operator re-marks `alpha`
- **WHEN** the configured Start/F5 control reaches final admission
- **THEN** `alpha` is routed through the existing typed retry path
- **AND** retry dispatch does not depend on process-wide Error mode

#### Scenario: Mixed recovery and ordinary marks preserve route semantics

- **GIVEN** marked targets contain retry-eligible `alpha` and ordinary `not queued` `beta`
- **WHEN** configured Start/F5 reaches final admission
- **THEN** this invocation dispatches only `alpha` with explicit-retry semantics
- **AND** `beta` is reported as deferred and remains marked
- **AND** a later configured Start MAY admit `beta` through the ordinary Start route

#### Scenario: Worktree-ineligible mark rejects atomically

- **GIVEN** marked targets include a worktree-ineligible change
- **WHEN** the configured start control reaches final admission
- **THEN** the complete request is rejected
- **AND** no scheduler, queue, retry-edge, or mode effect survives
- **AND** the diagnostic identifies that target and reason

#### Scenario: Non-startable status is excluded without blocking runnable work

- **GIVEN** marked targets include one runnable change and one currently non-startable status
- **AND** neither target violates the worktree eligibility fence
- **WHEN** Start reaches final admission
- **THEN** the runnable change is admitted
- **AND** the other target is excluded with target-specific diagnostic detail

#### Scenario: No runnable target rejects

- **GIVEN** every marked target is currently non-startable
- **WHEN** Start reaches final admission
- **THEN** admission is rejected before queue or scheduler effects
- **AND** the diagnostics identify the exclusions

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

<!-- Expected canonical result after archive: both historical duplicate `Queue State Synchronization` requirements will converge to this complete contract; archive validation must leave no duplicate legacy Space-to-queue semantics. -->

### Requirement: Active Change Input Lockout

`queue_status.is_active()` が true の change では、`@` 操作を受け付けてはならない（MUST NOT）。Space は active change でも process-local execution mark のみを変更し、停止要求、cancellation、queue mutation、または即時 `queue_status` 変更を発行してはならない（MUST NOT）。Per-change termination は独立した `K: kill` control を使用しなければならない（SHALL）。

#### Scenario: active change の Space は mark のみ変更する

- **GIVEN** the TUI is in Running mode
- **AND** cursor change has `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** only its process-local execution mark toggles
- **AND** no stop, cancellation, dequeue, or queue-status mutation occurs
- **AND** current active work continues

#### Scenario: active change で @ 操作は無効

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state remains unchanged
- **AND** the queue_status remains unchanged

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、表示中の non-terminal change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select、Running、Stopping、Stopped、および Error の全 execution mode で有効でなければならない（SHALL）。warning popup、confirmation、QR、またはその他の overlay が input を所有する場合は overlay がキーを消費し、Changes view の bulk mark を実行してはならない（MUST NOT）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。bulk mark は execution mark のみを変更し、DynamicQueue、reducer queue intent、retry、resolve、cancellation、scheduler、hook、または process mode を変更してはならない（MUST NOT）。Archived、merged、pushed、および rejected rows は対象外でなければならない（SHALL）。

#### Scenario: 全 execution mode で未マークを全マークする

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible non-terminal change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all visible non-terminal changes SHALL be marked
- **AND** no queue or runtime side effect occurs

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

<!-- Expected canonical result after archive: `tui-architecture` will separate execution marks from DynamicQueue, replace active Space stop with K, and converge duplicate synchronization requirements without dropping resolve or queued-display guarantees. -->
