## 1. Implementation
- [x] 1.1 受理結果 FAIL/CommandFailed 時の tasks.md 追記内容を stdout/stderr の末尾 N 行に置換し、既存の FINDINGS 抽出が使われないことを確認する（対象: src/orchestration/acceptance.rs, src/parallel/executor.rs）
- [x] 1.2 末尾 N 行の優先順位（stdout 優先、空の場合は stderr）と空出力時のフォールバック文言を定義・実装し、適用箇所を揃える
- [x] 1.3 CLI/TUI/並列実行それぞれで acceptance 失敗時に tasks.md が更新されることを確認する（ログ/実行経路確認）

## 2. Validation
- [x] 2.1 受理出力に FINDINGS があっても抽出せず、末尾 N 行が tasks.md に残ることを確認する（受理出力のサンプルで確認）
- [x] 2.2 acceptance 出力が空の場合にフォールバック文言が tasks.md に書かれることを確認する
