## 1. Implementation
- [ ] 1.1 `AppState` にログパネル表示フラグを追加し、初期値は表示（true）にする。検証: `src/tui/state.rs` の `AppState` 定義と `AppState::new` に `logs_panel_enabled` が追加されている。
- [ ] 1.2 `l` キーでログパネル表示をトグルする。検証: `src/tui/key_handlers.rs` に `KeyCode::Char('l')` が追加され、`AppState` のトグル処理が呼ばれている。
- [ ] 1.3 ログパネル有無に応じてレイアウトを切り替え、非表示時もステータスパネルは維持する。検証: `src/tui/render.rs` のChangesビュー描画が表示フラグを参照し、Running/Stopping/Stopped でもステータス行が表示される。
- [ ] 1.4 キーヒントに `l: logs` を追加する。検証: `src/tui/render.rs` のChangesパネルタイトルに `l: logs` が含まれる。
- [ ] 1.5 描画テストを追加/更新する。検証: `src/tui/render.rs` のテストで「ログが存在しても非表示にできる」「キーヒントに l が出る」を確認できる。
- [ ] 1.6 テストを実行する。検証: `cargo test tui::render` が成功する。
