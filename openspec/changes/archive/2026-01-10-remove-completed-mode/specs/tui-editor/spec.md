# tui-editor Specification Delta

## ADDED Requirements

### Requirement: ログベースのレイアウト表示

ログパネルの表示は、モードではなくログの存在に基づいて決定されなければならない（SHALL）。

#### Scenario: ログが存在する場合のレイアウト

- **GIVEN** TUIが選択モードである
- **AND** ログエントリが存在する
- **WHEN** 画面が描画される
- **THEN** ログパネルが表示される

#### Scenario: ログが存在しない場合のレイアウト

- **GIVEN** TUIが選択モードである
- **AND** ログエントリが空である
- **WHEN** 画面が描画される
- **THEN** ログパネルは表示されない

**Rationale**: Log display is now based on log existence rather than mode.
