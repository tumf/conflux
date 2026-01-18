# Change: Fix slot availability counting in parallel execution

## Why
並列実行中に空きスロットがあるにもかかわらず再分析が走らず、キュー追加が反映されないケースが発生しているため、正しいアクティブ判定で再分析と起動が進むようにする。

## What Changes
- 実行中 change のみをアクティブとして数え、空きスロット計算を修正する
- 停止状態（merged / merge_wait / error / not queued）はアクティブに含めない
- TUI/CLI の並列実行で同じアクティブ判定を用いる

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs, src/vcs/git/mod.rs, src/vcs/mod.rs
