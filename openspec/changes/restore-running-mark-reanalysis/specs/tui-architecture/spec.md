## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution marks and queue intent SHALL remain distinct projections. In Running mode, however, accepted Space, bulk `x`, and equivalent shared execution-mark mutations SHALL update one process-local pending mark snapshot. When that snapshot has remained unchanged for 10 seconds, the system SHALL reconcile eligible marked ordinary `not queued` changes into the current run through the explicit queue service.

A later stable unmark SHALL remove only still-pending ordinary queue membership that the same process can prove was created by mark reconciliation. It MUST NOT remove explicit queue membership, cancel active or admitted work, or alter retry, resolve, merge-wait, terminal, or otherwise ineligible work.

<!-- Expected canonical result after archive: Running marks remain distinct from queue intent but settle after 10 seconds into safe current-run queue additions and provenance-bound pending removals. -->

#### Scenario: Stable Running mark adds ordinary work

- **GIVEN** a run is active
- **AND** a visible loadable ordinary change is `not queued` and unmarked
- **WHEN** the user marks the change and the mark set remains unchanged for 10 seconds
- **THEN** its execution mark remains true
- **AND** the existing explicit queue service adds reducer queue intent and DynamicQueue membership exactly once
- **AND** the scheduler receives a queue notification

#### Scenario: Rapid mark changes settle only the final snapshot

- **GIVEN** a run is active
- **AND** one or more accepted mark mutations have started the stability deadline
- **WHEN** another accepted mark mutation occurs before 10 seconds elapse
- **THEN** the pending snapshot is replaced with the latest mark set
- **AND** the 10-second deadline restarts
- **AND** no superseded snapshot mutates queue intent

#### Scenario: Stable unmark removes only mark-created pending work

- **GIVEN** a change remains pending in the ordinary queue
- **AND** the current process recorded that membership as created by stable mark reconciliation
- **WHEN** the change is unmarked and the final mark set remains unchanged for 10 seconds
- **THEN** the existing explicit queue service removes that pending membership
- **AND** no cancellation or stop request occurs

#### Scenario: Unmarking does not revoke other work

- **GIVEN** a marked change is active, admitted, explicitly queued, retry-scoped, resolve-scoped, merge-waiting, terminal, or lacks mark-reconciliation provenance
- **WHEN** the user unmarks it and the final mark set remains unchanged for 10 seconds
- **THEN** its execution mark becomes false
- **AND** current-run queue, activity, retry, resolve, cancellation, and stop state remain unchanged

#### Scenario: Restart discards unsettled mark admission state

- **GIVEN** a Running mark snapshot is awaiting its stability deadline or has process-local removal provenance
- **WHEN** Conflux restarts
- **THEN** the pending snapshot, deadline, and provenance are absent
- **AND** next-action routing is recomputed from workspace and Git state

### Requirement: Queue State Synchronization

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space and bulk `x` SHALL update process-local execution marks immediately. In Running mode only, the shared mark stability coordinator MAY later create or remove ordinary pending queue intent according to `Dynamic Queue Management`; frontend display code MUST NOT synthesize queue state directly from a mark. In Select, Stopping, Stopped, and Error modes, mark mutation MUST NOT create current-run queue intent.

`ResolveWait` is scheduler-owned resolve retry work and `MergeWait` is merge-resolution work. Marking either row SHALL NOT modify `queue_status`, DynamicQueue, retry, or resolve intent. `@` SHALL remain ignored. The TUI MUST continue to display `ResolveWait` as `resolve pending`.

In parallel mode, once stable reconciliation or another admitted orchestration path queues a `NotQueued` change, refresh-derived reconciliation MUST preserve the queued display state until execution starts, startup is explicitly rejected, or an explicit dequeue occurs. Auto-refresh, reducer synchronization, and eligibility reconciliation MUST NOT regress it to `not queued` before backend analysis or dispatch.

Configured start keys SHALL remain app-level orchestration controls and MUST NOT emit cursor-local `ResolveMerge` or move a cursor `MergeWait` row to `resolve pending`.

At final admission, run control SHALL read one coherent mark snapshot. A worktree-ineligible marked target SHALL reject the complete request with target-specific diagnostics. Other currently non-startable statuses SHALL be excluded from that admission with target-specific diagnostics; if no runnable target remains, admission SHALL reject. Error-mode retry SHALL route only marked retry-eligible error targets and report other marked rows as excluded. Rejection MUST leave no partial queue, scheduler, retry-edge, or mode effect.

<!-- Expected canonical result after archive: queue and mark projections remain distinct while Running-mode stable reconciliation is the only implicit bridge into ordinary current-run queue intent. -->

#### Scenario: Queue and mark projections remain distinct

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other

#### Scenario: Running mark waits for settlement

- **GIVEN** a loadable ordinary Running-mode row is marked but remains `not queued`
- **WHEN** less than 10 seconds have elapsed since the latest accepted mark change
- **THEN** the row remains marked and `not queued`
- **AND** no DynamicQueue mutation or scheduler wake occurs

#### Scenario: Wait and error marks have no workflow side effect

- **GIVEN** a visible non-terminal row is in error, merge wait, resolve pending, or another non-ordinary wait state
- **WHEN** the user toggles its execution mark and the mark set settles
- **THEN** only the process-local mark changes
- **AND** no retry, resolve, queue, stop, or cancellation intent is created

#### Scenario: Non-Running marks remain future-run intent

- **GIVEN** the process is in Select, Stopping, Stopped, or Error mode
- **WHEN** the user changes one or all execution marks
- **THEN** mark state changes according to existing mode eligibility
- **AND** no current-run DynamicQueue or reducer queue intent is created by mark reconciliation

#### Scenario: Worktree-ineligible mark rejects atomically at Start

- **GIVEN** marked targets include a worktree-ineligible change
- **WHEN** the configured start control reaches final admission
- **THEN** the complete request is rejected
- **AND** no scheduler, queue, retry-edge, or mode effect survives
- **AND** the diagnostic identifies that target and reason

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、表示中の non-terminal change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select、Running、Stopping、Stopped、および Error の全 execution mode で有効でなければならない（SHALL）。warning popup、confirmation、QR、またはその他の overlay が input を所有する場合は overlay がキーを消費し、Changes view の bulk mark を実行してはならない（MUST NOT）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。bulk toggle は execution mark を即時変更するが、frontend 自身が DynamicQueue、reducer queue intent、retry、resolve、cancellation、scheduler、hook、または process mode を直接変更してはならない（MUST NOT）。Running mode では、変更後の全 mark set が10秒安定した場合に限り、共有 mark stability coordinator が `Dynamic Queue Management` に従って eligible ordinary pending work を current-run queue と同期しなければならない（SHALL）。Archived、merged、pushed、および rejected rows は対象外でなければならない（SHALL）。

<!-- Expected canonical result after archive: bulk marks stay atomic and frontend-local side-effect free, while the shared Running-mode coordinator settles the final bulk snapshot into safe current-run queue intent. -->

#### Scenario: Running bulk mark settles as one queue plan

- **GIVEN** the TUI is in Running mode
- **AND** multiple visible eligible ordinary changes are unmarked and `not queued`
- **WHEN** the user triggers bulk `x` and the resulting mark set remains unchanged for 10 seconds
- **THEN** all eligible non-terminal rows are marked immediately
- **AND** eligible ordinary `not queued` rows are admitted as one settled queue plan
- **AND** the scheduler may analyze the batch in one notification cycle

#### Scenario: Running bulk unmark preserves admitted work

- **GIVEN** all visible non-terminal changes are marked
- **AND** some rows are active, explicitly queued, waiting, or terminal while others remain mark-created pending work
- **WHEN** the user triggers bulk unmark and the resulting mark set remains unchanged for 10 seconds
- **THEN** eligible rows are unmarked
- **AND** only mark-created still-pending ordinary queue memberships may be removed
- **AND** active, explicitly queued, waiting, retry, resolve, and terminal work remains unchanged

#### Scenario: Non-Running bulk toggle has no queue side effect

- **GIVEN** the TUI is in Select, Stopping, Stopped, or Error mode
- **WHEN** the user triggers the bulk toggle
- **THEN** eligible execution marks receive the common target state
- **AND** no queue or runtime side effect occurs

#### Scenario: terminal row is excluded from bulk toggle

- **GIVEN** visible rows include archived, merged, pushed, or rejected changes
- **WHEN** the user triggers the bulk toggle
- **THEN** terminal rows are excluded without a mark refusal warning
- **AND** every eligible visible non-terminal row receives the common target mark state
