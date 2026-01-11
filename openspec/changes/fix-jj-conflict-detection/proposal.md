# Change: jj マージコンフリクト検出の修正

## Why

jj バックエンドでマージコンフリクトが発生しても `resolve_command` が実行されない。jj は Git と異なり、コンフリクトがあっても `jj new` コマンドが成功ステータス（exit code 0）で終了するため、現在のコードではコンフリクトが検出されず、resolve_command がトリガーされない。

## What Changes

- `merge_jj_workspaces` 関数で、コマンド成功時でも stderr の "conflict" 文字列をチェックする
- または、マージコミット作成後に `detect_conflicts()` を呼び出してコンフリクト状態を確認する
- コンフリクトがある場合は `VcsError::Conflict` を返し、`resolve_conflicts_with_retry` が呼ばれるようにする

## Impact

- Affected specs: `parallel-execution`
- Affected code: `src/vcs/jj/mod.rs` (`merge_jj_workspaces` 関数)
