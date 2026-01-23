## 1. Implementation
- [ ] 1.1 acceptance 差分コンテキスト生成に base branch 引数を追加し、初回は base branch → 現在コミットの差分ファイルを取得する。
      確認: `src/agent/runner.rs` の `build_acceptance_diff_context` が base branch を扱い、初回に `get_changed_files` を利用する。
- [ ] 1.2 parallel acceptance で diff context を組み立ててプロンプトに挿入し、base branch を `execute_acceptance_in_workspace` に渡す。
      確認: `src/parallel/executor.rs` の acceptance プロンプト構築で `<acceptance_diff_context>` が追加される。
- [ ] 1.3 acceptance プロンプト構築ヘルパーを更新し、diff context を system prompt 直後に挿入する。
      確認: `src/agent/prompt.rs` の `build_acceptance_prompt` と呼び出し箇所が一致する。
- [ ] 1.4 ACCEPTANCE_SYSTEM_PROMPT に diff context の解釈・優先レビュー指示を追加する。
      確認: `src/config/defaults.rs` に diff-based review セクションが含まれる。

## 2. Validation
- [ ] 2.1 acceptance プロンプトの diff context 挿入順序を検証するユニットテストを追加する。
      確認: `cargo test agent::prompt` が成功する。
