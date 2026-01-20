# Change: Gate re-analysis by available execution slots

## Why
現在の並列実行では、実行スロットが空いていない場合でも re-analysis が走るため、実行開始に直結しない分析が発生し、無駄な計算コストとログノイズが増える。実行スロットが空くまで re-analysis を保留することで、実行タイミングに合わせた分析に限定したい。

## What Changes
- 並列実行の re-analysis を「空きスロットがある時のみ実行」に変更する
- 空きスロットがない場合は re-analysis を保留し、スロットが空いたタイミングで再評価する

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs (re-analysis gating)
