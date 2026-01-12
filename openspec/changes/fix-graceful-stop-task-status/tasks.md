# Tasks: Graceful Stop後のタスク状態修正

## 実装タスク

- [x] `src/tui/state/events.rs`の`OrchestratorEvent::Stopped`ハンドラを修正
  - Processing/Archiving状態の変更をQueuedに戻すロジックを追加
  - 中断された変更の経過時間を記録
  - Force stopの実装を参考にする

- [x] テストケースを追加：`test_stopped_event_cleans_up_processing_changes`
  - Processing状態の変更がQueuedに戻ることを確認
  - Archiving状態の変更もQueuedに戻ることを確認
  - 複数の変更が同時にProcessing/Archivingの場合も正しく処理されることを確認

- [x] テストケースを追加：`test_stopped_event_records_elapsed_time`
  - started_atが設定されている変更のelapsed_timeが記録されることを確認

- [x] 既存テストが影響を受けないか確認
  - `cargo test`を実行
  - TUI関連のテストがすべてパスすることを確認

## 検証タスク

- [x] 手動テストシナリオ1：基本的なGraceful stop
  - TUIを起動
  - 変更を選択して処理を開始
  - Escキーを押してGraceful stopを実行
  - 変更のステータスがQueuedに戻ることを確認
  - スピナーが停止することを確認

- [x] 手動テストシナリオ2：Archiving中のGraceful stop
  - 変更の処理が完了し、Archiving状態になるまで待つ
  - Escキーを押してGraceful stopを実行
  - Archiving状態の変更もQueuedに戻ることを確認

- [x] 手動テストシナリオ3：Force stopとの比較
  - Graceful stop後の状態がForce stopと同じになることを確認
  - F5キーで再開できることを確認

- [x] 手動テストシナリオ4：複数の変更
  - 複数の変更をキューに追加
  - 1つ目の処理中にGraceful stopを実行
  - Processing状態の変更のみがQueuedに戻ることを確認
  - Queued状態の変更はそのまま残ることを確認

## ドキュメント更新

- [x] コードコメントを更新（必要に応じて）
  - Graceful stopとForce stopの動作が同じであることを明記
