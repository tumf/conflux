# Change: ログの常時ファイル出力と日次保持の標準化

## Why
現在は `tui --logs` 指定時のみログがファイルに出力されるため、TUI/CLI 実行のログが一貫して残らず調査が難しい。macOS と Linux の保存先も統一したい。

## What Changes
- `tui --logs` オプションを廃止し、常にファイルへログを出力する
- 出力先を `XDG_STATE_HOME`（未設定時は `~/.local/state`）配下に統一する
- `project_slug` と日付でログファイルを分離し、7日分のみ保持する
- TUI/CLI の両方で同一のログ保存ポリシーを適用する

## Impact
- Affected specs: observability, cli
- Affected code: cli, main, logging initialization, config/logging utilities
