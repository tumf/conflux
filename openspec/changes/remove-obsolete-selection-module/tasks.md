## Implementation Tasks

- [ ] repository-wide symbol searchで `src/orchestration/selection.rs` の関数が同moduleのtest以外から到達不能であることを確認する (verification: manual - `src/orchestration/selection.rs`、`src/orchestration/mod.rs`、全 `src/**/*.rs` の参照結果をreviewし、現役ownerが `src/serial_run_service.rs` とparallel analyzerであることを記録する)
- [ ] `src/orchestration/selection.rs` と `src/orchestration/mod.rs` のmodule宣言・古い責務説明を削除する (verification: integration - source pathから `orchestration::selection` 参照がなく、`cargo check --all-features` が成功する)
- [ ] 現役serial selectionの既存testでincomplete優先、stalled除外、progress優先のcontractを確認する (verification: unit - `src/serial_run_service.rs` のselection testsを `cargo test serial_run_service::tests --lib` で実行する)
- [ ] module削除後のformatとlintを実行し、dead-code suppressionやunused importの残骸がないことを確認する (verification: integration - `src/orchestration/mod.rs` と関連moduleを対象に `cargo fmt --all -- --check` と `cargo clippy -- -D warnings` が成功する)

## Future Work

serial mode自体の廃止判断はruntime behaviorとcanonical specに関わるため、このdead-code削除には含めない。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate remove-obsolete-selection-module --archive-gate`
