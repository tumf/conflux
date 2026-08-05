## MODIFIED Requirements

### Requirement: Unified Orchestration Module

The codebase SHALL expose one shared cumulative worktree orchestration flow to executable CLI and TUI frontends. It SHALL NOT retain `SerialRunService`, a repository-root execution loop, or a serial fallback.

#### Scenario: CLI and TUI use one scheduler

- **WHEN** CLI or local TUI starts selected changes
- **THEN** both frontends dispatch the cumulative worktree scheduler
- **AND** frontend-specific output and UI updates remain handled by adapters

#### Scenario: Single change does not select another service

- **WHEN** exactly one eligible change is selected
- **THEN** the same worktree scheduler executes it with one worker
- **AND** no alternate serial service is constructed

### Requirement: Execution Module Foundation

The system SHALL provide execution contexts and result types used by managed-worktree orchestration. Change-level apply and archive execution context SHALL carry managed-worktree identity; run-level context MAY remain workspace-neutral.

#### Scenario: Change execution context

- **GIVEN** a change ID, configuration, managed workspace path, and group identity are available
- **WHEN** a change-level execution context is created
- **THEN** the context contains the managed workspace path and group identity
- **AND** no serial constructor can produce a workspace-free change execution context

#### Scenario: ExecutionResult state transition

- **GIVEN** execution processing has started
- **WHEN** processing completes
- **THEN** `ExecutionResult::Success`, `ExecutionResult::Failed`, or `ExecutionResult::Cancelled` is returned

### Requirement: Common Apply Iteration Logic

The system SHALL manage repeated apply commands through the common apply loop used by the cumulative worktree executor. No serial-specific apply history owner or repository-root apply loop SHALL remain.

#### Scenario: Single apply execution

- **GIVEN** change ID `my-change`, a managed workspace, and an apply command
- **WHEN** the executor invokes `execute_apply_iteration()`
- **THEN** the apply command runs in the managed workspace
- **AND** post-execution progress is returned

#### Scenario: Repeated apply execution

- **GIVEN** `max_iterations = 50`
- **WHEN** apply repeats until tasks reach 100 percent
- **THEN** progress is checked after each iteration
- **AND** execution stops when complete or when the iteration budget is exhausted

### Requirement: Serial/Parallel 実行フローの共有化

システムは apply・archive・進捗更新を cumulative worktree orchestration の共有関数へ集約し、実行モード別の分岐を保持してはならない（MUST NOT）。

#### Scenario: 単一の実行フローを利用する

- **WHEN** 1件または複数件のchangeを実行する
- **THEN** apply・archive・進捗更新は同じmanaged-worktree共有関数経由で実行される
- **AND** repository-root serial fallbackは存在しない

#### Scenario: frontend固有の差分が分離される

- **WHEN** CLIまたはTUI固有の出力やイベント送信を実装する
- **THEN** 共有関数は実行フローのみを扱う
- **AND** 出力と表示の責務はfrontend adapterへ分離される
