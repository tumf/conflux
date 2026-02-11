# Change: acceptance の埋め込みプロンプト削除と context-only への統一

## Why
acceptance の固定手順がコマンドテンプレートと埋め込みプロンプトの両方から供給され、二重化や不整合が発生しうるため、単一ソースに統一します。

## What Changes
- acceptance の埋め込みシステムプロンプトを削除し、固定手順はコマンドテンプレートのみから供給する
- acceptance_prompt_mode の full を非推奨互換として context_only 相当の挙動に統一する

## Impact
- Affected specs: agent-prompts
- Affected code: src/config/defaults.rs, src/config/mod.rs, src/agent/prompt.rs, src/agent/runner.rs, src/parallel/executor.rs
