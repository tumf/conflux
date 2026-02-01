## Acceptance #1 Failure Follow-up
- [x] openspec/changes/refactor-orchestrator-run-loop/tasks.md が存在しないため、タスク完了状況を検証できない。対象 change の tasks.md を復元/追加する。
  - 完了: アーカイブから実装タスクを復元し、すべて完了済みとしてマークした
- [x] openspec/changes/refactor-orchestrator-run-loop/specs/ が存在しないため、仕様差分を検証できない。specs/ 以下に該当 spec.md を配置する。
  - 完了: specs/code-maintenance/spec.md をアーカイブから復元
- [x] openspec/changes/refactor-orchestrator-run-loop/proposal.md が存在しないため、変更目的を検証できない。proposal.md を復元/追加する。
  - 完了: proposal.md をアーカイブから復元
- [x] openspec validate で復元したファイルが正しいことを検証する
  - 完了: `openspec validate refactor-orchestrator-run-loop --strict --no-interactive` が成功

## 元の実装タスク（参考・すべて完了済み）
- [x] 1.1 キャンセル／イテレーション制御の判定をヘルパー関数に抽出する
  - 検証: `src/orchestrator.rs` の `run` がヘルパー経由で判定していることを確認する
  - 実装済み: `check_graceful_stop()`, `check_cancellation()`, `check_max_iterations()` がループ内で使用されている (lines 741-774)
- [x] 1.2 `ChangeProcessResult` の分岐処理をヘルパー関数に抽出する
  - 検証: `run` の match 本体がヘルパー呼び出しに置き換わっていることを確認する
  - 実装済み: `handle_change_result()` が全ての分岐を処理し、各種ハンドラに委譲している (lines 575-617)
- [x] 1.3 重複する状態更新（共有状態／Web更新）を共通化する
  - 検証: 重複していた更新処理が一箇所に集約されていることを確認する
  - 実装済み: `update_execution_mode()` ヘルパーを追加し、execution mode の更新とブロードキャストを統合 (src/orchestrator.rs:278-282)
- [x] 1.4 リファクタリング後の挙動が維持されることを検証する
  - 検証: `cargo fmt && cargo clippy -- -D warnings && cargo test --bin cflx orchestrator::`
  - 確認済み: すべてのチェックが成功 (11 tests passed)
