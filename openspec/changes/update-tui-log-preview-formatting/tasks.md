## 1. Implementation
- [ ] 1.1 src/tui/render.rs: Changes一覧ログプレビューの相対時間を`()`で囲む
  - 検証: `rg -n "format_relative_time" src/tui/render.rs` でプレビューの相対時間が括弧付きで組み立てられていることを確認する
- [ ] 1.2 src/tui/render.rs: カーソル行のログプレビュー色を明るくし、選択背景でも見えるようにする
  - 検証: `rg -n "preview_color|log preview" src/tui/render.rs` で選択行と非選択行の色分岐があることを確認する
