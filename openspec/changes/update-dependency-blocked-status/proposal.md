# Change: Update dependency-blocked change status

## Why
依存関係の解決待ちで実行できない change が TUI/Web で "not queued" と表示され、実行待ち理由が伝わらず運用判断を誤る可能性があります。

## What Changes
- 依存関係未解決の change を可視化するステータス語彙を追加する
- 並列実行の依存関係待ちを示すイベントと TUI/Web 表示を更新する
- 既存の "not queued" 表示は手動未キュー状態に限定する

## Impact
- Affected specs: parallel-execution, cli, web-monitoring
- Affected code: src/parallel/mod.rs, src/tui/state, src/web/state
