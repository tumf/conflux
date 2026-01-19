## MODIFIED Requirements
### Requirement: Workspace Preservation on Error

並列実行においてエラーまたはユーザーによる強制停止が発生した場合、workspaceを削除せずに保持しなければならない（MUST）。

#### Scenario: Workspace preserved on force stop
- **GIVEN** 並列実行が進行中である
- **AND** ユーザーがTUIで`Esc Esc`の強制停止を行う
- **WHEN** 並列実行がキャンセル扱いで早期終了する
- **THEN** worktreeは削除されず保持される
- **AND** 再開に利用できる状態が維持される
