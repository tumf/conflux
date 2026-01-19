# Change: completedを廃止して即時遷移にする

## Why
TUI/Webのキュー状態に「completed」が挟まることで、処理が停止したように見えたり、次フェーズ（archiving/merged）への遷移が分かりにくくなるため。

## What Changes
- `completed` を中間状態として使用せず、完了条件を満たした change は必ず archiving フェーズへ即時遷移させる（completed で停止するケースは存在しない）
- completed で停止する挙動は一切許容しない（いかなる場合も archiving に進む）
- completed 状態は観測できない（中間表示や集計に現れない）
- completed が state_update に載ることはない（いかなる場合も送信しない）
- TUI の QueueStatus と Web ダッシュボードの表示語彙から `completed` を除外する（status/集計ともに使用しない）
- 進捗カウント（completed_tasks/total_tasks）は引き続き保持し、archiving 以降の状態でも表示可能にする

## Impact
- Affected specs: cli, web-monitoring
- Affected code: TUI 状態遷移、TUI オーケストレーション、Web 状態更新、archiving フロー
