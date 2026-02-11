## MODIFIED Requirements

### Requirement: Approval State Transition in Select Mode

The TUI SHALL ignore `@` key presses and SHALL NOT change any selection or queue state.

#### Scenario: @ key does nothing in select mode
- **GIVEN** the TUI is in select mode
- **WHEN** the user presses `@`
- **THEN** the change state remains unchanged
- **AND** no approval-related log message is shown

### Requirement: Approval State Transition in Running Mode

The TUI SHALL ignore `@` key presses and SHALL NOT change any selection or queue state.

#### Scenario: @ key does nothing in running mode
- **GIVEN** the TUI is in running mode
- **WHEN** the user presses `@`
- **THEN** the change state remains unchanged

### Requirement: 未コミット change の操作ヒントを非表示にする

並列モードで未コミットの change が選択中の場合、Changes パネルのキーヒントは選択に関する操作を表示してはならない（SHALL）。

#### Scenario: 未コミット change は選択ヒントを表示しない
- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソルが未コミットの change にある
- **WHEN** Changes パネルを描画する
- **THEN** "Space: queue" のキーヒントは表示されない
