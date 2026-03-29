## Implementation Tasks

- [x] 1. `OutputHandler` trait に `on_agent_stderr(&self, line: &str)` メソッドを追加する（`src/orchestration/output.rs`）。verification: `cargo build` が通り、trait に新メソッドが存在する。
- [x] 2. `LogOutputHandler` で `on_agent_stderr` を `info!(target: "orchestrator::output", ...)` として実装する。verification: `cargo test --lib orchestration::output`。
- [x] 3. `NullOutputHandler`, `ChannelOutputHandler`, `ContextualOutputHandler` に `on_agent_stderr` を実装する。verification: `cargo build` が通る。
- [x] 4. apply / archive / acceptance のストリーミング出力コールバックで、`OutputLine::Stderr` の送出先を `on_stderr` から `on_agent_stderr` に変更する（`src/orchestration/apply.rs`, `src/orchestration/archive.rs`, `src/orchestration/acceptance.rs`）。verification: `cargo test` が通る。
- [x] 5. `serial_run_service.rs` と `parallel/executor.rs` のコールバックで `OutputLine::Stderr` を `on_agent_stderr` に変更する。verification: `cargo test` が通る。
- [x] 6. 既存テストの更新と新規テスト追加。verification: `cargo test` が通り、`on_agent_stderr` の info レベル出力を検証するテストが存在する。
- [x] 7. `cargo fmt --check && cargo clippy -- -D warnings` が通ることを確認する。verification: lint/clippy エラーなし。
