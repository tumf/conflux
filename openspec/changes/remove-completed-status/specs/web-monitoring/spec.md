## MODIFIED Requirements

### Requirement: Dashboard UI - Change List Display

Webダッシュボードは、TUIのQueueStatusと完全に一致するステータス語彙と集計ルールでchange一覧を表示しなければならない（SHALL）。

#### Scenario: QueueStatusに一致するステータス表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI は TUI の QueueStatus 表記（not queued, queued, processing, accepting, archiving, archived, merged, merge wait, resolving, error）で表示する
- **AND** `completed` は表示しない
- **AND** archiving 遷移が発生するため completed の中間表示は存在しない

#### Scenario: QueueStatus基準の集計表示
- **GIVEN** Web UI が全体進捗と統計を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI の集計は QueueStatus 基準で計算される
- **AND** `completed` は集計対象から除外される
- **AND** completed を中間状態として数えるケースは存在しない
- **AND** いかなる場合も completed を集計値として表示しない
- **AND** completed の単語を UI 上の統計ラベルとして表示しない
- **AND** completed を UI 上で観測できる状態は存在しない
- **AND** completed が state_update に現れないことを前提に表示を行う

#### Scenario: Acceptingの表示
- **GIVEN** change がQueueStatus::Acceptingである
- **WHEN** Web UI が change 行を表示する
- **THEN** ステータスは "accepting" と表示される
- **AND** 専用のバッジ色とアイコンが適用される
