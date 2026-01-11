# Change: ParallelExecutor の VCS-agnostic 化実装

## Why

現在、TUI で `[parallel:3:git]` と表示されるにもかかわらず、`ParallelExecutor` 内部で jj コマンドがハードコードされており、Git モードでは実際に動作しない。既存の `parallel-execution` スペックで定義された VCS バックエンド抽象化要件を実装し、Git リポジトリでも parallel 実行を可能にする必要がある。

## What Changes

- `ParallelExecutor` の `workspace_manager` を `JjWorkspaceManager` から `Box<dyn WorkspaceManager>` に変更
- `WorkspaceManager` trait に不足していたメソッドを追加（snapshot、describe、revision取得など）
- `JjWorkspaceManager` と `GitWorkspaceManager` に新しい trait メソッドを実装
- `WorkspaceCleanupGuard` を VCS-agnostic に修正
- `execute_apply_in_workspace` 内の jj コマンド直接呼び出しを VCS 分岐に変更
- conflict resolution のプロンプトを VCS 固有のものに変更

## Impact

- Affected specs: `parallel-execution`（既存要件の実装、スペック変更なし）
- Affected code:
  - `src/parallel_executor.rs` - メイン修正対象
  - `src/vcs_backend.rs` - trait 拡張
  - `src/jj_workspace.rs` - 新メソッド実装
  - `src/git_workspace.rs` - 新メソッド実装
  - `src/parallel_run_service.rs` - deprecation 対応
