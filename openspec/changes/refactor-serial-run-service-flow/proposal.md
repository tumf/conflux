# Change: SerialRunService のapply/acceptance分割

## Why
`SerialRunService` の apply/acceptance 処理が肥大化しており、変更時の影響範囲が大きくなっています。

## What Changes
- apply 後の再取得、acceptance 判定、結果処理をヘルパーに分割する
- 既存のフロー順序・出力・判定基準を維持する

## Impact
- Affected specs: code-maintenance
- Affected code: src/serial_run_service.rs
