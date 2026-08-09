## MODIFIED Requirements

### Requirement: Dynamic Queue Management

The system SHALL provide explicit services for dynamically adding and removing changes to/from the queue during execution.

DynamicQueue SHALL support the following operations:
- `push(id)`: Add a change ID to the queue (with duplicate check)
- `pop()`: Retrieve the next change ID from the queue
- `remove(id)`: Remove a specified change ID from the queue

Execution-mark mutation SHALL NOT implicitly call these operations. A mark expresses future run-target intent; it does not add, remove, stop, dequeue, or reschedule work in the active run.

#### Scenario: Marking during execution does not add to DynamicQueue

- **GIVEN** a run is active
- **AND** a visible pre-archive change is not marked
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

#### Scenario: Explicit queue operations retain duplicate and missing-ID behavior

- **WHEN** an explicit queue service attempts to add an existing ID or remove a missing ID
- **THEN** duplicate additions are rejected and missing removals are no-ops
- **AND** execution-mark state is unchanged

### Requirement: Queue State Synchronization

The system SHALL synchronize displayed queue state with DynamicQueue and reducer queue intent independently from execution marks.

Space and bulk `x` SHALL toggle only process-local execution marks for visible pre-archive rows and MUST NOT modify DynamicQueue, reducer queue intent, retry/resolve intent, active execution, cancellation, or process mode. Queue status MUST NOT synthesize an execution mark, and mark state MUST NOT synthesize queue status.

The configured start keybindings SHALL remain app-level orchestration controls. At final admission, run control SHALL read the current process-local marks and current reducer/worktree eligibility, then either dispatch a valid target set or reject without partial queue or scheduler effects.

#### Scenario: Queue and mark projections remain independent

- **GIVEN** a change is reducer-visible as `queued` but is not execution-marked
- **WHEN** frontend state is synchronized
- **THEN** the row remains queued and unmarked
- **AND** neither projection overwrites the other

#### Scenario: Marking wait and error rows has no workflow side effect

- **GIVEN** a visible pre-archive row is in error, merge wait, resolve pending, or another non-active wait state
- **WHEN** the user toggles its execution mark
- **THEN** only the process-local mark changes
- **AND** no retry, resolve, or queue intent is created

#### Scenario: Final admission rejects invalid marked targets atomically

- **GIVEN** one or more changes are execution-marked
- **AND** current reducer or worktree facts make the requested run target set invalid
- **WHEN** the configured start control reaches final admission
- **THEN** no scheduler is prepared or activated
- **AND** no queue, retry-edge, or mode mutation survives
- **AND** the diagnostic identifies the invalid marked target and current reason

<!-- Expected canonical result after archive: `tui-architecture` will separate execution marks from DynamicQueue and move current-state eligibility checks to run admission. -->

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、表示中の pre-archive change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select、Running、Stopping、Stopped、および Error の全 execution mode で有効でなければならない（SHALL）。warning popup、confirmation、QR、またはその他の overlay が input を所有する場合は overlay がキーを消費し、Changes view の bulk mark を実行してはならない（MUST NOT）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。bulk mark は execution mark のみを変更し、DynamicQueue、reducer queue intent、retry、resolve、cancellation、scheduler、hook、または process mode を変更してはならない（MUST NOT）。

#### Scenario: 全 execution mode で未マークを全マークする

- **GIVEN** the TUI is in Select, Running, Stopping, Stopped, or Error mode
- **AND** at least one visible pre-archive change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all visible pre-archive changes SHALL be marked
- **AND** no queue or runtime side effect occurs

#### Scenario: すべてマーク済みの場合は全アンマークする

- **GIVEN** all visible pre-archive changes are marked
- **WHEN** the user triggers the bulk toggle in any execution mode
- **THEN** all visible pre-archive changes SHALL be unmarked
- **AND** work already admitted to the current run remains unchanged

#### Scenario: post-archive row は bulk 対象外

- **GIVEN** visible rows include archived, merged, or pushed changes
- **WHEN** the user triggers the bulk toggle
- **THEN** post-archive rows are excluded without a mark refusal warning
- **AND** every visible pre-archive row receives the common target mark state

<!-- Expected canonical result after archive: `tui-architecture` will make bulk marks lifecycle-independent, mark-only, and post-archive-excluding. -->
