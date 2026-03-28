## MODIFIED Requirements

### Requirement: Bulk Toggle Key Hint

Changes パネルは、全マーク/全アンマークのトグルが有効な場合に `x: toggle all` を表示しなければならない（SHALL）。

`x` の一括トグルは Select/Stopped に加え Running モードでも利用可能とし、Running モードでは active change 以外にトグル対象が存在する場合のみヒントを表示しなければならない（SHALL）。

Stopping/Error の間は当該ヒントを表示してはならない（SHALL NOT）。

#### Scenario: Select モードでヒントを表示する
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change exists
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL include "x: toggle all"

#### Scenario: Running モードでトグル対象があるときヒントを表示する
- **GIVEN** the TUI is in running mode
- **AND** at least one non-active eligible change exists
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL include "x: toggle all"

#### Scenario: Running モードでトグル対象がないときヒントを表示しない
- **GIVEN** the TUI is in running mode
- **AND** all changes are active or otherwise ineligible for bulk toggle
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL NOT include "x: toggle all"

#### Scenario: Stopping モードでヒントを表示しない
- **GIVEN** the TUI is in stopping mode
- **WHEN** the Changes panel is rendered
- **THEN** key hints SHALL NOT include "x: toggle all"
