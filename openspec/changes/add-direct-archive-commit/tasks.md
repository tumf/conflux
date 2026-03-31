## Implementation Tasks

- [x] 1. `src/execution/archive.rs`: `ensure_archive_commit()` 内で AI resolve コマンド呼び出しの前に、直接 `git add -A && git commit -m "Archive: {change_id}"` を実行するロジックを追加する（verification: `cargo test` で既存テストがパス）
- [x] 2. `src/execution/archive.rs`: 直接 commit が成功した場合、`is_archive_commit_complete()` を呼び出して完了確認し、成功なら早期 return する（verification: `cargo test` でパス）
- [x] 3. `src/execution/archive.rs`: 直接 commit が失敗した場合（exit code != 0）、warn ログを出力して既存の AI resolve フローにフォールバックする（verification: `RUST_LOG=debug cargo test` でフォールバックログを確認）
- [x] 4. `src/execution/archive.rs`: 直接 commit のユニットテストを追加する — (a) dirty worktree で直接 commit 成功、(b) pre-commit hook 失敗で AI resolve フォールバック（verification: `cargo test test_direct_archive_commit`）
- [x] 5. `cargo fmt --check && cargo clippy -- -D warnings` でリント・フォーマットチェックをパスする

## Future Work

- pre-commit hook が複数回ファイルを変更するケースの最適化（現状は AI resolve で対応）
