## 1. Implementation
- [ ] 1.1 `src/tui/state/events.rs` の `ProcessingCompleted` と `ArchiveStarted` の進捗再取得処理を見直し、worktree上のアーカイブ済みtasks.mdを優先的に参照する（進捗が0/0のときは既存値を維持する）ことを確認する（該当ハンドラの分岐とタスク進捗更新を確認）。
- [ ] 1.2 `src/tui/state/events.rs` の進捗更新ロジックに、アーカイブ移動直後（activeが消えた状態）でもworktreeのアーカイブ先から進捗を取得するフォールバックを追加する（`parse_archived_change_with_worktree_fallback` の呼び出しを確認）。
- [ ] 1.3 `src/tui/state/events.rs` のテスト（アーカイブ途中のworktree移動を想定したケース）を追加/更新し、0/0にならず進捗が保持されることを確認する（該当テストの追加/更新を確認）。
- [ ] 1.4 必要に応じて `cargo test` または該当テストを実行し、テストがパスすることを確認する（コマンドと結果を記録）。
