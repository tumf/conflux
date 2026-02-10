## MODIFIED Requirements

### Requirement: New Change Detection

When auto-refresh detects new changes, they SHALL be displayed appropriately.

#### Scenario: New change detection
- **WHEN** auto-refresh detects a new change
- **THEN** the new change is added to the change list
- **AND** a "NEW" badge is displayed
- **AND** "Discovered new change: <id>" is logged

#### Scenario: Default state of new changes
- **WHEN** a new change is detected
- **THEN** it is unselected by default (`[ ]`)
- **AND** the new count in the footer is updated

#### Scenario: NEW badge display
- **WHEN** a change is newly detected
- **THEN** a "NEW" badge is displayed next to the change name
- **AND** the badge is displayed in a visually prominent color

#### Scenario: NEW badge cleared on selection
- **WHEN** user toggles selection on a change with NEW badge in Select mode
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

#### Scenario: NEW badge cleared on queue addition
- **WHEN** user adds a change with NEW badge to the queue (Running/Stopped mode)
- **THEN** the NEW badge is removed
- **AND** the new count in the footer is decremented

### Requirement: Dynamic Execution Queue
Running 中に queued change を外した場合、当該 change がまだ Processing を開始していないなら、オーケストレータはその change を実行対象から除外しなければならない（MUST）。
Applying/Accepting/Archiving/Resolving の change は `Space` による単体停止要求のみ許可し、`@` は状態変更を行わない（MUST NOT）。

#### Scenario: Running 中に queued change を外す
- **WHEN** TUI が Running モードである
- **AND** ユーザーが queued change を Space キーで NotQueued に切り替える
- **AND** その change が Processing を開始していない
- **THEN** その change は実行対象から除外される
- **AND** 以降の実行でその change は処理されない

#### Scenario: Running 中に実行中 change を単体停止する
- **GIVEN** TUI が Running モードである
- **AND** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが Space キーを押す
- **THEN** 当該 change の停止要求が発行される
- **AND** 停止完了後に当該 change は `not queued` に戻り、実行マークが解除される
- **AND** 他の queued change は継続して処理される

#### Scenario: Processing 中の change で @ は無効
- **GIVEN** change の queue_status が Applying/Accepting/Archiving/Resolving のいずれかである
- **WHEN** ユーザーが `@` キーを押す
- **THEN** queue_status と選択状態は変更されない

### Requirement: approve Subcommand

The CLI SHALL NOT provide an `approve` subcommand.

#### Scenario: Approve subcommand is rejected
- **WHEN** user runs `cflx approve set {change_id}`
- **THEN** CLI reports an unknown subcommand error
- **AND** exit code is non-zero

### Requirement: TUI Approval Toggle

The TUI SHALL ignore approval toggles and SHALL NOT change any state on `@` key presses.

#### Scenario: @ key does nothing
- **WHEN** user presses `@` key in any TUI mode
- **THEN** selection and queue status are unchanged
- **AND** no approval state is created or stored

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL start with all changes unselected and SHALL NOT auto-queue any change.

#### Scenario: TUI startup clears execution marks
- **WHEN** user starts the TUI
- **THEN** all changes are unselected by default
- **AND** no changes are automatically queued

### Requirement: Unapproved Changes Cannot Be Queued

The system SHALL allow changes to be queued regardless of approval state.

#### Scenario: TUI can queue any change
- **WHEN** TUI is in selection mode
- **AND** user presses Space to select a change
- **THEN** the change is queued without approval checks

#### Scenario: CLI run includes specified change
- **WHEN** user runs `cflx run --change {change_id}`
- **THEN** the change is added to the queue
- **AND** no approval warning is displayed

### Requirement: Git Uncommitted Changes Error Message

Git backend で未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。
未追跡ファイルの判定では `.gitignore` と `.git/info/exclude` の除外を適用しなければならない（MUST）。

#### Scenario: Error message format
- **WHEN** parallel execution is attempted with Git backend
- **AND** uncommitted changes exist
- **THEN** the error message includes:
  - Problem description
  - Resolution method (commit or stash)
  - Specific command examples

#### Scenario: Untracked files also trigger error
- **WHEN** parallel execution is attempted with Git backend
- **AND** only untracked files exist
- **THEN** the same error message is displayed
- **AND** files in `.gitignore` と `.git/info/exclude` は除外される

### Requirement: Enhanced Help Output

The CLI SHALL provide comprehensive help output that includes all subcommands, key options, and usage examples.

#### Scenario: Main help shows all subcommands
- **WHEN** user runs `cflx --help`
- **THEN** help output includes list of all subcommands: run, tui, init
- **AND** help output includes key options: --parallel, --max-concurrent, --dry-run, --vcs, --web, --web-port, --web-bind

#### Scenario: Run subcommand help shows detailed options
- **WHEN** user runs `cflx run --help`
- **THEN** help output includes detailed description of run subcommand
- **AND** help output includes examples of parallel execution
- **AND** help output includes examples of web monitoring

#### Scenario: TUI subcommand help shows keybindings
- **WHEN** user runs `cflx tui --help`
- **THEN** help output includes TUI key bindings (Space, F5, Esc, Tab, q)
- **AND** help output includes description of TUI features
- **AND** help output includes web monitoring options
