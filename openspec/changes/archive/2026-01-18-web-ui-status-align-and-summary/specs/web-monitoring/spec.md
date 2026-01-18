## MODIFIED Requirements
### Requirement: Dashboard UI - Change List Display
Webダッシュボードは、TUIと同一のQueueStatus表記と最上位の全体進捗を含む一覧を表示しなければならない（SHALL）。

#### TUI Status Mapping Table
Web UI must align with the TUI QueueStatus enum display values:

| TUI QueueStatus | Web Display String | Description |
|-----------------|-------------------|-------------|
| NotQueued | "not queued" | Not in execution queue |
| Queued | "queued" | Waiting in execution queue |
| Processing | "processing" | Currently being processed |
| Completed | "completed" | Processing completed, waiting for archive |
| Archiving | "archiving" | Currently being archived |
| Archived | "archived" | Archived after completion |
| Merged | "merged" | Merged to main branch (parallel mode) |
| MergeWait | "merge wait" | Waiting for merge resolution |
| Resolving | "resolving" | Currently resolving a merge |
| Error(String) | "error" | Error occurred during processing |

**Deprecated Status Values**: The legacy status values ("pending", "in_progress", "complete") shall not be displayed in the Web UI.

#### Scenario: TUIと一致するステータス表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI は TUI の QueueStatus 表記（not queued, queued, processing, completed, archiving, archived, merged, merge wait, resolving, error）で表示する
- **AND** pending/in_progress/complete の表記は表示しない

#### Scenario: 全体進捗を最上位に表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** 進捗が再計算される
- **THEN** 全体進捗のサマリーは画面最上位（ヘッダー直下）で表示される
- **AND** 進捗バーと完了タスク数が視認できる
- **AND** 全体進捗セクションには、全変更の総タスク数と完了タスク数を集計した進捗バーが含まれる
- **AND** 進捗パーセンテージが数値で表示される

#### Scenario: change 行の情報をスリム化する
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change がレンダリングされる
- **THEN** change 行は ID、QueueStatus、進捗、イテレーション番号のみを主要情報として表示する
- **AND** 追加情報は折りたたみ領域にまとめる

#### Scenario: イテレーション番号を表示する
- **GIVEN** change に対して apply/archive のループが実行されている
- **WHEN** Web UI が change 行を表示する
- **THEN** change 行に最新のイテレーション番号が表示される
- **AND** イテレーション番号は "Iteration: N" の形式で表示される（N は現在のイテレーション回数）
- **AND** イテレーション番号が 0 または未設定の場合は表示しない

#### Scenario: 操作ボタンを折りたたみ表示する
- **GIVEN** Web UI に SPC/Approve 操作が存在する
- **WHEN** change 行が通常表示される
- **THEN** 操作ボタンはデフォルトで非表示になる
- **AND** ユーザー操作で展開した場合のみ表示される
