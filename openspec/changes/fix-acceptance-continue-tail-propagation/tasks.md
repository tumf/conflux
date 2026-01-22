# Tasks

## 1. 不要なコード変更をリバート
- [x] 1.1 `src/orchestration/acceptance.rs` の `AcceptanceResult::Continue` を元の unit variant に戻す
  - 検証: `AcceptanceResult::Continue` がフィールドを持たない
- [x] 1.2 `src/acceptance.rs` の `AcceptanceResult::Continue` も同様にリバート
  - 検証: パーサー側も unit variant に戻っている
- [x] 1.3 `src/serial_run_service.rs` の `ChangeProcessResult::AcceptanceContinue` をリバート
  - 検証: unit variant に戻っている
- [x] 1.4 関連するマッチアーム（parallel/mod.rs, orchestrator.rs, tui/orchestrator.rs）をリバート
  - 検証: `cargo build` が成功

## 2. AcceptanceHistory から前回の出力を取得するメソッド追加
- [x] 2.1 `AcceptanceHistory` に `get_last_attempt()` または `get_last_tail()` メソッドを追加
  - 検証: `src/history.rs` に新しいメソッドが追加されている
- [x] 2.2 `AgentRunner` に前回の acceptance 出力を取得するヘルパーメソッドを追加
  - 検証: `src/agent/runner.rs` で `AcceptanceHistory` から tail を取得できる

## 3. Acceptance プロンプトに前回の出力を含める
- [x] 3.1 acceptance プロンプト生成時に、前回の acceptance tail（stdout_tail/stderr_tail）をプロンプトに含める
  - 検証: `src/agent/prompt.rs` または `src/config/defaults.rs` の acceptance プロンプト生成で tail が追加される
- [x] 3.2 プロンプトに含める際は `<last_acceptance_output>` タグで囲む
  - 検証: タグが正しく挿入されている

## 4. テストと検証
- [x] 4.1 `cargo fmt && cargo clippy && cargo test` で全体検証
  - 検証: エラーなし
