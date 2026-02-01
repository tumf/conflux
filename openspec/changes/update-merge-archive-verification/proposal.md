# Change: parallel merge前のarchive完了検証を強化

## Why
並列実行でworktreeのarchiveが未完了のままmergeが走ると、baseに`openspec/changes/{id}`が残り、変更の整合性が崩れるため。現状の`verify_archive_completion`はファイル存在チェックのみで、Git状態（コミット済みか）を検証していない。

## What Changes
- `attempt_merge`のmerge前検証を`verify_archive_completion`から`is_archive_commit_complete`に置き換える
- `is_archive_commit_complete`は以下を検証する:
  1. worktreeがclean（未コミットの変更がない）
  2. `openspec/changes/<change_id>`が存在しない
  3. archiveエントリが存在する
- archive未完了の場合は`MergeDeferred`を返し、原因が分かるエラー理由を含める

## Impact
- Affected specs: parallel-execution (Individual Merge on Archive Completion)
- Affected code: src/parallel/mod.rs, src/parallel/tests/executor.rs
