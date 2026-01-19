## ADDED Requirements
### Requirement: worktreeのtasks進捗読み取りは取得失敗で上書きしない
worktreeのtasks.mdから進捗を取得できない場合、archive/resolving中の進捗を0/0で上書きしてはならない（MUST NOT）。取得できる場合のみ進捗を更新しなければならない（MUST）。

#### Scenario: worktreeのtasks取得失敗時は進捗を維持する
- **GIVEN** worktreeが存在し、変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** worktree/archived fallbackのtasks.md読み取りが失敗する
- **THEN** 進捗は直前の値を維持する
