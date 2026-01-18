## 1. Implementation
- [x] 1.1 全ステートでの進捗更新ロジックを整理し、取得失敗を 0 件完了として扱わない方針に統一する
- [x] 1.2 TUI 側で全ステートの tasks.md 進捗を反映する
- [x] 1.3 WebState 側で全ステートの進捗保持ルールを適用する
- [x] 1.4 進捗更新の回帰テストを追加する（TUI + WebState）

## 2. Validation
- [x] 2.1 `cargo test`
- [x] 2.2 `npx @fission-ai/openspec@latest validate update-progress-archive-resolve --strict`
- [x] 2.3 提案内容のレビュー
