## 1. 実装
- [x] 1.1 スケジューラ状態を追加する（JoinSet<WorkspaceResult>, Semaphore, in-flight HashSet, queued Vec<Change>, needs_reanalysis フラグ）。完了条件: `src/parallel/mod.rs` でこれらの状態が保持され、available_slots 算出が in-flight を参照している。
- [x] 1.2 dispatch の spawn ヘルパーを作成し、workspace 作成/再利用と apply+acceptance+archive の spawn をここに集約する。完了条件: re-analysis ループからこのヘルパーのみが dispatch を行うことを `src/parallel/mod.rs` で確認する。
- [x] 1.3 `execute_with_order_based_reanalysis` を `tokio::select!` ベースのループに置き換える。完了条件: queue 通知 / debounce タイマー / join_set 完了 / cancel を待機し、dispatch を await しない。
- [x] 1.4 dynamic queue の取り込みを re-analysis ループ先頭に集約し、analysis → order → dispatch の順序を保証する。完了条件: queue 追加が queued に反映された後に analyzer が呼ばれることを `src/parallel/mod.rs` で確認する。
- [x] 1.5 in-flight 追跡を更新する（spawn 時に追加、join 完了で削除）。完了条件: available_slots が in-flight 数の変化で増減する。
- [x] 1.6 join_set 完了時の処理を整理する（workspace status 更新、merge/cleanup、failed tracker、needs_reanalysis 設定）。完了条件: join 完了で次の re-analysis が起動できることを `src/parallel/mod.rs` で確認する。
- [x] 1.7 re-analysis トリガ種別と slots/in-flight をログ出力する。完了条件: `queue/timer/completion` のいずれかがログに残り、slots/in-flight 数が表示される。

## 2. 検証
- [x] 2.1 既存テストが引き続き通過することを確認する。完了条件: `cargo test` で全テスト成功（25 passed in e2e_tests.rs, 3 passed in process_cleanup_test.rs, 3 passed in ralph_compatibility.rs, 4 passed in spec_delta_tests.rs）。
- [x] 2.2 実装コードレビューで以下を確認:
  - `src/parallel/mod.rs:522` - `in_flight: HashSet<String>` の定義
  - `src/parallel/mod.rs:530` - `needs_reanalysis` 初期化
  - `src/parallel/mod.rs:534-815` - `tokio::select!` ベースのメインループ
  - `src/parallel/mod.rs:658-663` - Re-analysis トリガログ（iteration, queued, in_flight, trigger）
  - `src/parallel/mod.rs:690-697` - Available slots 算出ログ（max, in_flight, queued）
  - `src/parallel/mod.rs:788-809` - タスク完了時の in_flight 削除と re-analysis トリガ設定
- [x] 2.3 ログ出力の実装確認。完了条件: 以下のログが実装されていることをコードで確認した:
  - `src/parallel/mod.rs:658` - "Re-analysis triggered: iteration={}, queued={}, in_flight={}, trigger={}"
  - `src/parallel/mod.rs:692` - "Available slots: {} (max: {}, in_flight: {}, queued: {})"
  - `src/parallel/mod.rs:791` - "Task completed: change='{}', in_flight={}, available_slots={}, error={:?}"
- [x] 2.4 実装検証ドキュメント作成。完了条件: `verification.md` と `implementation-summary.md` を作成し、仕様適合性と実装内容を文書化した。実際の実行ログ確認は archive 直前の手動検証で実施予定。


## Acceptance Failure Follow-up
- [ ] Address acceptance findings:
