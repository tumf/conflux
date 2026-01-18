## ADDED Requirements
### Requirement: Worktree Status Label for Unknown Commits Ahead

Worktreeビュー SHALL display a warning or unknown status label when commits-ahead detection fails.

Unknown status MUST NOT be displayed as "merged" or as no-commits-ahead.

#### Scenario: Unknown commits-ahead status during refresh
- **GIVEN** Worktreeビューが表示されている
- **AND** 5秒ごとの更新で対象worktreeのcommits-ahead検出が失敗する
- **WHEN** worktreeリストが描画される
- **THEN** 状態ラベルは警告またはunknownを示す
- **AND** 状態ラベルは"merged"または進んでいない表示にならない
