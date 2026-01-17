## MODIFIED Requirements
### Requirement: Merge Key Hint Display Conditions
TUI Worktree View SHALL display merge-related status labels using lowercase strings.

#### Scenario: merging ラベルは小文字で表示する
- **GIVEN** TUI is in Worktrees view
- **AND** マージ開始イベントを受信している
- **WHEN** マージ状態ラベルが表示される
- **THEN** ラベルは "merging" の小文字で表示される

#### Scenario: merged ラベルは小文字で表示する
- **GIVEN** TUI is in Worktrees view
- **AND** worktreeのブランチがbaseに対してaheadではない
- **WHEN** マージ状態ラベルが表示される
- **THEN** ラベルは "merged" の小文字で表示される
