## 1. Implementation
- [x] 1.1 TUIの単体停止コマンド追加（`TuiCommand::StopChange`）と入力分岐の実装。active changeでSpaceを押した場合に停止要求を送る。キーヒントはactive行で`Space: stop`を表示する。
  - Verify: `src/tui/events.rs`, `src/tui/key_handlers.rs`, `src/tui/render.rs` を確認し、active行のSpaceが停止要求へ分岐していること
- [x] 1.2 停止完了/失敗イベント（`ChangeStopped`/`ChangeStopFailed`）の追加と、停止完了時の`not queued`遷移・実行マーク解除の反映。
  - Verify: `src/tui/state.rs` のイベント処理に追加され、`selected=false`/`queue_status=NotQueued`になることを確認
- [x] 1.3 Serial実行の単体キャンセル経路を追加し、停止対象のみ中断して残りのqueuedを継続する。
  - Verify: `src/tui/orchestrator.rs` または `src/serial_run_service.rs` でchange単位のキャンセル判定が行われ、停止対象のみが除外されることを確認
- [x] 1.4 Parallel実行でchange単位のキャンセルを導入し、in-flightから除外後に再分析が走るようにする。
  - Verify: `src/parallel/mod.rs` にキャンセル受付と`needs_reanalysis`の更新が入り、queuedが残る場合に再分析が走ることを確認
- [x] 1.5 仕様差分の作成と検証。
  - Verify: `npx @fission-ai/openspec@latest validate add-single-change-stop --strict --no-interactive`

## Acceptance #1 Failure Follow-up
- [x] Serial の単体停止が active change を実行中に中断できていません。`run_orchestrator` では停止フラグを `SerialRunService::process_change` 呼び出し前 (`src/tui/orchestrator.rs:241`) にしか確認しておらず、`cancel_check` はグローバル `cancel_token` のみを見ています (`src/tui/orchestrator.rs:393`)。`process_change` 実行中にも対象 change の停止要求を検知して `ChangeStopped` を発火し、当該 change を `not queued` に戻す経路を実装してください。
  - Implementation: `cancel_check` クロージャを修正し、グローバル `cancel_token` に加えて `DynamicQueue::try_is_stopped()` を確認するようにしました (`src/tui/orchestrator.rs:397-406`)。`DynamicQueue` に非ブロッキングな `try_is_stopped` メソッドを追加し (`src/tui/queue.rs:139-148`)、`SerialRunService::process_change` 内の apply ループがこれを検出できるようにしました。エラー処理時に停止要求を検出し、`ChangeStopped` イベントを送信して `not queued` に戻すようにしました (`src/tui/orchestrator.rs:761-784`)。
- [x] Parallel の単体停止が真の in-flight キャンセルになっていません。停止フラグ確認は dispatch 前 (`src/parallel/mod.rs:2401`) と apply+acceptance ループ先頭 (`src/parallel/mod.rs:2484`) のみで、実行中コマンドを中断する経路がありません。実行中タスク/子プロセスを change 単位で中断できる仕組みを追加し、停止完了時に `ChangeStopped` を確実に通知して残り queued の再分析継続を保証してください。
  - Implementation: 各 change 専用の `per_change_cancel` トークンを作成し、バックグラウンドタスクでグローバル cancel と `DynamicQueue` の停止フラグを監視するようにしました (`src/parallel/mod.rs:2483-2506`)。この per-change トークンを `execute_apply_in_workspace`, `execute_acceptance_in_workspace`, `execute_archive_in_workspace` に渡すことで、実行中のコマンドを中断できるようにしました。Apply, acceptance, archive の各エラーハンドリングで停止要求を検出し、`ChangeStopped` イベントを送信するようにしました (`src/parallel/mod.rs:2601-2628, 2826-2846, 2848-2873, 2937-2961`)。
