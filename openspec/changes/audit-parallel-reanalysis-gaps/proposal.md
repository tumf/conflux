# Change: 並列再分析の未反映ギャップ監査と反映

## Why
並列実行の再分析が `order` 方式に移行したはずなのに、現行実装が group 変換前提のまま動作している可能性が高い。仕様と実装の不整合が続くと、再分析の起動判断や依存解決が不正確になり、並列実行の信頼性が落ちる。

## What Changes
- 仕様（parallel-execution / parallel-analysis）と実装の差分を徹底的に棚卸しする
- `order` ベースの再分析に合わせて実装を修正し、スロット駆動の起動を保証する
- CLI/TUI の経路で挙動差が出ないように実行ループを整理する
- 再分析のギャップを検出できるテストとログを追加する

## Impact
- Affected specs: parallel-execution, parallel-analysis
- Affected code: parallel executor, parallel run service, analyzer
