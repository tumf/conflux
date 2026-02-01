## 1. Implementation
- [ ] 1.1 src/tui/render.rs: RunningモードChanges一覧のステータス組み立てを更新し、経過時間をスピナー直後へ移動する
  - 検証: src/tui/render.rs の Running行で「spinner → elapsed → status」順になることを確認する
- [ ] 1.2 src/tui/render.rs: ログプレビューの幅計算を新しい表示順に合わせて更新する
  - 検証: `rg -n "elapsed_width|status_width|spinner" src/tui/render.rs` で末尾の経過時間カラム計算が削除されていることを確認する
