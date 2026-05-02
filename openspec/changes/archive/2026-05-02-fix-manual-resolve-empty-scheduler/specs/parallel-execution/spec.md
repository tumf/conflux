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

When invoked by a loop-based frontend with no active changes but with reducer-owned `ResolveWait` work, `ParallelRunService` SHALL start scheduler-owned retry processing instead of returning before the executor can synchronize reducer state.

#### Scenario: empty active changes with resolve wait enters scheduler retry

**Given**: `ParallelRunService` is invoked with an empty active change list from a loop-based frontend
**And**: the shared orchestrator state contains change `alpha` in `ResolveWait`
**When**: parallel run startup evaluates committed active changes
**Then**: the service does not return solely because the active change list is empty
**And**: the executor synchronizes `ResolveWait` from shared state
**And**: scheduler-owned merge retry dispatch is attempted for `alpha`

#### Scenario: normal committed-change filtering still applies to active changes

**Given**: `ParallelRunService` is invoked with active changes to apply
**When**: one active change is not present in the HEAD commit tree or has uncommitted files under `openspec/changes/<change_id>/`
**Then**: that active change is skipped with the existing warning/rejection events
**And**: this filtering does not suppress separate reducer-owned `ResolveWait` retry work when such work exists
