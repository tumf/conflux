# Change: イベントシステムの統一

## Why

現在、2つの異なるイベントシステムが存在する：

1. **`ParallelEvent`** (`src/parallel/events.rs`): parallel mode 用、25+ のイベントバリアント
2. **`OrchestratorEvent`** (`src/tui/events.rs`): TUI 用、ProcessingStarted, ChangeArchived など

これらの間には `src/tui/parallel_event_bridge.rs` というブリッジレイヤーが存在し、`ParallelEvent` を `OrchestratorEvent` に変換している。

この2重構造は以下の問題を引き起こす：

1. **コードの重複**: 同じ概念（ApplyStarted, ProgressUpdated など）が2つの型で定義
2. **変換オーバーヘッド**: ブリッジレイヤーでの変換処理
3. **不一致のリスク**: 新しいイベントを追加する際に両方の型とブリッジを更新する必要
4. **複雑性**: 開発者が2つのイベントシステムを理解する必要

## What Changes

- 新規: `src/events.rs` - 統一イベント型 `ExecutionEvent`
- 修正: `src/parallel/` - `ExecutionEvent` を使用するよう変更
- 修正: `src/tui/` - `ExecutionEvent` を使用するよう変更
- 削除: `src/tui/parallel_event_bridge.rs` - 不要になるため削除
- 削除: `src/parallel/events.rs` の `ParallelEvent` - 統一イベントに置き換え

## Impact

- Affected specs: tui-architecture, parallel-execution
- Affected code:
  - 新規: `src/events.rs`
  - 修正: `src/parallel/mod.rs`, `src/parallel/executor.rs`
  - 修正: `src/tui/events.rs`, `src/tui/runner.rs`, `src/tui/orchestrator.rs`
  - 削除: `src/tui/parallel_event_bridge.rs`
- **BREAKING**: なし（内部リファクタリング）

## 依存関係

- 前提: `add-parallel-hooks` の完了（hooks イベントも統一イベントに含める必要があるため）
