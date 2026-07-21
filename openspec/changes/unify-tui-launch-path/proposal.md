---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/main.rs
  - src/cli.rs
  - tests/run_exit_tests.rs
  - openspec/specs/code-maintenance/spec.md
verifications:
  - id: tui-launch-parity-tests
    requirement: Bare and explicit TUI entrypoints retain identical initialization and validation behavior
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: binary integration tests covering equivalent bare and explicit TUI invocations plus feature build checks
    rerun: cargo test --test run_exit_tests && cargo check --all-features && cargo check --no-default-features
    prerequisites: []
---

# TUI起動経路を単一helperへ統合する

**Change Type**: implementation

## Problem/Context

`src/main.rs:486-568` のbare `cflx` 起動と `src/main.rs:570-641` の明示的な `cflx tui` 起動は、同じTUI初期化を別々に実装している。両方がconfig load、remote/local change取得、optional web monitor起動、remote client生成、`run_tui_with_remote` 呼び出しを持ち、差分はglobal flagsから `TuiArgs` を組み立てる入口部分だけである。

現在も `tests/run_exit_tests.rs:107-130` が `--push` と `--server` の拒否を両入口で個別に確認している。今後のTUI起動変更を片方へだけ反映するdriftを防ぐため、既存のentrypoint contractをcharacterization testで固定して単一helperへ委譲する。

## Proposed Solution

- `TuiArgs` を受け取り、TUI起動のvalidation・logging・config・change source・web monitor・remote client・run呼び出しを担当する1つのasync helperを `src/main.rs` 内に抽出する。
- bare entrypointはglobal flagsから現在と同じ `TuiArgs` を構築し、明示的entrypointはparse済み `TuiArgs` を渡す。
- 既存の `tui_post_archive_action` validation順、remote/local selection、feature-gated warning、web startup failure fallbackを維持する。
- 新しいmodule、trait、依存crateは追加しない。

## Acceptance Criteria

- bare `cflx` と `cflx tui` は同じhelperを通ってTUIを起動する。
- equivalent flagsに対するconfig path、web settings、push behavior、server endpoint/token、remote/local change sourceは現在と同じである。
- `--push` と `--server` の競合は両入口でTUI初期化前に同じように拒否される。
- web-monitoring feature有効時のserver起動と失敗fallback、無効時のwarningは変わらない。
- `run_tui_with_remote` へ渡す値とCLI exit behaviorは変わらない。

## Explicit Completion Conditions

- `src/main.rs` の2つのmatch armは同じasync helperを呼び、TUI初期化本体が重複していない。
- `tests/run_exit_tests.rs` にbare/explicit entrypointの同等なvalidation結果を固定するbinary integration coverageがある。
- default featuresとno-default-featuresの両方でmain binaryがcompileする。
- `cargo test --test run_exit_tests`、`cargo check --all-features`、`cargo check --no-default-features`、`cargo fmt --all -- --check` が成功する。

## Out of Scope

- TUI画面、event loop、state reducer、remote protocolの変更。
- `run` subcommandのweb monitor初期化との統合。
- CLI flag名、default値、認証方式、ログ形式の変更。
- 新しいTUI起動abstractionや複数実装を想定したtraitの追加。
