# Change: TUI/Web UIのステータス語彙と表示形式を明確化する

## Why
TUI/Web UIのステータス表示にprocessingが残ると、現在何をしているか（apply/acceptance/archive/resolve）が判別できません。運用判断を行いやすくするため、表示語彙とフォーマットを明確化します。

## What Changes
- 表示語彙を `not queued, queued, applying, accepting, archiving, resolving, completed, archived, merged, merge wait, error` に統一する
- processing 表記を廃止し、実行フェーズごとの表示に置き換える
- 反復回数がある状態は `status:iteration` 形式で表示する（例: `applying:1`, `archiving:2`）
- Web UIの集計も新語彙に基づいて算出する

## Impact
- Affected specs: cli, web-monitoring
- Affected code: TUIの表示・イベント状態管理、Web UIの状態集計/表示
