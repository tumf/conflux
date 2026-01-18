## 1. Implementation
- [ ] 1.1 全ステートでの進捗更新ロジックを整理し、取得失敗を 0 件完了として扱わない方針に統一する
- [ ] 1.2 TUI 側で全ステートの tasks.md 進捗を反映する
- [ ] 1.3 WebState 側で全ステートの進捗保持ルールを適用する
- [ ] 1.4 進捗更新の回帰テストを追加する（TUI + WebState）

## 2. Validation
- [ ] 2.1 `cargo test`
- [ ] 2.2 `npx @fission-ai/openspec@latest validate update-progress-archive-resolve --strict`
- [ ] 2.3 提案内容のレビュー
