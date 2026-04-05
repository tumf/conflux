## Implementation Tasks

- [x] 1. resumed workspace routing で tasks completion を評価する現行経路を特定し、implementation task incomplete を Apply 優先 gate とする設計を `src/parallel/dispatch.rs` / `src/parallel/executor.rs` に対応づける (verification: 対象 routing 箇所が proposal/design に反映されている)
- [x] 2. unchecked implementation task が残る resumed implementation workspace を Apply に戻すよう routing を更新する (verification: incomplete tasks の resume が Apply を選ぶ回帰テストが追加される)
- [x] 3. completed tasks の resumed workspace では既存の Acceptance > Archive routing が維持されることを確認する (verification: completed tasks の resume routing 回帰テストが追加される)
- [x] 4. tasks incomplete による Apply 再ルーティング理由をログまたはイベントで観測可能にする (verification: 対応ログ/イベントの検証が追加される)
- [x] 5. quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)

## Future Work

- spec-only changes や Future Work checkbox の resume policy を必要に応じて別 change で整理する

## Acceptance #1 Failure Follow-up

- [x] `cargo test` failing の原因を修正し、`server::api::projects::tests::test_add_project_creates_worktree_on_server_branch` が再度成功することを確認する
- [x] quality gate 実行結果に合わせて task 5 の完了状態を見直し、必要なら再実行後にのみ `[x]` に戻す
- [x] acceptance 実行前に `.cflx/acceptance-state.json` の未コミット変更が残らないようにワークツリーをクリーンな状態へ戻す

## Acceptance #2 Failure Follow-up

- [x] `.cflx/acceptance-state.json` の未コミット変更を解消し、`git status --porcelain` が空になることを確認する

## Acceptance #3 Failure Follow-up

- [x] resume routing の tasks 判定を `## Implementation Tasks` セクション限定の独自パーサーから、archive guard と同一の `task_parser::parse_content` 相当に変更し、スコープ不一致を解消する (verification: `## Acceptance #N Failure Follow-up` に未完了 checkbox がある場合に resume が Apply を選ぶ回帰テストが追加される)
- [x] `read_implementation_task_progress` を archive guard と同一スコープの判定に置き換え、独自セクション限定パーサーを廃止する (verification: `src/parallel/dispatch.rs` の回帰テストで routing と archive guard の判定結果が一致することを確認できる)
- [x] quality gate を実行する (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`)
