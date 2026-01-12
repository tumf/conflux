# Change: Apply ロジックの共通化と VCS 操作の統一

## Why

### Apply ロジックの重複

Apply 処理のロジックが3箇所で重複している：

1. **CLI Serial**: `src/orchestrator.rs::apply_change()` - シンプルな1回の apply
2. **TUI Serial**: `src/tui/orchestrator.rs::run_orchestrator()` 内のインラインコード - ストリーミング出力付き
3. **Parallel**: `src/parallel/executor.rs::execute_apply_in_workspace()` - 反復ループ付き (~300行)

共通の処理：
- apply コマンドの実行
- 進捗チェック（completed/total タスク）
- 成功/失敗の判定

相違点：
- CLI Serial: フック実行あり、反復なし
- TUI Serial: ストリーミング出力、キャンセレーション対応
- Parallel: 最大50回の反復、プログレスコミット作成

### VCS 操作の重複

`parallel/executor.rs` 内で git/jj コマンドが直接実行されているが、既存の `src/vcs/` モジュールの `WorkspaceManager` trait が十分に活用されていない。

## What Changes

- 新規: `src/execution/apply.rs` - 共通 Apply ロジック
  - `check_task_progress()` の活用拡大
  - `ApplyIterator` - 反復 apply のための共通構造体
- 修正: `src/parallel/executor.rs` - VCS 操作を `WorkspaceManager` 経由に変更
- 修正: 各モードで共通関数を使用

## Impact

- Affected specs: code-maintenance, parallel-execution
- Affected code:
  - 新規: `src/execution/apply.rs`
  - 修正: `src/execution/mod.rs`
  - 修正: `src/parallel/executor.rs`
  - 修正: `src/tui/orchestrator.rs`
- **BREAKING**: なし（内部リファクタリング）

## 依存関係

- 前提: `create-execution-module` の完了
- 後続: `add-parallel-hooks` で hooks サポートを追加
