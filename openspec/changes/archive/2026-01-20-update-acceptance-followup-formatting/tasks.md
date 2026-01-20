## 1. フォローアップタスクの整形変更
- [x] 1.1 acceptance failure の tail 出力を番号ではなく箇条書きで記録する（完了条件: `src/orchestration/acceptance.rs` で複数行時のフォーマットが "- <line>" 形式になっている）
- [x] 1.2 tail 出力の内容は加工せずにそのまま記録する（完了条件: `ACCEPTANCE: FAIL` などの行がそのまま tasks.md に並ぶ）

## 2. 検証
- [x] 2.1 `npx @fission-ai/openspec@latest validate update-acceptance-followup-formatting --strict` を実行し、エラーがないことを確認する


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ACCEPTANCE: FAIL
  2) FINDINGS:
  3) - `src/orchestration/acceptance.rs` の `update_tasks_on_acceptance_failure` が `findings.len() == 1` の場合に箇条書きの `- ` を付けず、仕様の「行ごとの箇条書き」に一致しません（統合は `src/orchestrator.rs` の `acceptance_test_streaming` 失敗分岐から呼ばれています）。
