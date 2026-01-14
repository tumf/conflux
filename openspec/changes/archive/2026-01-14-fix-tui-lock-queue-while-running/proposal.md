# Change: 実行中のキュー削除が未着手 change に反映される

## Why
TUI の serial 実行中に、承認済みだが選択（マーク）が外れた change がそのまま実行されてしまうことがある。
これは「実行対象の確定（pending）と、実行中の UI 操作（マーク外し）が同期しない」ために起き、ユーザーの意図と実際の実行がズレる。

本変更では、実行中でも「未着手の queued change」を外した場合に必ず実行対象から除外されるようにし、意図しない実行を防ぐ。
（Processing/Archiving の change は従来通り操作不可のまま。）

## What Changes
- Running 中に queued change を外した場合、未着手であれば実行対象から除外される
- キュー削除は共有キュー/オーケストレータ側の pending にも反映される
- Processing/Archiving 中の change は引き続き操作不可

## Impact
- Affected specs: `openspec/specs/cli/spec.md`（Dynamic Execution Queue の挙動）
- Affected code (予定): `src/tui/state/mod.rs`（selection/queue 操作）, `src/tui/runner.rs`, `src/tui/orchestrator.rs`
- User impact: 実行中にマークを外した change が実行される問題を解消する

## Non-Goals
- Processing 中の change を中断・キャンセルする挙動は追加しない
