# Change: Update archive state handling and task completion gates

## Why
並列実行で、ファイルはアーカイブ済みだがコミットが未完了の状態が存在し、apply/archiving の判定がずれることがある。あわせて、Future Work 移動の基準が「面倒だから移動」に誤用されるのを防ぎ、人間作業・外部作業のみを対象として明確化したい。さらに、tasks.md が100%完了していない状態で archive に進むことを防ぐ必要がある。

## What Changes
- archive 状態を「archiving（ファイルのみ）」「archived（コミット完了）」で区別する
- worktree 内のファイルアーカイブ状態に応じて apply/archiving の遷移を更新する
- Future Work 移動の許可条件を人間作業・外部作業・長時間待ちに限定する
- tasks.md が100%完了していない場合は archive に進まない

## Impact
- Affected specs: parallel-execution, agent-prompts
- Affected code: src/execution/state.rs, src/parallel/mod.rs, src/parallel/executor.rs, src/agent.rs
