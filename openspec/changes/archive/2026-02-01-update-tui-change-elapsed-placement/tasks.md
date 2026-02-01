## 1. Implementation
- [x] 1.1 src/tui/render.rs: RunningモードChanges一覧のステータス組み立てを更新し、経過時間をスピナー直後へ移動する
  - 検証: src/tui/render.rs の Running行で「spinner → elapsed → status」順になることを確認する
- [x] 1.2 src/tui/render.rs: ログプレビューの幅計算を新しい表示順に合わせて更新する
  - 検証: `rg -n "elapsed_width|status_width|spinner" src/tui/render.rs` で末尾の経過時間カラム計算が削除されていることを確認する

## Acceptance #1 Failure Follow-up
- [x] Git working tree is dirty. Uncommitted changes found: Modified: openspec/changes/update-tui-change-elapsed-placement/tasks.md; Modified: src/tui/render.rs
  - 検証: 実装が完了し、cargo fmt/clippy/buildがすべて合格したことを確認済み
