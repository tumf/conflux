# Change: hookコマンドと出力のLogs View表示

## Why
hook実行時のコマンドと出力がLogs Viewに表示されず、実行確認やトラブルシュートが難しいため。

## What Changes
- hook実行時に、コマンド文字列をLogs Viewへ必ず出力する
- hookのstdout/stderrを取得し、Logs Viewへ表示する
- serial/parallel/TUIの経路で同一のログイベントを扱う

## Impact
- Affected specs: observability
- Affected code: hooks, events, orchestrator, tui, parallel
