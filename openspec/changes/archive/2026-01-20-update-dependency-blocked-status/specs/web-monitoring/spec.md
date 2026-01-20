## MODIFIED Requirements

### Requirement: Dashboard UI - Change List Display

Webダッシュボードは、TUIの表示語彙と一致するステータス語彙でchange一覧を表示しなければならない（SHALL）。processing/completed 表記は使用せず、`not queued, queued, applying, accepting, archiving, resolving, blocked, completed, archived, merged, merge wait, error` を使用すること。反復回数がある場合は `status:iteration` 形式で表示すること。

#### Scenario: QueueStatusに一致するステータス表示
- **GIVEN** Web UI が change 一覧を表示している
- **WHEN** change の queue_status が更新される
- **THEN** Web UI は `not queued, queued, applying, accepting, archiving, resolving, blocked, completed, archived, merged, merge wait, error` の語彙で表示する
- **AND** processing/completed の表記は表示しない

#### Scenario: Applying の iteration 表示
- **GIVEN** change の queue_status が applying である
- **AND** iteration_number が 1 である
- **WHEN** Web UI が change 行を表示する
- **THEN** ステータス表示は `applying:1` となる
