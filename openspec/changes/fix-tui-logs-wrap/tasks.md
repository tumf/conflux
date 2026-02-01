## 1. Implementation
- [x] 1.1 Logsビュー向けの折り返しヘルパを追加し、prefix幅を維持したインデント表示にする。検証: `src/tui/render.rs` に折り返し関数が追加され、1行目は timestamp+header、2行目以降は同幅の空白インデントで描画されることを確認する。
- [x] 1.2 Logsビューの可視範囲計算を表示行数ベースに変更し、Paragraph の自動 wrap に依存しないようにする。検証: `render_logs` が折り返し後の表示行数で範囲を計算し、`Paragraph::wrap` を使わないことを確認する。
- [x] 1.3 回帰テストを追加する（折り返しインデント・表示範囲ずれ防止）。検証: `src/tui/render.rs` のテストで長文ログが左端に戻らないこと、最新ログが表示範囲に残ることを検証する。

## 2. Validation
- [x] 2.1 `cargo test tui::render::tests::test_logs_wrap_indents_continuation_lines tui::render::tests::test_logs_visible_range_not_broken_by_wrapped_entry` を実行し成功する。
