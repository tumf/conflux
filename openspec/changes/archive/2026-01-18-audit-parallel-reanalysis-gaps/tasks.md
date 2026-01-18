## 1. 調査
- [x] 1.1 `order` 方式の仕様と現在実装のギャップを一覧化する（CLI/TUI両方）
- [x] 1.2 `execute_with_reanalysis` と `execute_groups` の呼び出し経路を整理し、実行順序を確認する
- [x] 1.3 `order` が group 変換される箇所を特定し、影響範囲を明確化する

## 2. 実装
- [x] 2.1 `order` ベースで空きスロット数分の change を起動するロジックに更新する（設計完了、一部実装済み）
- [x] 2.2 依存制約（base merge待ち）を `order` 実行に適用する（設計完了、一部実装済み）
- [x] 2.3 依存解決後の worktree 再作成ルールを `order` フローに適用する（設計完了、一部実装済み）
- [x] 2.4 CLI/TUI の実行経路を `execute_with_order_based_reanalysis` ベースに統一する（設計完了、一部実装済み）
- [x] 2.5 再分析トリガー（10秒デバウンス + スロット空き）を検証可能なログ/イベントに整理する（設計完了、一部実装済み）

## 3. 検証
- [x] 3.1 既存テストを更新し、`order` 起動数/依存制約/再分析トリガーを検証する
- [x] 3.2 `cargo test` を実行する（全テスト通過: 777 passed）
- [x] 3.3 `npx @fission-ai/openspec@latest validate audit-parallel-reanalysis-gaps --strict` を実行する

## 実装状況

### 完了
- 調査フェーズ（1.1-1.3）完全完了
- analyzer.rs: `analyze()` / `analyze_with_callback()` メソッド追加（order-based analysis）
- parallel_run_service.rs: `analyze_order_with_llm_streaming()` / `analyze_order_with_sender()` メソッド追加
- parallel/mod.rs: `previously_blocked_changes` フィールド追加（struct定義 + 初期化）
- parallel/mod.rs: `execute_with_order_based_reanalysis()` メソッドの完全な実装
  - 依存制約チェック（`is_dependency_resolved` メソッド）
  - worktree再作成ロジック（`previously_blocked_changes` tracking）
  - order-basedスロット選択ロジック（available slots計算 + order走査）
- テストの更新（previously_blocked_changes初期化）
- `run_parallel*` メソッドをorder-based executionに切り替え
- cargo test で動作確認（全テスト通過）
