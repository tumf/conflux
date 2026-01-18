## MODIFIED Requirements
### Requirement: Event-Driven State Updates
TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。
すべての状態で tasks.md から取得できる進捗を反映し続けなければならない（MUST）。
進捗取得に失敗した場合でも completed を 0 に上書きしてはならない（MUST NOT）。取得失敗は 0 件完了とは別の状態として扱う。
TUI は worktree を優先し、ベースツリーへフォールバックしながら進捗を取得しなければならない（MUST）。

#### Scenario: 任意の状態で進捗が更新される
- **GIVEN** TUI が任意の状態の変更を表示している
- **AND** tasks.md から進捗が取得できる
- **WHEN** 自動リフレッシュや進捗イベントが実行される
- **THEN** TUI は completed/total の表示を更新する

#### Scenario: 任意の状態で進捗取得が失敗した場合は保持する
- **GIVEN** TUI が任意の状態の変更を表示している
- **AND** tasks.md の読み取りに失敗する
- **WHEN** 自動リフレッシュや進捗イベントが実行される
- **THEN** TUI は直前の completed/total 表示を維持する
- **AND** 取得失敗を 0 件完了として扱わない

#### Scenario: archiving/resolving/mergedでworktree進捗を優先する
- **GIVEN** TUI が archiving/resolving/merged/archived 状態の変更を表示している
- **AND** worktree側の tasks.md に最新進捗が存在する
- **WHEN** 進捗更新イベントまたは自動リフレッシュが実行される
- **THEN** TUI は worktree側の completed/total を表示する
