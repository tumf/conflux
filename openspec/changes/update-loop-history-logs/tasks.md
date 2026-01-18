## 1. 実装
- [x] 1.1 archive のデフォルト system-context を削除し、既定値を空にする
- [x] 1.2 逐次/並列の apply で履歴コンテキストを必ず注入する
- [x] 1.3 逐次/並列の archive で履歴コンテキストを必ず注入する
- [x] 1.4 resolve/analysis を含む全ループログに試行番号ヘッダーを付与する

## 2. 検証
- [x] 2.1 openspec の変更検証を実行する: `npx @fission-ai/openspec@latest validate update-loop-history-logs --strict`
