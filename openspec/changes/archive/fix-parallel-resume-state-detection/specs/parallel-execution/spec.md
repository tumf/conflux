## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

システムはCLIとTUIの並列実行を扱う統一的な`ParallelRunService`を提供しなければならない（SHALL）。

サービスはイベント通知のためのコールバック機構を受け取り、TUIへ送るイベントは共有状態の更新より先に送信しなければならない（MUST）。これによりUI更新が共有状態のロック待ちで遅延しない。

サービスは以下をカプセル化すること：
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes

ParallelRunService は、コミットツリーに存在しない change の除外と警告通知を CLI/TUI のどちらの経路でも同一ロジックで実行しなければならない（SHALL）。

また、既存 workspace を再利用して resume する場合、呼び出し元が fresh start と resume を区別できるよう、検出した workspace state を user-visible reporting に利用できる情報として保持または伝搬しなければならない（MUST）。

#### Scenario: caller can distinguish reused workspace resume from fresh start

- **GIVEN** parallel execution is requested for a change with an existing reusable workspace
- **WHEN** the service detects the workspace state before dispatch
- **THEN** the service preserves enough information for the caller to know the change resumed from an existing workspace
- **AND** the caller is not forced to present the run as a fresh start

### Requirement: Workspace state detection guides resume behavior

Workspace reuse SHALL follow the detected `WorkspaceState` and MUST NOT silently treat an archived-or-beyond workspace as a fresh apply start.

#### Scenario: archived workspace is not presented as a fresh apply start

- **GIVEN** a reusable workspace exists for a change
- **AND** workspace state detection returns `WorkspaceState::Archived` or a later terminal state
- **WHEN** parallel execution decides how to handle that workspace
- **THEN** the change follows the resume behavior for that detected state
- **AND** user-visible reporting does not imply that apply started from scratch

#### Scenario: archived tasks fallback does not hide resume origin

- **GIVEN** a reused workspace contains archived `tasks.md` that reports all tasks complete
- **WHEN** apply/resume logic checks task progress
- **THEN** the system may use that progress only according to the detected resume state rules
- **AND** the caller can still tell that the result came from workspace reuse rather than a fresh apply start
