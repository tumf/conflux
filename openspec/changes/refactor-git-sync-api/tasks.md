## Implementation Tasks

- [x] 1. resolve command と sync planning の現在挙動を characterization test で固定する。
  verification: unit - `cargo test server::api::git_sync::tests::test_run_resolve_command_quoted_template_does_not_double_quote`、`cargo test server::api::git_sync::tests::test_plan_sync_skips_when_remote_sha_matches_local_sha`。
  completion: prompt 展開、skip 判定、エラー判定がテストで固定されている。
- [x] 2. pull/push/sync route の現在挙動を integration test で固定する。
  verification: integration - `cargo test server::api::git_sync::tests::test_git_sync_success_response_contains_pull_and_push_sections`、`cargo test server::api::git_sync::tests::test_git_pull_non_fast_forward_detection`。
  completion: route status と response body の主要分岐が確認できる。
- [x] 3. `src/server/api/git_sync.rs` の resolve command、planning、route orchestration、test fixture を責務別に分割する。
  verification: integration - `cargo test server::api::git_sync`。
  completion: 公開 route contract を変えず、内部責務が小さな module/function に分離されている。
- [x] 4. git sync API error handling の後退がないことを確認する。
  verification: integration - `cargo test server::api::git_sync`。
  completion: resolve command 未設定、non-fast-forward、already up-to-date の代表分岐が成功する。

## Future Work

- VCS abstraction layer 全体への移行や Git/JJ 共通化は別 change で扱う。

## Final Validation

Expected archive gate: `cflx openspec validate refactor-git-sync-api --archive-gate`
