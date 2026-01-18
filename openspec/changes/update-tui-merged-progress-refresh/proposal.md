# Change: Merged後のタスク進捗をアーカイブから再取得

## Why
TUIのMerged表示では進捗が更新されず、実際には完了しているタスクが0/4などの古い値のまま残ることがある。アーカイブ済みのtasks.mdを読み直して正しい進捗を表示し、ユーザーの状況認識を正確にする。

## What Changes
- Merged/Archived/Resolve完了時に、アーカイブ内のtasks.mdから進捗を再取得して反映する。
- 読み込みに失敗した場合は既存の進捗値を維持する。

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/state/events.rs
