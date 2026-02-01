## 1. Implementation
- [ ] 1.1 `src/tui/utils.rs` に接尾辞指定可能なUnicode安全の省略関数を追加し、既存の省略関数をそれ経由にする（検証: `rg -n "truncate_to_display_width_with_suffix" src/tui/utils.rs`）
- [ ] 1.2 変更一覧のログプレビュー省略処理を新関数に置き換える（検証: `rg -n "truncate_to_display_width_with_suffix" src/tui/render.rs`）

## 2. Tests
- [ ] 2.1 日本語を含むログプレビューの省略がpanicしないことを検証するレンダリングテストを追加する（検証: `rg -n "log preview" src/tui/render.rs` または追加したテスト名の確認）
- [ ] 2.2 `cargo test tui::render` を実行しテストが成功する（検証: コマンドが成功する）
