## 1. 仕様・設計
- [ ] 1.1 acceptance の既存ループ仕様と iteration 継続要件を確認する（確認先: openspec/specs/cli/spec.md）
- [ ] 1.2 acceptance ログヘッダと iteration 継続の仕様差分を整理する（確認先: openspec/changes/add-acceptance-iteration-header/specs/cli/spec.md）

## 2. 実装
- [ ] 2.1 acceptance ログ出力に iteration を付与するイベント拡張（確認先: src/events.rs, src/tui/state/events.rs, src/tui/orchestrator.rs）
- [ ] 2.2 acceptance ループの iteration 継続ロジックを実装する（確認先: src/orchestrator.rs, src/tui/orchestrator.rs, src/parallel/mod.rs）
- [ ] 2.3 TUI ログ表示の acceptance ヘッダを確認する（確認先: src/tui/render.rs）
- [ ] 2.4 既存テストの更新（確認先: src/tui/state/events.rs のログテスト）

## 3. 検証
- [ ] 3.1 変更に影響するテストを実行する（コマンド: cargo test）
- [ ] 3.2 acceptance ログで iteration が継続表示されることを確認する（確認先: TUI ログパネル表示）
