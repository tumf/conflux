## ADDED Requirements

### Requirement: Bulk Toggle Key Hint

Changes パネルは、全マーク/全アンマークのトグルが有効な場合に `x: toggle all` を表示しなければならない（SHALL）。

Running/Stopping/Error の間は当該ヒントを表示してはならない（SHALL NOT）。

#### Scenario: Select モードでヒントを表示する
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change exists
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL include "x: toggle all"

#### Scenario: Running モードでヒントを表示しない
- **GIVEN** the TUI is in running mode
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL NOT include "x: toggle all"
