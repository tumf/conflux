## MODIFIED Requirements
### Requirement: すべての状態でtasks進捗を保持する
Web state_updateは、tasks.mdの読み取りに失敗した場合にcompleted_tasks/total_tasksを0/0で上書きしてはならない（MUST NOT）。archive/resolving中でも直前の進捗が維持されなければならない（MUST）。

#### Scenario: Archive/Resolving中にprogress取得が失敗する
- **GIVEN** 変更がArchivingまたはResolving状態である
- **AND** 直前のprogressが0/0ではない
- **WHEN** state_updateの生成時にtasks.mdの読み取りが失敗し0/0となる
- **THEN** completed_tasks/total_tasksは直前の値を維持する
