# Change: 依存関係解析の実行と解析の分割

## Why
依存関係解析の実行・ストリーミング収集・解析が単一関数に集中しており、保守性が低下しています。

## What Changes
- 解析コマンド実行と出力解析/検証を分割する
- 既存のプロンプト内容と判定結果を維持する

## Impact
- Affected specs: parallel-analysis
- Affected code: src/analyzer.rs
