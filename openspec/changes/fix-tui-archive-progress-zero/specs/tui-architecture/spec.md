## MODIFIED Requirements
### Requirement: すべての状態でtasks.mdの進捗を反映する
TUIは、archive/resolving中であってもtasks.mdから取得できる進捗を表示し続けなければならない（MUST）。tasks.mdの読み取りが失敗し0/0になる場合、直前の進捗を上書きしてはならない（MUST NOT）。
アーカイブ途中でworktree上のtasks.mdがアーカイブ先へ移動した場合でも、アーカイブ先のtasks.mdから進捗を再取得しなければならない（MUST）。

#### Scenario: アーカイブ移動直後でも進捗を保持する
- **GIVEN** 変更がArchiving状態であり、worktree上でtasks.mdがアーカイブ先へ移動されている
- **AND** 直前のprogressが0/0ではない
- **WHEN** TUIが進捗を再取得する
- **THEN** アーカイブ先tasks.mdから進捗を取得し、表示を0/0にしない
