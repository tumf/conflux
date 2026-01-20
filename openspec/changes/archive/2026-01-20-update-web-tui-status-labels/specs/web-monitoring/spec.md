## MODIFIED Requirements

### Requirement: Dashboard UI - Change List Display

Webダッシュボードは、TUIの表示語彙と一致するステータス語彙でchange一覧を表示しなければならない（SHALL）。processing 表記は使用せず、`not queued, queued, applying, accepting, archiving, resolving, completed, archived, merged, merge wait, error` を使用すること。

#### Scenario: QueueStatusに一致するステータス表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI は `not queued, queued, applying, accepting, archiving, resolving, completed, archived, merged, merge wait, error` の語彙で表示する
- **AND** processing の表記は表示しない

#### Scenario: QueueStatus基準の集計表示
- **GIVEN** Web UI が全体進捗と統計を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI の集計は QueueStatus 基準で計算される
- **AND** applying/accepting/archiving/resolving は進行中として集計される

### Requirement: Dashboard UI - Task Status Visualization

反復回数がある状態は `status:iteration` 形式で表示しなければならない（SHALL）。

#### Scenario: Applying の iteration 表示
- **GIVEN** change の queue_status が applying である
- **AND** iteration_number が 1 である
- **WHEN** Web UI が change 行を表示する
- **THEN** ステータス表示は `applying:1` となる
