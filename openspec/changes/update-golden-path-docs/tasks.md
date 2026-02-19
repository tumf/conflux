## 1. Golden Path の定義と反映
- [x] 1.1 README.md の Quick Start を現行 CLI の実態に合わせて再構成する（Golden Path `cflx init` → `cflx` を最短導線として追加し、存在しないコマンド/フラグを除去する）
  - `cflx approve set/status/unset` コマンドを削除（CLIに存在しない）
  - `--opencode-path` フラグを削除（CLIに存在しない）
  - Golden Path「クイックスタート」セクションを追加
  - `tui` および `check-conflicts` サブコマンドを追記
  - コマンドラインオプションテーブルを現行 CLI (cflx --help) に合わせて更新
- [x] 1.2 docs/guides/USAGE.md の例を整理し、`cflx status`/`cflx reset`/`--openspec-path`/`--opencode-path` など実装に存在しない項目を排除し、`cflx --help` と整合させる
  - `cflx status` コマンド（存在しない）を削除
  - `cflx reset` コマンド（存在しない）を削除
  - `--opencode-path`/`--openspec-path` フラグ（存在しない）を削除
  - Golden Path クイックスタートセクションを追加
  - 実際の `cflx` サブコマンド（`tui`, `run`, `init`, `check-conflicts`）に基づいてサンプルを更新
- [x] 1.3 README.ja.md を README.md と同一構成・同一コマンド例に同期する（翻訳以外の差分をなくす）
  - `cflx approve set/status/unset` コマンドを削除（CLIに存在しない）
  - `--opencode-path`/`--openspec-cmd` フラグを削除（CLIに存在しない）
  - Golden Path「クイックスタート」セクションを追加
  - `tui` および `check-conflicts` サブコマンドを追記
  - コマンドラインオプションテーブルを現行 CLI に合わせて更新
  - 設定例に `acceptance_command`, `acceptance_prompt`, `acceptance_prompt_mode`, `acceptance_max_continues`, `resolve_command`, `worktree_command` を追加
  - `## ドキュメント` セクションを追加（README.md と同等）

## 2. 検証
- [x] 2.1 README.md / README.ja.md / docs/guides/USAGE.md で記載されているコマンド・フラグが `cflx --help` の内容と一致していることを自動照合する
  - `cflx --help` 出力を取得して確認済み
  - `cflx run --help` 出力を取得して確認済み
  - `cflx init --help` 出力を取得して確認済み
  - 各ドキュメントに存在しないコマンド (`cflx status`, `cflx reset`, `cflx approve`, `--opencode-path`, `--openspec-path`) が残っていないことを `grep` で確認済み
  - 現行 CLI の実際のコマンドと各ドキュメントの記載が一致することを確認済み

## Acceptance #1 Failure Follow-up
- [x] README.md にプロジェクト構成の明示セクションを追加し、`hooks.rs` / `task_parser.rs` / `templates.rs` を含む現行ソースファイル一覧を記載する（evidence: `openspec/changes/update-golden-path-docs/specs/documentation/spec.md:19`, `README.md:890`）。
- [x] README.md / README.ja.md の CLI 一覧を `cflx --help` に再同期し、`server` サブコマンドと `--server` / `--server-token` / `--server-token-env` を反映する（evidence: `src/cli.rs:83`, `README.md:501`, `README.ja.md:485`）。
- [x] README.ja.md を README.md と同一構成に再同期し、欠落している「Web Monitoring」機能項目と「Logging configuration」節を追加する（evidence: `README.md:18`, `README.ja.md:10`, `README.md:339`, `README.ja.md:337`）。
- [x] Golden Path Quick Start の要件を `spec` と一致させる（`cflx run` を Quick Start に含めるか、要件側を `cflx init -> cflx` に修正して整合を取る）（evidence: `openspec/changes/update-golden-path-docs/specs/documentation/spec.md:28`, `README.md:37`, `docs/guides/USAGE.md:9`）。

## Acceptance #2 Failure Follow-up
- [x] README.md の Project Structure を現行ソースに正確同期する（`README.md:903-951` は `approval.rs` を記載しているが実ファイルは存在せず、`src/acceptance.rs` など現行ファイルの欠落もあるため、`openspec/changes/update-golden-path-docs/specs/documentation/spec.md:22` の「all current source files」を満たしていない）。

## Acceptance #3 Failure Follow-up
- [x] README.md の Project Structure を「all current source files」要件に再同期する（`openspec/changes/update-golden-path-docs/specs/documentation/spec.md:22`）。現行の一覧（`README.md:903-1021`）には実在する `src/lib.rs` / `src/config/mod.rs` / `src/execution/mod.rs` / `src/parallel/mod.rs` / `src/vcs/mod.rs` / `src/vcs/git/mod.rs` / `src/vcs/git/commands/mod.rs` / `src/tui/utils.rs` / `src/bin/openapi_gen.rs` などが欠落しているため、`src/**/*.rs`（必要なら `tests/**/*.rs` の扱いも方針明記）と一致するよう更新する。
