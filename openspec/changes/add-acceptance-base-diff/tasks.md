## 1. Implementation
- [x] 1.1 acceptance 差分コンテキスト生成に base branch 引数を追加し、初回は base branch → 現在コミットの差分ファイルを取得する。
      確認: `src/agent/runner.rs` の `build_acceptance_diff_context` が base branch を扱い、初回に `get_changed_files` を利用する。
- [x] 1.2 parallel acceptance で diff context を組み立ててプロンプトに挿入し、base branch を `execute_acceptance_in_workspace` に渡す。
      確認: `src/parallel/executor.rs` の acceptance プロンプト構築で `<acceptance_diff_context>` が追加される。
- [x] 1.3 acceptance プロンプト構築ヘルパーを更新し、diff context を system prompt 直後に挿入する。
      確認: `src/agent/prompt.rs` の `build_acceptance_prompt` と呼び出し箇所が一致する。
- [x] 1.4 ACCEPTANCE_SYSTEM_PROMPT に diff context の解釈・優先レビュー指示を追加する。
      確認: `src/config/defaults.rs` に diff-based review セクションが含まれる。

## 2. Validation
- [x] 2.1 acceptance プロンプトの diff context 挿入順序を検証するユニットテストを追加する。
      確認: `cargo test agent::prompt` が成功する。

## Acceptance #1 Failure Follow-up
- [x] src/orchestration/acceptance.rs の acceptance_test_streaming が base branch を渡さず agent.run_acceptance_streaming を呼び出しており、初回 acceptance で base 差分コンテキストが入らないため、base branch を渡すか差分生成を更新する。
  確認: `src/orchestration/acceptance.rs` が `get_current_branch` で base branch を取得し `run_acceptance_streaming` に渡す。
- [ ] Git working tree をクリーンにする (すべての変更をコミット)。
