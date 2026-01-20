## 1. フォローアップタスクの整形変更
- [x] 1.1 acceptance failure の tail 出力を番号ではなく箇条書きで記録する（完了条件: `src/orchestration/acceptance.rs` で複数行時のフォーマットが "- <line>" 形式になっている）
- [x] 1.2 tail 出力の内容は加工せずにそのまま記録する（完了条件: `ACCEPTANCE: FAIL` などの行がそのまま tasks.md に並ぶ）

## 2. 検証
- [x] 2.1 `npx @fission-ai/openspec@latest validate update-acceptance-followup-formatting --strict` を実行し、エラーがないことを確認する
