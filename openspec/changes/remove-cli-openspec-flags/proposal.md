# Change: CLIのOpenSpec/Opencode関連フラグ削除とヘルプ拡張

## Why
CLIの `--opencode-path` と `--openspec-cmd` が既に非推奨になっており、設定ファイル主導の運用に統一するため削除する必要がある。あわせて `cflx --help` に全サブコマンドと主要オプション一覧を明示し、実行時の確認コストを下げる。

## What Changes
- `cflx` CLIから `--opencode-path` と `--openspec-cmd` を削除する
- `cflx --help` に全サブコマンドと主要オプション一覧（`--web`/`--web-port`/`--web-bind` など）を追加する
- 仕様上の OpenSpec コマンド上書き（`OPENSPEC_CMD` / `--openspec-cmd`）を廃止する

## Impact
- Affected specs: cli, configuration
- Affected code: src/cli.rs, src/main.rs, src/tui/runner.rs, src/tui/orchestrator.rs
