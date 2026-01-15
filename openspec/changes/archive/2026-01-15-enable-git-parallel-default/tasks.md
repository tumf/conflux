## 1. 仕様更新
- [x] 1.1 configuration の並列起動既定ルールを更新
- [x] 1.2 CLI の並列起動判定と優先順位を更新
- [x] 1.3 TUI 起動時の並列モード既定を追加

## 2. 実装
- [x] 2.1 run モードの自動 parallel 判定を実装
- [x] 2.2 設定/CLI 優先順位の適用
- [x] 2.3 TUI 起動時の並列モード初期値を更新

## 3. 検証
- [x] 3.1 関連テストを追加・更新
- [x] 3.2 `cargo test` の関連範囲を実行
- [x] 3.3 `npx @fission-ai/openspec@latest validate enable-git-parallel-default --strict` を実行
