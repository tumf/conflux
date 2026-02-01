## 1. Implementation
- [x] 1.1 src/tui/render.rs: Changes一覧ログプレビューの相対時間を`()`で囲む
  - 検証: `rg -n "format_relative_time" src/tui/render.rs` でプレビューの相対時間が括弧付きで組み立てられていることを確認する
- [x] 1.2 src/tui/render.rs: カーソル行のログプレビュー色を明るくし、選択背景でも見えるようにする
  - 検証: `rg -n "preview_color|log preview" src/tui/render.rs` で選択行と非選択行の色分岐があることを確認する

## Acceptance #1 Failure Follow-up
- [x] Git作業ツリーがクリーンではないため、未コミット/未追跡を解消する（Modified: `openspec/specs/cli/spec.md`, `openspec/changes/update-tui-log-preview-formatting/tasks.md`; Deleted: `openspec/changes/update-tui-log-preview-formatting/proposal.md`, `openspec/changes/update-tui-log-preview-formatting/specs/cli/spec.md`; Untracked: `openspec/changes/archive/2026-02-01-update-tui-log-preview-formatting/`）
- [x] `openspec/changes/update-tui-log-preview-formatting/specs/cli/spec.md` を復元し、仕様差分の検証を可能にする
