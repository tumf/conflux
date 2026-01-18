# Change: Merge stall circuit breaker

## Why
長時間ベースブランチに merge コミットが反映されない場合、オーケストレーションが実質停止していることに気づけない。30分以上 merge 進捗が無い状態を明示的に stall と判定し、即時停止して復旧判断をしやすくする。

## What Changes
- ベースブランチの merge コミット進捗を監視し、30分間進捗が無い場合にオーケストレーションを即時停止する
- 監視対象を serial/parallel の両モードに適用する
- 停止理由を CLI/TUI/Web のイベント/ログに反映する
- 監視間隔と閾値を設定で上書き可能にする

## Impact
- Affected specs: circuit-breaker, configuration, parallel-execution, cli, tui-architecture, web-monitoring
- Affected code: orchestrator run loop, parallel executor run loop, config parsing, web state updates
