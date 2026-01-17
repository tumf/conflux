# Change: worktreeのデフォルト作成先を永続ディレクトリへ変更

## Why
/tmp がRAMディスクになり得る環境では worktree が消えやすく、作業中の提案が失われる可能性があるため、より永続的で標準的な場所にデフォルトを置きたい。

## What Changes
- デフォルトの worktree 基準ディレクトリを OS 標準のユーザーデータ領域へ変更する
- macOS では XDG_DATA_HOME が設定されている場合はそれを優先し、未設定なら Application Support を使用する
- Linux では XDG_DATA_HOME（未設定時は ~/.local/share）を使用する

## Impact
- Affected specs: configuration, tui-worktree-view, parallel-execution
- Affected code: `src/tui/runner.rs`, `src/parallel/mod.rs`, `src/config/defaults.rs` (想定)
