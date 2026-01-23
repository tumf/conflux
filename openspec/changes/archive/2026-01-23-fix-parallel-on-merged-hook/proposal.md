# Change: パラレルモードのon_mergedフック実行漏れ修正

## Why
パラレルモードで自動マージが成功しても`on_merged`が呼ばれず、フックに依存する後処理が実行されません。

## What Changes
- パラレルモードのマージ成功経路で`on_merged`を必ず実行する（即時マージ/再解析/動的キュー/再開済みマージ）。
- hooks仕様でパラレルモードのマージ成功経路における`on_merged`実行条件を明確化する。

## Impact
- Affected specs: hooks
- Affected code: src/parallel/mod.rs
