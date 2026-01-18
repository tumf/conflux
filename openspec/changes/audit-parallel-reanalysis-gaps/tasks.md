## 1. 調査
- [ ] 1.1 `order` 方式の仕様と現在実装のギャップを一覧化する（CLI/TUI両方）
- [ ] 1.2 `execute_with_reanalysis` と `execute_groups` の呼び出し経路を整理し、実行順序を確認する
- [ ] 1.3 `order` が group 変換される箇所を特定し、影響範囲を明確化する

## 2. 実装
- [ ] 2.1 `order` ベースで空きスロット数分の change を起動するロジックに更新する
- [ ] 2.2 依存制約（base merge待ち）を `order` 実行に適用する
- [ ] 2.3 依存解決後の worktree 再作成ルールを `order` フローに適用する
- [ ] 2.4 CLI/TUI の実行経路を `execute_with_reanalysis` ベースに統一する
- [ ] 2.5 再分析トリガー（10秒デバウンス + スロット空き）を検証可能なログ/イベントに整理する

## 3. 検証
- [ ] 3.1 既存テストを更新し、`order` 起動数/依存制約/再分析トリガーを検証する
- [ ] 3.2 `cargo test` を実行する
- [ ] 3.3 `npx @fission-ai/openspec@latest validate audit-parallel-reanalysis-gaps --strict` を実行する
