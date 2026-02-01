## 1. 実装
- [x] 1.1 並列開始時の change フィルタ（コミットツリー基準）を共通ヘルパーに集約する（検証: 2経路以上で同一ヘルパーが使われていることを確認）
- [x] 1.2 除外した change の警告通知を共通化する（検証: 警告生成が単一ヘルパー経由になっていることを確認）
- [x] 1.3 既存のイベント送信順序を維持する（検証: 共有状態更新より先にイベントが送信されることをコードで確認）
- [x] 1.4 既存の挙動維持を確認するため `cargo test` を実行する（検証: `cargo test` が成功）

## Acceptance #1 Failure Follow-up
- [x] openspec/changes/refactor-parallel-run-service-prep/specs/parallel-execution/spec.md:32-35 の要件どおり、git 未存在時に ParallelRunService 自身がエラーを返すよう実装する（`ParallelRunService::check_vcs_available` が bool を返すだけになっているため見直す）
- [x] TUI 並列経路で `ParallelRunService::check_vcs_available` が呼ばれておらず git 未存在時のエラーが発生しないため、`src/tui/orchestrator.rs:1194` の並列開始前にサービス側の検証を追加する
- [x] `src/parallel_run_service.rs:578-652` の `group_by_dependencies` が本番フローで未使用のため、CLI/TUI/parallel の実行経路に接続するか削除して dead code を解消する

## Acceptance #2 Failure Follow-up
- [x] Git の作業ツリーが未クリーンのため、未コミット変更（`openspec/changes/refactor-parallel-run-service-prep/tasks.md`, `src/orchestrator.rs`, `src/parallel_run_service.rs`, `src/tui/orchestrator.rs`）を解消する
- [x] `src/parallel_run_service.rs:598-599` の `group_by_dependencies` が `#[allow(dead_code)]` で本番経路から未使用のため、CLI/TUI/parallel の実行経路へ接続するか削除して dead code を解消する（参照は `src/parallel_run_service.rs:723-789` のテストのみ）
