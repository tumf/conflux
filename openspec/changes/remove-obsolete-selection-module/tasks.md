## Implementation Tasks

- [x] repository-wide symbol searchで `src/orchestration/selection.rs` の関数が同moduleのtest以外から到達不能であることを確認する (verification: manual - `src/orchestration/selection.rs`、`src/orchestration/mod.rs`、全 `src/**/*.rs` の参照結果をreviewし、現役ownerが `src/serial_run_service.rs` とparallel analyzerであることを記録する)
  - evidence: `rg` で旧 `select_next_change` の参照は同module内testだけと確認。serial ownerは `src/orchestrator.rs` と `src/tui/orchestrator.rs` が呼ぶ `SerialRunService::select_next_change`、parallel ownerは既存のanalyzer/order-based dispatchであり、削除対象moduleへの参照はない。
- [x] `src/orchestration/selection.rs` と `src/orchestration/mod.rs` のmodule宣言・古い責務説明を削除する (verification: integration - source pathから `orchestration::selection` 参照がなく、`cargo check --all-features` が成功する)
  - evidence: `src/orchestration/selection.rs` を削除し、`src/orchestration/mod.rs` からmodule宣言と古いselection責務・統合中の説明を削除。repository-wide source searchで旧module参照なし。`cargo check --all-features` 成功。
- [x] 現役serial selectionの既存testでincomplete優先、stalled除外、progress優先のcontractを確認する (verification: unit - `src/serial_run_service.rs` のselection testsを `cargo test serial_run_service::tests --lib` で実行する)
  - evidence: `cargo test serial_run_service::tests --lib` 成功（21 passed、0 failed、0 ignored、default suite実行時間0.59s）。
- [x] module削除後のformatとlintを実行し、dead-code suppressionやunused importの残骸がないことを確認する (verification: integration - `src/orchestration/mod.rs` と関連moduleを対象に `cargo fmt --all -- --check` と `cargo clippy -- -D warnings` が成功する)
  - evidence: `cargo fmt --all -- --check`、`cargo clippy -- -D warnings`、旧module参照検索、`git diff --check` が成功。削除により露出した既存互換APIのdead-code警告は `touch_legacy_api_symbols` に既存方針どおり登録した。

## Future Work

serial mode自体の廃止判断はruntime behaviorとcanonical specに関わるため、このdead-code削除には含めない。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate remove-obsolete-selection-module --archive-gate`
