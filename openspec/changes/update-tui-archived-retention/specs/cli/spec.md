## ADDED Requirements

### Requirement: Archived change の一覧保持

TUI は archived 状態になった change をアプリ終了まで Changes 一覧に残さなければならない（SHALL）。

#### Scenario: archived change が即時に一覧から消えない
- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` に更新された
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: 選択モードでも archived change を維持
- **GIVEN** TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: TUI 再起動後は archived change が一覧から消える
- **GIVEN** archived change が Changes 一覧に残っている
- **WHEN** TUI を終了して再起動する
- **THEN** archived change は Changes 一覧に表示されない
