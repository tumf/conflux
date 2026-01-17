## MODIFIED Requirements
### Requirement: Worktree View Status Labels
Worktreeビュー SHALL display worktree status labels in lowercase for merge-related states.

#### Scenario: merged ラベルを小文字で表示する
- **GIVEN** Worktreeビューが表示されている
- **AND** worktreeがbaseに対してaheadではない
- **WHEN** worktreeの状態ラベルが描画される
- **THEN** statusラベルは "merged" の小文字で表示される

#### Scenario: merging ラベルを小文字で表示する
- **GIVEN** Worktreeビューが表示されている
- **AND** worktreeがマージ進行中である
- **WHEN** worktreeの状態ラベルが描画される
- **THEN** statusラベルは "merging" の小文字で表示される
