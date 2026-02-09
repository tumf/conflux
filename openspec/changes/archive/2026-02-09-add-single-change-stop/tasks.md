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

## Acceptance #2 Failure Follow-up
- [x] Serial の active change 停止が Acceptance 実行中に全体停止へ誤変換されます。`SerialRunService::process_acceptance_result` は `AcceptanceResult::Cancelled` を `ChangeProcessResult::Cancelled` に変換し (`src/serial_run_service.rs:580`)、`run_orchestrator` はこの結果を受けると `pending_changes.clear(); break;` で全体停止します (`src/tui/orchestrator.rs:437`)。単体停止由来の Cancelled を識別して `ChangeStopped` を送信し、対象 change のみ `not queued` へ戻して他の queued change を継続実行するように修正してください。
  - Implementation: `ChangeProcessResult` に `ChangeStopped` variant を追加しました (`src/serial_run_service.rs:612`)。`process_acceptance_result` メソッドを修正し、`is_single_change_stopped` クロージャを受け取るようにしました (`src/serial_run_service.rs:530-539`)。`AcceptanceResult::Cancelled` を受け取った際に、`is_single_change_stopped()` を確認し、true なら `ChangeProcessResult::ChangeStopped` を返すようにしました (`src/serial_run_service.rs:588-597`)。TUI orchestrator で `is_single_change_stopped` クロージャを作成し、`DynamicQueue::try_is_stopped()` のみを確認するようにしました (`src/tui/orchestrator.rs:407-409`)。`ChangeStopped` の処理を追加し、`ChangeStopped` イベントを送信して対象 change のみを `pending_changes` から削除し、他の queued change は継続するようにしました (`src/tui/orchestrator.rs:454-468`)。CLI mode では `is_single_change_stopped` として常に false を返すクロージャを渡し、`ChangeStopped` は全体停止として扱うようにしました (`src/orchestrator.rs:703-711, 834`)。

## Acceptance #3 Failure Follow-up
- [x] Acceptance 中の単体停止で `ChangeProcessResult::ChangeStopped` を処理する分岐が停止フラグをクリアしていません。`run_orchestrator` の `Ok(ChangeProcessResult::ChangeStopped)` 分岐では `pending_changes` から除外して継続するのみで、`dynamic_queue.clear_stopped(&change_id)` が呼ばれていません (`src/tui/orchestrator.rs:453-468`)。このため停止済み change を再度 queued に戻した際、古い stop フラグが残って即時再停止される可能性があります。`ChangeStopped` 分岐でも停止完了時に stop フラグをクリアしてください（他の停止完了経路と同様に）。
  - Implementation: `ChangeStopped` 分岐の先頭に `dynamic_queue.clear_stopped(&change_id).await;` を追加しました (`src/tui/orchestrator.rs:454`)。これにより停止済み change を再度 queued に戻した際に古い stop フラグが残らず、即時再停止を防ぎます。
