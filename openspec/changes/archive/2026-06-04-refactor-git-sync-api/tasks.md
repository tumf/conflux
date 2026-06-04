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

## Acceptance #1 Failure Follow-up
- [x] Archive commitability is blocked by the real commit-path pre-commit hook. Evidence: `prek run --all-files` exited 1 in job `3015ee00c1427d831eac0294ceef19db`; hook `end-of-file-fixer` modified `.claude/settings.json` (`/Users/tumf/.local/share/agent-exec/jobs/3015ee00c1427d831eac0294ceef19db/stdout.log:2-7`). The workspace is now dirty (`git status --short` shows `M .claude/settings.json`), and cflx-accept rules require dirty working tree to fail acceptance. Action: kept and included the hook-produced `.claude/settings.json` EOF fix; reran the real commit-path hook checks with `agent-exec run -- prek run --all-files`, job `26e6e1a5c27b128a1100af284f5559a1`, exit 0.
- [x] OpenSpec/task/spec behavior evidence checked and was otherwise sufficient.
  verification: archive gate - `cflx openspec validate refactor-git-sync-api --archive-gate`; strict validation - `cflx openspec validate refactor-git-sync-api --strict`; integration - `cargo test server::api::git_sync`.
  completion: `openspec/changes/refactor-git-sync-api/tasks.md:3-14` has all active implementation tasks checked; spec delta requires preserved git sync representative branches at `openspec/changes/refactor-git-sync-api/specs/code-maintenance/spec.md:3-12`; strict validation passed; `cargo test server::api::git_sync` exited 0 in job `df445a0171637b4a6fa865993c3e34e2`; rerun of real commit-path hook checks exited 0 in job `26e6e1a5c27b128a1100af284f5559a1`.
