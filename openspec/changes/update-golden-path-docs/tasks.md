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
