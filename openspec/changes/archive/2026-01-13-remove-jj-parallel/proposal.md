# Change: jj workspace並列の廃止とgit worktreeデフォルト化

## Why
jj workspaceを前提とした並列実行は導入負荷が高く、Git環境のみで完結させたいという要望があるためです。Git worktreeに統一することで並列実行の前提を明確化し、運用負担を削減します。

## What Changes
- 並列実行のVCSバックエンドをgit worktreeのみに統一する
- CLI/TUI/設定からjj依存の検出・オプション・エラーメッセージを削除する
- `--vcs jj` と `vcs_backend: "jj"` の選択肢を廃止する
- **BREAKING**: jj workspace並列とjj前提の自動判定が利用不可になる

## Impact
- Affected specs: parallel-execution, cli, configuration, tui-editor
- Affected code: `src/parallel/`, `src/vcs/`, `src/cli.rs`, `src/tui/`, `src/config/`
