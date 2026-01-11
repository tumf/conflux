# Change: ParallelExecutor の責務分割

## Why

`parallel_executor.rs` は 1580 行と巨大で、以下の責務が混在している：

1. ワークスペースのライフサイクル管理 (`WorkspaceCleanupGuard`)
2. 並列実行のオーケストレーション
3. コンフリクト検出・解決ロジック
4. イベント送信

単一ファイルに責務が集中しているため、テストが困難で変更時の影響範囲が把握しにくい。

## What Changes

- `parallel_executor.rs` を責務ごとにサブモジュールへ分割
  - `parallel/mod.rs` - ParallelExecutor 本体（オーケストレーション）
  - `parallel/cleanup.rs` - WorkspaceCleanupGuard
  - `parallel/conflict.rs` - コンフリクト検出・解決
  - `parallel/events.rs` - ParallelEvent 定義とイベント送信
  - `parallel/executor.rs` - apply/archive の実行ロジック
- 各モジュールを単独でテスト可能に

## Impact

- 対象 specs: `code-maintenance`
- 対象コード:
  - `src/parallel_executor.rs` → `src/parallel/` ディレクトリに分割
  - `src/parallel_run_service.rs` - インポートパスの更新
  - `src/tui/parallel_event_bridge.rs` - インポートパスの更新
