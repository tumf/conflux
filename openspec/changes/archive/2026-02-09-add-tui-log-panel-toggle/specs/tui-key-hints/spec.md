## ADDED Requirements
### Requirement: Log Panel Toggle Hint
Changes ビューのChangesパネルはログパネルの切り替え操作として `l: logs` を表示しなければならない（SHALL）。

#### Scenario: Select mode shows log toggle hint
- **GIVEN** TUI is in select mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"

#### Scenario: Running mode shows log toggle hint
- **GIVEN** TUI is in running mode
- **WHEN** Changes panel is rendered
- **THEN** key hints include "l: logs"
