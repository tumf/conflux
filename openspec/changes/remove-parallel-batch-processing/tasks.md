## 1. Implementation
- [ ] 1.1 `src/parallel/mod.rs` のバッチ/グループ前提処理を特定し、削除対象の関数と呼び出し箇所を一覧化する（完了条件: tasks.md に対象関数と呼び出し箇所が列挙されている）
- [ ] 1.2 `execute_with_reanalysis` のバッチ完了待ちを撤廃し、空きスロット駆動の連続ディスパッチに置き換える（完了条件: バッチ境界の待ちがなく、空きスロットでキューが消化される）
- [ ] 1.3 `execute_group` と `ParallelGroup` の使用箇所を廃止し、依存失敗チェックをディスパッチ時の個別評価に移動する（完了条件: グループ単位のスキップ処理が削除される）
- [ ] 1.4 `src/tui/orchestrator.rs` のバッチループを連続ディスパッチに置き換える（完了条件: batch_ids の集合ループが削除される）
- [ ] 1.5 `src/parallel_run_service.rs` と `src/parallel/executor.rs` の batch/group 文言をスロット駆動の表現へ置換する（完了条件: batch/group のログ文言が除去される）

## 2. Validation
- [ ] 2.1 `npx @fission-ai/openspec@latest validate remove-parallel-batch-processing --strict` を実行し、エラーがないことを確認する
- [ ] 2.2 並列実行中に change を追加し、バッチ完了を待たずに起動されることを確認する（完了条件: TUI ログで追加から 10 秒以内に新規 change の起動が確認できる）
