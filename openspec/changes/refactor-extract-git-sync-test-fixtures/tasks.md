## Implementation Tasks

- [x] 1. `git/sync` の主要シナリオ（同期済み、SHA 差異あり、remote ahead など）の公開レスポンスを固定する characterization test を先に追加または更新する (verification: integration - `src/server/api/git_sync.rs` の `test_git_sync_skips_resolve_when_already_up_to_date` / `test_git_sync_runs_resolve_when_shas_differ` / `test_git_sync_runs_resolve_when_remote_ahead` で `status`, `resolve_command_ran`, `resolve_exit_code`, `push.status`, `skipped_reason` を assertion)
- [x] 2. registry / AppState / router 初期化を共通 fixture helper に抽出し、各テストが期待挙動の assertion に集中できる構造へ整理する (verification: integration - `src/server/api/git_sync.rs` に `setup_git_sync_fixture` と `invoke_git_sync` を追加し、git_sync endpoint tests が helper を利用)
- [x] 3. local bare repo / scratch clone / divergence 生成の重複手順を helper 化し、ケースごとの repo 変異を意図中心に記述できるようにする (verification: integration - `src/server/api/git_sync.rs` に `run_git` を追加し、divergence シナリオで git command 重複を削減)
- [x] 4. helper 抽出後も `git_sync` のログ意味論とレスポンス契約に回帰がないことを確認する (verification: integration - `cargo test git_sync_` 実行で success / skip / resolve-run シナリオの回帰なしを確認)
- [x] 5. proposal delta と関連テスト整理を strict validation と Rust テストで確認する (verification: integration - `cflx openspec validate refactor-extract-git-sync-test-fixtures --strict --evidence warn` と `cargo test git_sync_` 実行済み)

## Future Work

- `server/api` 他モジュールの大型統合テストにも共通 fixture パターンを横展開する
- API テストの JSON assertion helper を導入して可読性をさらに上げる

## Acceptance #1 Failure Follow-up
- [x] `src/server/api/git_sync.rs:1721-1784` の `test_git_sync_runs_resolve_when_shas_differ` は最終的に `json2["status"] == "synced"` しか確認しておらず、`openspec/changes/refactor-extract-git-sync-test-fixtures/tasks.md:3` が要求する `resolve_command_ran` / `resolve_exit_code` / `push.status` などの公開レスポンス契約を明示的に検証していない。差分ありシナリオで対象フィールドを assertion すること。
- [x] `src/server/api/git_sync.rs:1788-1808` の `test_git_sync_runs_resolve_when_remote_ahead` も `json["status"] == "synced"` しか確認しておらず、`openspec/changes/refactor-extract-git-sync-test-fixtures/specs/testing/spec.md:18-21` が求める主要回帰シナリオの JSON 契約固定として不十分。remote ahead シナリオで `resolve_command_ran` / `resolve_exit_code` / `push.status` などの公開レスポンス項目を明示的に assertion すること。

## Acceptance #2 Failure Follow-up
- [x] `src/server/api/git_sync.rs:1809-1818` の `test_git_sync_runs_resolve_when_shas_differ` は `resolve_command_ran` / `resolve_exit_code` / `push.status` / `skipped_reason` の存在や型しか見ておらず、`openspec/changes/refactor-extract-git-sync-test-fixtures/tasks.md:3` と `openspec/changes/refactor-extract-git-sync-test-fixtures/specs/testing/spec.md:18-21` が要求する characterization coverage として値を固定できていない。`src/server/api/git_sync.rs:575-582,615-623` の実装契約に合わせて `resolve_command_ran == true`, `resolve_exit_code == 0`, `push.status == "pushed"`, `skipped_reason == null` まで明示的に assertion すること。
- [ ] `src/server/api/git_sync.rs:1846-1855` の `test_git_sync_runs_resolve_when_remote_ahead` も `resolve_command_ran` / `resolve_exit_code` / `push.status` / `skipped_reason` を緩くしか検証しておらず、主要回帰シナリオの JSON 契約を固定できていない。`src/server/api/git_sync.rs:575-582,615-623` の success response に合わせて `resolve_command_ran == true`, `resolve_exit_code == 0`, `push.status == "pushed"`, `skipped_reason == null` を明示的に assertion すること。

## Implementation Blocker #1
- category: spec_contradiction
- summary: remote_ahead シナリオが現行 fixture では 200/synced 契約に到達せず 422 を返し、要求 assertion と両立しない
- evidence:
  - /Users/tumf/.local/share/agent-exec/jobs/eada44d79c4d93deb593e81a1e305e0d/stdout.log:28-31
  - src/server/api/git_sync.rs:1891 (remote_ahead test の status assertion)
- impact: Acceptance #2 Failure Follow-up の remote_ahead 契約固定タスクを完了できない
- unblock_actions:
  - remote_ahead fixture で non-fast-forward にならない再現条件（local/remote SHA 構成）を設計し直す
  - 期待契約を 200/synced に固定するか、422 系を別シナリオとして仕様更新するかを明確化する
- owner: server-api maintainers
- decision_due: 2026-04-30

## Rejecting Recovery Tasks

- [ ] Investigate blocker in openspec/changes/refactor-extract-git-sync-test-fixtures/REJECTED.md and implement a non-rejection recovery path before rerunning apply
