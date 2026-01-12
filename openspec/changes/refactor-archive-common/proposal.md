# Change: アーカイブロジックの共通化

## Why

現在、アーカイブ処理のロジックが2箇所で重複している：

1. **TUI Serial**: `src/tui/orchestrator.rs::archive_single_change()` (~180行)
2. **Parallel**: `src/parallel/executor.rs::execute_archive_in_workspace()` (~290行)

共通の処理：
- タスク完了の検証（100%チェック）
- archive コマンドの実行とストリーミング出力
- アーカイブ後のパス検証（change が archive/ に移動したか確認）
- VCS へのコミット

相違点：
- TUI Serial は hooks を実行するが、Parallel は実行しない
- Parallel は workspace_path を考慮するが、Serial はカレントディレクトリで動作

この重複により、バグ修正や機能追加が2箇所で必要となり、メンテナンスコストが増大している。

## What Changes

- 新規: `src/execution/archive.rs` - 共通アーカイブロジック
  - `verify_archive_completion()` - パス検証の共通関数
  - `execute_archive()` - コマンド実行の共通関数
- 修正: `src/tui/orchestrator.rs` - 共通関数を使用するよう変更
- 修正: `src/parallel/executor.rs` - 共通関数を使用するよう変更

## Impact

- Affected specs: code-maintenance
- Affected code:
  - 新規: `src/execution/archive.rs`
  - 修正: `src/execution/mod.rs`
  - 修正: `src/tui/orchestrator.rs`
  - 修正: `src/parallel/executor.rs`
- **BREAKING**: なし（内部リファクタリング）

## 依存関係

- 前提: `create-execution-module` の完了
- 後続: `add-parallel-hooks` で hooks サポートを追加
