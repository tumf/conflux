# Change: Update TUI orchestration status display

## Why
MergeWait が残って実行ループが終了しているにも関わらず、オーケストレーションが Running と表示され続けるため、実行状態が誤解される。
ヘッダーとステータス行の表示内容も現状の実行可能状態と合っていない。

## What Changes
- MergeWait が残っていても、並列実行の処理ループが終了したら AllCompleted を送信して Select に遷移する。
- ヘッダー表示を Ready/Running に整理し、Select Mode 表記を廃止する。
- ステータス行は選択中の change 進捗と Running 累計時間のみを表示する。

## Impact
- Affected specs: parallel-execution, cli
- Affected code: src/parallel/mod.rs, src/tui/render.rs
