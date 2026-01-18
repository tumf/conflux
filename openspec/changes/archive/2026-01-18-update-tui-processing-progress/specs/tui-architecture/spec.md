## MODIFIED Requirements
### Requirement: Event-Driven State Updates
TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。
Processing中の変更についても、tasks.md から取得した進捗が利用可能な場合は表示を更新しなければならない（SHALL）。
進捗取得に失敗した場合は、直前の表示を維持しなければならない（MUST）。

#### Scenario: Processing中に進捗が更新される
- **GIVEN** TUI が Processing 中の変更を表示している
- **AND** worktree の tasks.md から進捗が取得できる
- **WHEN** 自動リフレッシュが実行される
- **THEN** TUI は completed/total の表示を更新する

#### Scenario: Processing中に進捗取得が失敗した場合は保持する
- **GIVEN** TUI が Processing 中の変更を表示している
- **AND** tasks.md の読み取りに失敗する
- **WHEN** 自動リフレッシュが実行される
- **THEN** TUI は直前の completed/total 表示を維持する
