## Implementation Tasks

- [ ] bare `cflx` と `cflx tui` の現在の引数validation parityをbinary characterization testで固定する (verification: integration - `tests/run_exit_tests.rs` でequivalent invocationsのexit statusと事前validationを確認し、`cargo test --test run_exit_tests` が成功する)
- [ ] `TuiArgs` を受け取ってpost-archive validation、logging、config load、change取得を行う単一async helperを `src/main.rs` に抽出する (verification: integration - `src/main.rs` のbare/explicit match armが同じhelperを呼び、既存binary integration testsが成功する)
- [ ] optional web monitor起動、feature無効warning、remote client生成、`run_tui_with_remote` 呼び出しをhelperへ移し、現在のfallbackと引数を維持する (verification: integration - `src/main.rs` の両cfg branchを `cargo check --all-features` と `cargo check --no-default-features` でcompileする)
- [ ] bare entrypointのglobal flagsから現在と同じ `TuiArgs` を構築し、explicit entrypointのparse済み値と同じ起動処理へ渡す (verification: integration - `tests/run_exit_tests.rs` のbare/explicit parity casesと `src/cli.rs` のTUI argument testsが成功する)
- [ ] formattingとlintを実行し、抽出後のunused importや重複初期化がないことを確認する (verification: integration - `src/main.rs` を対象に `cargo fmt --all -- --check` と `cargo clippy -- -D warnings` が成功する)

## Future Work

`run` subcommandのweb monitor起動との共通化は、戻り値とlogging contractが異なるため別提案で扱う。

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-tui-launch-path --archive-gate`
