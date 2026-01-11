## ADDED Requirements

### Requirement: Archived 状態の checkbox 表示

TUI は archived 状態の change の checkbox をグレー色で表示しなければならない（SHALL）。

#### Scenario: 実行モードで archived 状態の change の checkbox がグレー表示

- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される
- **AND** checkbox のテキストは `[x]` のまま（内容は変わらない）

#### Scenario: 選択モードに戻った際も archived 状態は維持

- **GIVEN** 処理が完了し TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される
