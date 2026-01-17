# Change: applyエージェントの反復実行ポリシーを明文化

## Why
applyエージェントが「回帰リスクが高い」「テストが必要」といった理由でタスクを Future Work に移し、実装が完了しないまま終わるケースが発生した。現在の apply system prompt は Future Work への移動条件を定義しているが、ユーザーへの質問ができない運用前提や MaxIteration まで継続すべき期待動作が明示されていない。

## What Changes
- apply system prompt に「質問は不可」「MaxIteration まで最善を尽くして継続する」ことを明記する
- 難易度や回帰リスクを理由に Future Work へ移すことを禁止する
- 例外は、既に `(future work)` と明記されたタスクのみとする

## Impact
- Affected specs: agent-prompts
- Affected code: src/agent.rs (APPLY_SYSTEM_PROMPT)
