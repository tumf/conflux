## 1. Implementation
- [x] 1.1 resolve プロンプトの「最終マージ」手順を `git merge --no-ff --no-commit <branch>` に変更し、コミット前に `openspec/changes/{change_id}` の復活を削除する条件を明記する（検証: `src/parallel/conflict.rs` の resolve プロンプトを確認）
- [x] 1.2 復活判定の条件を「`openspec/changes/{change_id}/proposal.md` が存在し、かつ `openspec/changes/archive/` に同一 `change_id` のアーカイブが存在する」に限定する文言を追加する（検証: resolve プロンプト内に条件が明示されていることを確認）
- [x] 1.3 resolve プロンプトの要件が満たされていることを検証するテストを追加する（例: `src/parallel/tests/conflict.rs` に `--no-commit` と削除手順が含まれることをアサート、検証: `cargo test parallel::tests::conflict::test_resolve_merges_prompt_contains_cleanup_instructions`）
