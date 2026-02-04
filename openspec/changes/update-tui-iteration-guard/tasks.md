## 1. Implementation
- [x] 1.1 `src/tui/state.rs` に iteration 更新の共通ヘルパー（単調増加 + None無視）を追加し、Apply/Archive/Acceptance/Resolve の output ハンドラから利用する（出力イベント到着順に依らず後退しないことを確認する）
- [x] 1.2 `src/tui/state.rs` の `handle_*_started` で `queue_status` 切替時に `iteration_number=None` を設定し、前ステージの値を持ち越さない（開始ログ後の表示がクリアされることを確認する）
- [x] 1.3 `src/tui/state.rs` のユニットテストを追加し、ステージ不一致/古い iteration で値が後退しないことを検証する（`cargo test` で該当テストが通ること）

## 2. Verification
- [x] 2.1 `cargo test` を実行し、TUI の状態更新テストがすべて成功することを確認する

## Acceptance #1 Failure Follow-up
- [x] `src/tui/state.rs` の出力ハンドラで `queue_status` と一致するステージのみ iteration を更新するガードを追加し、別ステージの出力で上書きされないようにする
- [x] `src/tui/state.rs` の `test_iteration_cross_stage_isolation` を修正し、ステージ外の出力が `iteration_number` を更新しないことを検証する
