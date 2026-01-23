# Change: acceptance 初回でも base 差分レビューを行う

## Why
acceptance の初回実行では差分コンテキストが付与されず、レビュー範囲が広くなって再調査が発生しやすい。base branch との差分を提示することで、初回から変更範囲に集中した検証を行えるようにする。

## What Changes
- 初回 acceptance でも base branch → 現在コミットの差分ファイルを `<acceptance_diff_context>` に含める
- 2回目以降の差分ロジックを維持し、diff context を acceptance プロンプトに確実に挿入する
- ACCEPTANCE_SYSTEM_PROMPT に diff context の使い方と優先レビュー指示を追加する

## Impact
- Affected specs: agent-prompts
- Affected code: src/agent/runner.rs, src/parallel/executor.rs, src/agent/prompt.rs, src/config/defaults.rs
