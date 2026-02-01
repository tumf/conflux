# Change: acceptance プロンプトの単一ソース化

## Why
acceptance の固定手順が複数箇所に存在すると、指示の重複や矛盾が発生しやすくなります。固定手順の出所を一つに統一し、可変コンテキストのみを渡すことで運用の安定性を高めます。

## What Changes
- acceptance の固定手順を `.opencode/commands/cflx-accept.md` に集約し、オーケストレーターは可変コンテキストのみを渡す
- デフォルトテンプレートで `acceptance_prompt_mode: context_only` を採用する
- `acceptance_command` が `cflx-accept` を参照する前提を明確化する

## Impact
- Affected specs: agent-prompts
- Affected code: src/templates.rs, src/config/defaults.rs, .opencode/commands/cflx-accept.md
