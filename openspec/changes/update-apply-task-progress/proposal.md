# Change: Applyがtasks.mdを更新し続けるようプロンプトを強化する

## Why
apply実行時にtasks.mdの更新が遅延・欠落することがあり、進捗の可視性とタスクの整合性が損なわれています。

## What Changes
- apply用システムプロンプトに、タスク完了時の即時更新と終了前整合確認を明記する
- タスクの分割・具体化が発生した場合も同時にtasks.mdを更新する指示を追加する

## Impact
- Affected specs: agent-prompts
- Affected code: src/agent.rs (APPLY_SYSTEM_PROMPT)
