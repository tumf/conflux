# Change: 実行中の変更を依存分析に含める

## Why
Dynamic キューで変更を追加した際、実行中（applying）の変更が依存分析対象に含まれず、依存関係が崩れて誤った順序で apply されることがあるためです。

## What Changes
- 依存分析プロンプトに「実行中の変更（in-flight）」を含め、依存関係判定に反映させる
- 実行中の変更は選択対象ではないことを明示し、依存のみとして扱う

## Impact
- Affected specs: parallel-analysis
- Affected code: src/parallel/mod.rs, src/parallel_run_service.rs, src/agent/runner.rs（分析プロンプト構築）
