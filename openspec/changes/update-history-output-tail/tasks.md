## 1. Implementation
- [x] 1.1 apply/archive の履歴に出力末尾を保存するための構造拡張を追加する
- [x] 1.2 stdout/stderr の収集・要約ロジックを apply/archive 共通で実装する
- [x] 1.3 逐次/並列ループから履歴へ出力を引き渡す
- [x] 1.4 既存履歴フォーマットを拡張し、末尾出力をプロンプトへ注入する
- [x] 1.5 テストを追加/更新し、履歴が出力を含むことを検証する
- [x] 1.6 resolve の履歴に出力末尾を保存するための構造拡張を追加する

## 2. Validation
- [x] 2.1 `cargo test`
- [x] 2.2 `npx @fission-ai/openspec@latest validate update-history-output-tail --strict`
