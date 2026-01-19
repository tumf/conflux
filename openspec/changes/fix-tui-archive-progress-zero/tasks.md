## 1. Implementation
- [ ] 1.1 `src/tui/runner.rs` L356-358 の `Ok(progress) =>` ブランチを修正し、`progress.total == 0` の場合は `parse_archived_change_with_worktree_fallback` を試し、それでも 0/0 なら既存値を保持するようにする
- [ ] 1.2 `cargo test` で既存テストがパスすることを確認する
- [ ] 1.3 TUIでアーカイブ処理を実行し、ファイル移動直後の自動更新で進捗が0にならないことを手動確認する
