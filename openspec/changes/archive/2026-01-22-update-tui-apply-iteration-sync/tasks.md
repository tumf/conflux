## 1. 実装
- [x] 1.1 ApplyOutput のイベントに iteration を引き渡す (src/execution/apply.rs) / 確認: trait 実装と呼び出しが同一シグネチャでコンパイルできる
- [x] 1.2 ParallelApplyEventHandler が ApplyOutput に iteration を付与する (src/parallel/output_bridge.rs) / 確認: ApplyOutput の iteration が Some で送出される
- [x] 1.3 TUI が ApplyOutput 受信時に iteration_number を最新値へ更新する (src/tui/state/events/output.rs) / 確認: change.iteration_number が更新されることをログ/デバッガで確認

## 2. 検証
- [x] 2.1 `cargo test` を実行し、既存テストが通ることを確認する

## Future Work
- TUI 実行中に apply が複数回走る変更を処理し、Changes 一覧が `applying:<最新イテレーション>` を表示することを確認する (手動確認)
