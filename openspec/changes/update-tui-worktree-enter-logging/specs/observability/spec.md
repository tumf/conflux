## MODIFIED Requirements
### Requirement: TUIログの無視理由の明示
TUIで入力が無視される場合、原因がユーザーに判別できるようwarningログを残さなければならない（SHALL）。

#### Scenario: WorktreesビューでEnterが無視された理由をログに残す
- **GIVEN** TUIがWorktreesビューを表示している
- **WHEN** Enterキー操作が条件不足により無視される
- **THEN** warningログは無視理由を含むメッセージを出力する
- **AND** メッセージはユーザーが条件を判断できる内容である
