# Change: merged済みchangeの再解析ループ停止

## Why
merged済みのchangeがanalysisにかけられ続け、全件完了後もオーケストレーションが停止しない問題を解消し、並列実行の完了判定を正しく行うため。

## What Changes
- mergedと判定できるchangeをanalysis対象から除外する。
- analysis前に除外対象が空になった場合はオーケストレーションを終了する。
- 除外理由がログ・イベントで明示されるようにする。

## Impact
- Affected specs: parallel-execution
- Affected code: 並列実行ループ、workspace state判定、イベント送出
