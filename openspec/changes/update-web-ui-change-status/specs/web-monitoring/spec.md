## MODIFIED Requirements
### Requirement: Dashboard UI - Change List Display
Webダッシュボードは、TUIのQueueStatusと完全に一致するステータス語彙と集計ルールでchange一覧を表示しなければならない（SHALL）。

#### Scenario: QueueStatusに一致するステータス表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI は TUI の QueueStatus 表記（not queued, queued, processing, completed, accepting, archiving, archived, merged, merge wait, resolving, error）で表示する
- **AND** pending/in_progress/complete の表記は表示しない

#### Scenario: QueueStatus基準の集計表示
- **GIVEN** Web UI が全体進捗と統計を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI の集計は QueueStatus 基準で計算される
- **AND** legacy status（pending/in_progress/complete）は集計に使用しない

#### Scenario: Acceptingの表示
- **GIVEN** change がQueueStatus::Acceptingである
- **WHEN** Web UI が change 行を表示する
- **THEN** ステータスは "accepting" と表示される
- **AND** 専用のバッジ色とアイコンが適用される
