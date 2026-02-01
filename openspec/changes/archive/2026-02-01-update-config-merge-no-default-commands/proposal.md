# Change: 設定マージとコマンド既定値の撤廃

## Why
部分的な `.cflx.jsonc` が存在するとグローバル設定が無視され、意図しない `DEFAULT_*_COMMAND` が実行される問題があるため、設定をマージ型にし、コマンド既定値へのフォールバックを廃止する必要がある。

## What Changes
- 設定読み込みをマージ型に変更し、優先度の高い設定が存在する項目のみ上書きする
- コマンド系 (`apply/archive/analyze/acceptance/resolve`) の既定値フォールバックを廃止し、未設定はエラーとする
- 既存の設定優先順位とエラーメッセージを仕様化し、単体テストで保証する

## Impact
- Affected specs: `openspec/specs/configuration/spec.md`
- Affected code: `src/config/mod.rs`, `src/config/defaults.rs`, `src/error.rs`（構成上のエラー整備）, 設定読み込みのテスト
