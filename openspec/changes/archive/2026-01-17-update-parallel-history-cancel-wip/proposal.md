# Change: 共通ループによる履歴注入と停止処理の統一

## Why
逐次と並列で apply/archive の履歴注入や停止判定が別実装になっており、同じ挙動を保つための保守コストが高くなっています。

## What Changes
- apply/archive の履歴注入を共通ループに集約し、逐次/並列で同一の注入ロジックを使用する
- キャンセル/停止の判定と処理フローを共通化し、モード差をなくす
- WIP スナップショットと進捗停滞時の扱いを共通化して挙動を維持する

## Impact
- Affected specs: cli
- Affected code: src/execution, src/orchestrator, src/parallel
