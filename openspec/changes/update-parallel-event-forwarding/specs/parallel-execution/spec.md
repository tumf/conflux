## MODIFIED Requirements
### Requirement: Shared Parallel Orchestration Service
システムはCLIとTUIの並列実行を扱う統一的な`ParallelRunService`を提供しなければならない（SHALL）。

サービスはイベント通知のためのコールバック機構を受け取り、TUIへ送るイベントは共有状態の更新より先に送信しなければならない（MUST）。これによりUI更新が共有状態のロック待ちで遅延しない。

サービスは以下をカプセル化すること：
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes

#### Scenario: CLI uses ParallelRunService
- **WHEN** the CLI runs in parallel mode (`--parallel` flag)
- **THEN** the CLI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be logged to stdout via the callback mechanism

#### Scenario: TUI uses ParallelRunService
- **WHEN** the TUI runs in parallel mode
- **THEN** the TUI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be forwarded to the TUI event channel via the callback mechanism
- **AND** event forwarding happens before shared state updates so Accepting can render promptly

#### Scenario: TUI event forwarding precedes shared state update
- **GIVEN** `ParallelEvent::AcceptanceStarted` is processed by the forwarder
- **WHEN** the event is forwarded to the TUI
- **THEN** the TUI event channel receives the event before the shared state write lock is acquired
- **AND** the change status can transition to `Accepting` while acceptance is running

#### Scenario: Parallel mode requires git repository
- **WHEN** parallel execution is requested
- **AND** a `.git` directory does not exist
- **THEN** `ParallelRunService` SHALL return an error indicating a git repository is required
- **AND** no parallel execution is started
