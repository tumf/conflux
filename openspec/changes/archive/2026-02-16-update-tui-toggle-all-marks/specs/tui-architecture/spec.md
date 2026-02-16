## ADDED Requirements

### Requirement: Bulk Execution Mark Toggle

Changes ビューは、実行マーク可能な change を対象に、全マーク/全アンマークを1操作で切り替えられなければならない（SHALL）。

この操作は Select/Stopped モードでのみ有効で、Running/Stopping/Error では無効でなければならない（SHALL）。

トグル対象に未マークが1件でも存在する場合は対象を全てマークし、対象が全てマーク済みの場合は全てアンマークしなければならない（SHALL）。

#### Scenario: 未マークが残っている場合は全マークする
- **GIVEN** the TUI is in select mode
- **AND** at least one eligible change is not marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be marked

#### Scenario: すべてマーク済みの場合は全アンマークする
- **GIVEN** the TUI is in stopped mode
- **AND** all eligible changes are marked
- **WHEN** the user triggers the bulk toggle
- **THEN** all eligible changes SHALL be unmarked
