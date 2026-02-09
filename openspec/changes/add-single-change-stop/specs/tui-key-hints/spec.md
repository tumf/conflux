## ADDED Requirements
### Requirement: Active Change Stop Hint
Changes パネルは、カーソルが active change にある場合、`Space: stop` を表示しなければならない（SHALL）。

#### Scenario: Running mode shows Space: stop for active change
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the Changes panel is rendered
- **THEN** key hints include "Space: stop"
