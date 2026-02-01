## 1. Implementation
- [ ] 1.1 Applying中のステータス文字列から進捗百分率を除外する（確認: `src/tui/render.rs` のApplying分岐が`[applying:iteration]`形式になっている）
- [ ] 1.2 Applying中のタスク進捗を`<completed>/<total>(<percent>%)`形式に変更する（確認: `src/tui/render.rs` の表示と幅計算が同一フォーマットになっている）
- [ ] 1.3 既存ログプレビューの幅計算が新フォーマットと一致することを確認する（確認: `src/tui/render.rs` でApplying時の`tasks_width`が新フォーマットに基づく）

## 2. Validation
- [ ] 2.1 `npx @fission-ai/openspec@latest validate update-tui-applying-progress-format --strict` が成功する
