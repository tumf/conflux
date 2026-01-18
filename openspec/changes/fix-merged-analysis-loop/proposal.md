# Change: merged済みchangeの再解析ループ停止

## Why
queued対象のみをanalysisする前提にもかかわらず、merged済みchangeがanalysisに残り続け、実行中のchangeがなくなってもオーケストレーションが停止しない問題があるため。queuedのみを対象とすること、queuedと実行中が両方空なら終了することを明確化する。

## What Changes
- analysis対象はqueuedに限定する。
- 実行中のchangeがなく、queuedも空の場合はオーケストレーションを終了する。
- queueが空のときはanalysisを実行しない。
- queued外のchange（merged済み、実行済み、削除済み）はanalysis対象から除外する。

## Impact
- Affected specs: parallel-execution
- Affected code: 並列実行ループ、queue処理、終了判定
