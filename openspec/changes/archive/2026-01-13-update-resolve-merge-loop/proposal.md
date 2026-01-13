# Change: コンフリクト解決後のマージ完了ループを追加

## Why
並列実行のマージコンフリクト解決後にマージが完了せず停止するケースがあり、apply/archive と同様に完了までの再試行が必要です。

## What Changes
- コンフリクト解決後にマージ完了まで再試行するループを定義する
- 解決→再マージの繰り返しが上限回数で失敗した場合の扱いを明確化する

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/parallel/mod.rs`, `src/parallel/conflict.rs`
