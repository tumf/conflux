# Change: Parallel mode への hooks サポート追加

## Why

現在、hooks（`pre_apply`, `post_apply`, `pre_archive`, `post_archive` など）は serial mode でのみサポートされており、parallel mode では実行されない。これは以下の問題を引き起こす：

1. **機能の不一致**: ユーザーが設定した hooks が parallel mode では無視される
2. **ワークフローの断絶**: CI/CD パイプラインや通知システムとの統合が parallel mode で機能しない
3. **一貫性の欠如**: serial と parallel で同じ設定ファイルを使用しているのに動作が異なる

## What Changes

- 修正: `src/parallel/executor.rs` - `HookRunner` を受け取り、適切なタイミングで hooks を実行
- 修正: `src/parallel/mod.rs` - `ParallelExecutor` に `HookRunner` を統合
- 修正: `src/parallel_run_service.rs` - hooks 設定を executor に渡す

### サポートする hooks

| Hook | タイミング | 備考 |
|------|----------|------|
| `pre_apply` | apply コマンド実行前 | 各 change の各反復で実行 |
| `post_apply` | apply コマンド成功後 | 各 change の各反復で実行 |
| `pre_archive` | archive コマンド実行前 | 各 change で1回 |
| `post_archive` | archive コマンド成功後 | 各 change で1回 |
| `on_change_start` | change 処理開始時 | 各 change で1回 |
| `on_change_complete` | change 100% 完了時 | 各 change で1回 |
| `on_error` | エラー発生時 | エラー毎に実行 |

### サポートしない hooks（parallel mode では意味がない）

| Hook | 理由 |
|------|------|
| `on_start` | グループ単位で開始するため、変更選択ロジックが異なる |
| `on_finish` | グループ単位で終了するため |

## Impact

- Affected specs: hooks, parallel-execution
- Affected code:
  - 修正: `src/parallel/executor.rs`
  - 修正: `src/parallel/mod.rs`
  - 修正: `src/parallel_run_service.rs`
- **BREAKING**: なし（機能追加）

## 依存関係

- 前提: `refactor-archive-common` と `refactor-apply-common` の完了
- 理由: 共通ロジック内で hooks を呼び出すことで、重複を避ける
