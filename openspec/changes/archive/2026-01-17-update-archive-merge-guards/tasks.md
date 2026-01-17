# Tasks

- [x] 1. archive コミット完了判定に `openspec/changes/<change_id>` 存在チェックを追加する。
- [x] 2. `ensure_archive_commit` で `openspec/changes/<change_id>` が残っている場合はエラーにする。
- [x] 3. merge 実行前に `verify_archive_completion` を再検証し、未アーカイブなら `MergeDeferred` を返す。
- [x] 4. 並列実行のユニットテストを追加して、archive 戻りが merge されないことを確認する。
- [x] 5. `npx @fission-ai/openspec@latest validate update-archive-merge-guards --strict` を実行する。
