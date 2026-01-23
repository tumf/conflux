## 1. 実装
- [x] 1.1 apply プロンプト組み立て時に `AcceptanceHistory` の stdout/stderr tail を取得し、`<last_acceptance_output>` ブロックとして追加する（stdout_tail 優先、空なら stderr_tail）。確認: `src/agent/runner.rs` と `src/parallel/executor.rs` の apply プロンプト生成経路にブロック挿入がある。
- [x] 1.2 acceptance 失敗後の最初の apply 試行にのみ tail を注入する判定を追加する。確認: 同一 change の連続 apply で 2 回目以降はブロックが含まれない。
- [x] 1.3 `build_apply_prompt` のテストを追加し、acceptance tail が含まれることと stdout/stderr の優先順位を検証する。確認: `cargo test agent::tests::test_build_apply_prompt_with_acceptance_tail` が通る。

## 2. 検証
- [x] 2.1 `cargo test agent::tests::test_build_apply_prompt_with_acceptance_tail` を実行し成功する。確認: テスト出力が PASS で終了する。

## Acceptance #1 Failure Follow-up
- [x] Git 作業ツリーが dirty のままです。未コミットの変更: `openspec/changes/add-acceptance-tail-to-apply/tasks.md`、`src/agent/prompt.rs`、`src/agent/runner.rs`、`src/agent/tests.rs`、`src/execution/apply.rs`、`src/parallel/executor.rs`、`src/parallel/mod.rs`、`src/parallel/tests/executor.rs`

## Acceptance #2 Failure Follow-up
- [x] `src/parallel/executor.rs:execute_apply_in_workspace` が `acceptance_history` から `<last_acceptance_output>` を構築する一方、`src/parallel/executor.rs:execute_acceptance_in_workspace` は `AgentRunner` の履歴のみ更新して `ParallelExecutor.acceptance_history` を更新していないため、並列モードで acceptance 失敗後の apply に tail が注入されない。acceptance 実行時に共有 `acceptance_history` を更新する。
