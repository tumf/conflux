# Change: archive コミットの resolve_command 委譲と merge change_id の正規化

## Why
並列実行において archive 後のコミットが pre-commit で中断すると、再開時に archived 扱いで merge/cleanup が進み、マージ前にワークスペースが削除される。また merge 処理の change_id がコミット SHA に置き換わる経路があり、`Merge change: <change_id>` 検証に失敗する。

## What Changes
- archive フェーズの `git add/commit` を `resolve_command` に委譲し、pre-commit 中断時も再ステージ・再コミットで収束させる
- merge フェーズで使用する change_id を `openspec/changes/{change_id}` の ID に統一し、worktree ブランチ名と change_id の対応を明示する
- resume 時の archived 判定を「archive コミット完了済み」基準に強化し、未コミットの archive を再処理する

## Impact
- Affected specs: `parallel-execution`
- Affected code (expected): `src/parallel/mod.rs`, `src/parallel/conflict.rs`, `src/parallel/executor.rs`, `src/execution/archive.rs`, `tests/`
