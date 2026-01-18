# Change: 再開時に acceptance を必ず実行する

## Why
acceptance 実行中にオーケストレーションが中断した場合、再開時に acceptance をスキップして archive が進む可能性があり、品質担保が弱くなっているためです。

## What Changes
- 既存ワークスペースを resume した場合、archive が未完了であれば acceptance を必ず再実行する
- acceptance の実行結果が保持されない前提で再開時の動作を明確化する

## Impact
- Affected specs: parallel-execution
- Affected code: src/execution/state.rs, src/parallel/mod.rs
