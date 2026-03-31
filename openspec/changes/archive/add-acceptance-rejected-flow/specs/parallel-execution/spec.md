## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

システムはCLIとTUIの並列実行を扱う統一的な`ParallelRunService`を提供しなければならない（SHALL）。

サービスはイベント通知のためのコールバック機構を受け取り、TUIへ送るイベントは共有状態の更新より先に送信しなければならない（MUST）。これによりUI更新が共有状態のロック待ちで遅延しない。

サービスは以下をカプセル化すること：
- Git availability checking
- Change grouping by dependencies
- ParallelExecutor coordination
- Archiving of completed changes
- Rejection of blocked changes (acceptance Blocked → rejection flow)

ParallelRunService は、コミットツリーに存在しない change の除外と警告通知を CLI/TUI のどちらの経路でも同一ロジックで実行しなければならない（SHALL）。

Acceptance が `Blocked` を返した場合、ParallelRunService は rejection フロー（REJECTED.md 生成 → base コミット → resolve → worktree 削除）を実行し、`WorkspaceResult` で `error: None, rejected: Some(reason)` を返さなければならない（SHALL）。

#### Scenario: CLI uses ParallelRunService

- **WHEN** the CLI runs in parallel mode (`--parallel` flag)
- **THEN** the CLI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be logged to stdout via the callback mechanism

#### Scenario: TUI uses ParallelRunService

- **WHEN** the TUI runs in parallel mode
- **THEN** the TUI SHALL use `ParallelRunService` to execute changes
- **AND** events SHALL be forwarded to the TUI event channel via the callback mechanism
- **AND** event forwarding happens before shared state updates so Accepting can render promptly

#### Scenario: Acceptance Blocked triggers rejection flow in parallel mode

- **GIVEN** a change is executing in parallel mode
- **WHEN** acceptance returns `Blocked`
- **THEN** the rejection flow SHALL execute within the worktree context
- **AND** the worktree SHALL be deleted after rejection completes
- **AND** `WorkspaceResult.rejected` SHALL contain the rejection reason
- **AND** `WorkspaceResult.error` SHALL be `None`
