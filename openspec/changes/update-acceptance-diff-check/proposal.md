# Change: Accept once, then diff-based acceptance

## Why
現状の acceptance は CONTINUE のたびに網羅的チェックを再実行しており、確認済み事項を繰り返し検証してしまう。1回目の網羅的チェック結果を前提に、2回目以降は差分と指摘事項に集中させることで、検証の効率と収束性を高めたい。

## What Changes
- 1回目の acceptance は現行通り網羅的に検証する
- 2回目以降の acceptance は前回チェック以降に更新されたファイル一覧と前回の指摘事項のみを対象に検証する
- acceptance プロンプトに「必要に応じて対象ファイルを読んで確認する」方針を明記する

## Impact
- Affected specs: cli, parallel-execution
- Affected code: acceptance prompt generation, acceptance history tracking
