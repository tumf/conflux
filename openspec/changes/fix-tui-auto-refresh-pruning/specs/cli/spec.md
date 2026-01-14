## MODIFIED Requirements

### Requirement: Auto-refresh Feature

TUI は Changes リストを定期的に自動更新しなければならない（SHALL）。

#### Scenario: 5秒ごとの自動更新
- **WHEN** TUI が表示されている
- **THEN** Changes リストは 5 秒ごとに更新される
- **AND** 進捗（タスク完了数など）が反映される
- **AND** fetched 一覧に存在しない change は Changes リストから除外される
- **BUT** 現在の TUI セッション中に apply を開始した change は、fetched 一覧に存在しなくても保持される

#### Scenario: 更新中も表示が継続する
- **WHEN** 自動更新が進行中である
- **THEN** TUI の表示は中断されない
- **AND** 更新完了後に変更が反映される
