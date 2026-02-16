# Change: コマンド無出力タイムアウトの導入

## Why
AIエージェント実行が長時間無出力のまま継続すると、停止判定や原因追跡が難しくなるため、無出力を検出して安全に中断できる仕組みが必要です。

## What Changes
- AIコマンド（apply/archive/resolve/analyze/acceptance など）の無出力監視タイムアウトを追加する
- タイムアウト検知時に警告ログを出し、所定の猶予後に強制終了する
- 無出力タイムアウトは設定で有効/無効や時間を調整できる

## Impact
- Affected specs: command-queue, configuration, observability
- Affected code: src/command_queue.rs, src/ai_command_runner.rs, src/agent/runner.rs, src/config/defaults.rs, src/config/mod.rs
