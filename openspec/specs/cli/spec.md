# cli Specification

## Purpose
Defines CLI commands, subcommands, flags, and their behaviors for the cflx binary.
## Requirements
### Requirement: Subcommand Structure

CLI SHALL have a subcommand structure that supports future command extensions.

#### Scenario: Run without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Run with unknown subcommand

- **WHEN** user runs with a non-existent subcommand
- **THEN** an error message with available subcommands is displayed

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.

#### Scenario: Run with specific change

- **WHEN** user runs `cflx run --change <id>`
- **THEN** only the specified change is processed
- **AND** the snapshot log shows only the specified change

#### Scenario: Run with comma-separated changes

- **WHEN** user runs `cflx run --change a,b,c`
- **THEN** only changes `a`, `b`, `c` are processed
- **AND** the snapshot log shows only `a`, `b`, `c`

#### Scenario: Run with non-existent change

- **WHEN** user runs `cflx run --change nonexistent`
- **AND** no change named `nonexistent` exists
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** exits with "No changes found"

#### Scenario: Run with mixed valid and invalid changes

- **WHEN** user runs `cflx run --change a,nonexistent,c`
- **AND** `a` and `c` exist but `nonexistent` does not
- **THEN** a warning message "Specified change 'nonexistent' not found, skipping" is displayed
- **AND** only `a` and `c` are processed
- **AND** the snapshot log shows only `a`, `c`

#### Scenario: Successful run exits promptly

- **GIVEN** orchestration completes successfully and no restart was explicitly requested
- **WHEN** `cflx run` logs successful completion
- **THEN** the command exits promptly with status code 0
- **AND** it does not wait for an additional stop signal before terminating

### Requirement: Orchestration loop runs apply and archive
`run` サブコマンドは OpenSpec change workflow のオーケストレーションループを実行しなければならない（SHALL）。
オーケストレーターは apply 成功後に acceptance ループを実行し、archive 開始前に結果を判定しなければならない（SHALL）。
acceptance ループは change に対して `acceptance_command` を実行し、出力テキストから pass/fail/continue/blocked を判定して処理を分岐しなければならない（SHALL）。
- exit code はコマンド実行成否のみを示し、acceptance 判定には使用しない。
- acceptance prompt はハードコードされた acceptance prompt の後に設定値の `acceptance_prompt` を連結しなければならない（MUST）。
- acceptance verdict parsing は PASS/FAIL/CONTINUE/BLOCKED マーカーが非意味的な装飾（Markdown 強調など）を伴っていても認識しなければならない（MUST）。
- acceptance が FAIL の場合、apply ループへ戻る前に tasks.md を更新しなければならない（MUST）。
- tasks.md の更新は、acceptance の失敗回数に対応する `## Acceptance #<n> Failure Follow-up` セクションを末尾に追加するか、既存の関連タスクを未完了に戻す形で行わなければならない（MUST）。
- `Acceptance #<n> Failure Follow-up` の `<n>` は当該 acceptance 試行の 1 始まりの試行番号と一致しなければならない（MUST）。
- acceptance の失敗理由（findings）は tasks.md に記録しなければならない（MUST）。
- 失敗理由の記録は acceptance エージェントが tasks.md を直接編集して行い、オーケストレーターは acceptance 出力から findings を抽出して tasks.md に追記してはならない（MUST NOT）。
- acceptance prompt は FAIL 時に tasks.md の follow-up を更新する指示を含めなければならない（MUST）。
- `Acceptance #<n> Failure Follow-up` セクションでは、各 finding を `- [ ] <finding>` の未完了タスクとして 1 行ずつ記録しなければならない（MUST）。番号付きの箇条書きを使用してはならない（MUST NOT）。
- follow-up セクションに `Address acceptance findings:` のようなラッパー行やネストされた箇条書きを追加してはならない（MUST NOT）。
- acceptance の findings として扱う内容は stdout/stderr の tail 行を用い、`ACCEPTANCE:` マーカーと `FINDINGS:` 行を除外しなければならない（MUST）。
- findings の `- ` 箇条書き構造を解析して抽出してはならない（MUST NOT）。
- ログで件数を表示する場合は findings ではなく tail 行数である旨を明示しなければならない（MUST）。曖昧な "N findings" 表現を使用してはならない（MUST NOT）。
- apply ループは acceptance failure 後も同じ iteration カウンター値で再開しなければならない（MUST）。
- 出力が CONTINUE を示す場合、オーケストレーターは `acceptance_max_continues` 回まで acceptance を再試行しなければならない（MUST）。
- acceptance マーカーが存在しない場合、オーケストレーターは CONTINUE として扱い、`acceptance_max_continues` に従って再試行しなければならない（MUST）。
- CONTINUE の上限を超えた場合、オーケストレーターは FAIL として扱い apply ループへ戻らなければならない（MUST）。
- acceptance 失敗後に apply ループへ戻る際、acceptance ループの iteration カウンターを引き継がなければならない（MUST）。
- acceptance ループの iteration カウンターは試行ごとに増加し、acceptance failure 後に apply ループへ戻ってもリセットしてはならない（MUST NOT）。
- 2 回目以降の acceptance は、前回の acceptance 以降に更新されたファイル一覧と過去の findings に集中し、フルチェックを行ってはならない（MUST NOT）。
- 2 回目以降の acceptance prompt は、前回 acceptance 以降に更新されたファイル一覧（パスのみ）を含めなければならない（MUST）。
- 2 回目以降の acceptance prompt は、前回の acceptance findings を含め、解消確認を指示しなければならない（MUST）。
- 2 回目以降の acceptance prompt は、必要に応じて関連ファイルを読むよう指示し、diff 内容を含めてはならない（MUST NOT）。
- 2 回目以降の acceptance prompt は、前回の acceptance コマンド出力（stdout_tail/stderr_tail）を `<last_acceptance_output>` タグで囲んで含めなければならない（MUST）。
- acceptance コマンド出力は `AcceptanceHistory` に既に保存されているため、新規フィールド追加なしで参照可能でなければならない（MUST）。
- acceptance が BLOCKED の場合、オーケストレーターは当該 change の apply ループを停止し、再試行してはならない（MUST NOT）。
- acceptance が BLOCKED の場合、当該 change は停止状態として記録し、次の change 処理へ進まなければならない（MUST）。

#### Scenario: Acceptance retry narrows to updated files and prior findings
- **GIVEN** change が apply iteration を正常完了する
- **AND** acceptance output が CONTINUE を示す
- **WHEN** オーケストレーターが同じ change に対して次の acceptance を実行する
- **THEN** acceptance prompt は前回の acceptance 以降の更新ファイル一覧のみを含む（diff content なし）
- **AND** acceptance prompt は前回の acceptance findings を含み、解消確認を指示する
- **AND** acceptance prompt は必要に応じて関連ファイルを読むよう指示し、diff 内容を含めない

#### Scenario: Acceptance failure follow-up uses numbered section and flat tasks
- **GIVEN** acceptance output が FAIL で 2 件の findings を含む
- **WHEN** acceptance エージェントが指示に従って tasks.md を更新する
- **THEN** tasks.md の末尾に `## Acceptance #1 Failure Follow-up` が追加される
- **AND** セクション内に `- [ ] <finding>` の未完了タスクが 2 行追加される
- **AND** `Address acceptance findings` のようなラッパー行やネスト箇条書きは含まれない

#### Scenario: CONTINUE tail propagation to next acceptance prompt
- **GIVEN** acceptance output が CONTINUE を示す
- **AND** `AcceptanceHistory` に前回の acceptance 試行が記録されている
- **WHEN** オーケストレーターが acceptance ループを継続する
- **THEN** 次の acceptance プロンプトに `<last_acceptance_output>` タグで囲まれた前回の stdout_tail/stderr_tail が含まれる
- **AND** エージェントは前回の調査結果を参照して次の調査アクションを決定できる

#### Scenario: Acceptance failure logging avoids misleading findings count
- **GIVEN** acceptance output の tail に `ACCEPTANCE: FAIL` と `FINDINGS:` が含まれる
- **WHEN** オーケストレーターが acceptance FAIL を記録する
- **THEN** findings として保存される tail 行から `ACCEPTANCE:` マーカーと `FINDINGS:` 行が除外される
- **AND** ログは "N findings" のような誤解を招く件数表現を出さない

#### Scenario: Acceptance blocked stops apply loop
- **GIVEN** acceptance output が `ACCEPTANCE: BLOCKED` を示す
- **WHEN** オーケストレーターが acceptance 結果を処理する
- **THEN** 当該 change の apply ループは停止する
- **AND** 同一 change の apply は再試行されない

### Requirement: Default TUI Launch

When launched without a subcommand, the interactive TUI SHALL be displayed.

#### Scenario: Launch without subcommand

- **WHEN** user runs `cflx` without arguments
- **THEN** the interactive TUI is launched
- **AND** the change list is displayed in selection mode

#### Scenario: Launch with run subcommand (backward compatibility)

- **WHEN** user runs `cflx run`
- **THEN** the orchestration loop is executed directly as before

### Requirement: Change Selection Mode

At TUI launch, the change selection mode SHALL be displayed, allowing users to select changes for processing.

#### Scenario: Exit
- **WHEN** user presses `Ctrl+C`
- **THEN** TUI exits and the terminal is restored to its original state

### Requirement: Start Execution of Selected Changes

When F5 key is pressed in selection mode, processing of selected changes SHALL begin.

#### Scenario: Start execution with F5 key

- **WHEN** user presses F5 key
- **AND** one or more changes are selected
- **THEN** TUI switches to running mode
- **AND** selected changes are added to the queue

#### Scenario: F5 key with no selection

- **WHEN** user presses F5 key
- **AND** no changes are selected
- **THEN** execution does not start
- **AND** a warning message is displayed

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。
正常完了時は Ready 表示に戻り、停止要求がない限り Stopped へ遷移してはならない。

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** TUI は Select（Ready）モードに戻る
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Status line uses selected change progress
- **GIVEN** TUI が任意のモードである
- **AND** 1 件以上の change が選択されている（x）
- **WHEN** ステータスパネルが描画される
- **THEN** 進捗バーは選択された change の total/completed を合算して反映する
- **AND** ステータス行は進捗バーと経過時間のみを表示する

#### Scenario: Status line shows accumulated running time
- **GIVEN** TUI が一度でも Running モードになっている
- **WHEN** Ready または Stopped モードでステータスパネルが描画される
- **THEN** 経過時間は累積の Running 時間を表示する

#### Scenario: Header hides status in stopped and error modes
- **GIVEN** TUI が Stopped または Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない

### Requirement: TUI Layout Structure

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** ログパネルが有効な場合は停止メッセージを含むログパネルが表示される
- **AND** ログパネルが無効な場合でも停止メッセージはログに記録される

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Stopped" status in gray
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)

### Requirement: Auto-refresh Feature

TUI SHALL periodically auto-refresh the change list.

#### Scenario: Periodic auto-refresh

- **WHEN** TUI is displayed
- **THEN** the change list is refreshed every 5 seconds
- **AND** progress status is updated

#### Scenario: Display continues during refresh

- **WHEN** auto-refresh is in progress
- **THEN** TUI display is not interrupted
- **AND** changes are reflected after refresh completes

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

### Requirement: Error State Display

When an error occurs, TUI SHALL explicitly display the error state.

#### Scenario: Mode transition on error

- **WHEN** opencode execution fails with an error (LLM error, insufficient credits, etc.)
- **THEN** TUI mode transitions to "Error"
- **AND** header status displays "Error" in red

#### Scenario: Status panel error display

- **WHEN** TUI is in error state
- **THEN** status panel displays "Error in <change_id>"
- **AND** "Press F5 to retry" guidance is displayed

#### Scenario: Change display in error state

- **WHEN** TUI is in error state
- **THEN** the errored Change status shows "[error]" in red
- **AND** other queued Changes maintain their state

### Requirement: Error Retry with F5 Key

In error state, pressing F5 key SHALL retry processing of the failed Change.

#### Scenario: Retry with F5 key

- **WHEN** TUI is in error state
- **AND** user presses F5 key
- **THEN** the errored Change is added back to the queue
- **AND** TUI transitions to "Running" mode
- **AND** processing resumes

#### Scenario: Log display on retry

- **WHEN** user initiates retry with F5 key
- **THEN** log panel displays "Retrying: <change_id>"

#### Scenario: State after successful retry

- **WHEN** the retried processing succeeds
- **THEN** the Change status updates to "completed" or "archived"
- **AND** remaining queued Changes continue processing

### Requirement: init Subcommand

`init` subcommand SHALL generate a `.cflx.jsonc` configuration template file in the current directory.

#### Scenario: Generate default template (claude)

- **WHEN** user runs `cflx init`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: Generate opencode template

- **WHEN** user runs `cflx init --template opencode`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: Generate claude template explicitly

- **WHEN** user runs `cflx init --template claude`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: Generate codex template

- **WHEN** user runs `cflx init --template codex`
- **AND** no `.cflx.jsonc` exists in the current directory
- **THEN** a `.cflx.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: Config file already exists without force flag

- **WHEN** user runs `cflx init`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: Overwrite existing config with force flag

- **WHEN** user runs `cflx init --force`
- **AND** `.cflx.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: Invalid template name

- **WHEN** user runs `cflx init --template invalid`
- **THEN** the command exits with an error
- **AND** an error message lists valid template options (opencode, claude, codex)

### Requirement: Footer Dynamic Guidance Display

The selection mode footer SHALL display appropriate guidance messages based on application state.

#### Scenario: Guidance when no changes

- **WHEN** TUI is in selection mode
- **AND** the change list is empty
- **THEN** the footer displays "Add new proposals to get started"

#### Scenario: Guidance when no changes selected

- **WHEN** TUI is in selection mode
- **AND** one or more changes exist
- **AND** no changes are selected
- **THEN** the footer displays "Select changes with Space to process"

#### Scenario: Guidance when changes are selected

- **WHEN** TUI is in selection mode
- **AND** one or more changes are selected
- **THEN** the footer displays "Press F5 to start processing"

### Requirement: Running Footer Progress Bar Display

The running mode footer SHALL display a progress bar for overall processing progress.

#### Scenario: Completed task progress retention
- **WHEN** a change transitions to Completed, Archived, or Merged state
- **AND** the latest tasks.md is available at apply completion or archive start
- **THEN** the TUI preserves the last known task counts for that change
- **AND** the progress percentage does not decrease (monotonically increasing)

### Requirement: Processing Item Spinner Animation

TUI は実行中の表示を processing ではなくフェーズ別の語彙で示さなければならない（SHALL）。apply の実行中は `applying`、acceptance 実行中は `accepting`、archive 実行中は `archiving`、resolve 実行中は `resolving`、依存待ちの change は `blocked` を表示すること。反復回数がある場合は `status:iteration` 形式で表示すること。

#### Scenario: Applying 状態の表示
- **GIVEN** TUI が running mode で change を処理している
- **WHEN** apply が実行中である
- **THEN** change のステータス表示は `applying` となる

#### Scenario: Resolving の iteration 表示
- **GIVEN** change の queue_status が resolving である
- **AND** iteration_number が 2 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `resolving:2` となる

#### Scenario: 依存待ちの表示
- **GIVEN** change の queue_status が依存待ちである
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `blocked` となる

### Requirement: Completion Detection Retry Settings

Retry behavior SHALL be implemented for completion state detection.

#### Scenario: Default retry settings

- **WHEN** no retry settings exist in configuration file
- **THEN** maximum retry count is 3
- **AND** retry interval is 500 milliseconds

#### Scenario: Cancel during retry

- **WHEN** retry loop is in progress
- **AND** cancellation token is cancelled
- **THEN** retry loop terminates immediately
- **AND** process is cleaned up properly

### Requirement: TUI Unicode Display Width Support

The TUI SHALL correctly calculate and truncate text based on Unicode display width, not byte length or character count.

#### Scenario: Japanese text truncation in logs
- **WHEN** a log message contains Japanese characters (e.g., "設定ファイル初期化")
- **AND** the message exceeds the available display width
- **THEN** the message is truncated at a valid display width boundary
- **AND** ellipsis "..." is appended
- **AND** no panic occurs due to UTF-8 boundary issues

#### Scenario: Mixed ASCII and CJK text
- **WHEN** a log message contains both ASCII and CJK characters
- **THEN** ASCII characters count as 1 display column
- **AND** CJK characters count as 2 display columns
- **AND** truncation respects the total display width

#### Scenario: Emoji handling
- **WHEN** a log message contains emoji characters
- **THEN** emoji characters are counted with their proper display width
- **AND** truncation does not split emoji sequences

#### Scenario: ANSI control codes are stripped before display
- **WHEN** a log message contains ANSI control sequences (e.g., "\x1b[96mRead\x1b[0m")
- **THEN** the log panel displays the message without those control sequences (e.g., "Read")
- **AND** display width calculation and truncation operate on the stripped text

### Requirement: Native Task Progress Parsing

The system SHALL provide native change list discovery by directly reading the filesystem instead of relying on external commands.

#### Scenario: List all changes natively

```
Given openspec/changes directory exists
And it contains subdirectories ["change-a", "change-b"]
When list_changes_native() is called
Then it returns Vec<Change> with 2 entries
And each Change has id matching directory name
And each Change has task counts from tasks.md
```

#### Scenario: Handle missing tasks.md gracefully

```
Given openspec/changes/my-change directory exists
And tasks.md file does not exist in that directory
When list_changes_native() is called
Then the change is included with completed_tasks=0 and total_tasks=0
```

#### Scenario: Empty changes directory

```
Given openspec/changes directory exists but is empty
When list_changes_native() is called
Then it returns empty Vec<Change>
```

#### Scenario: Changes directory does not exist

```
Given openspec/changes directory does not exist
When list_changes_native() is called
Then it returns empty Vec<Change>
```

### Requirement: Task Progress Fallback Behavior

The system SHALL use native task parsing as primary source when openspec CLI returns zero task counts.

#### Scenario: CLI returns zero tasks
- **WHEN** openspec CLI returns `completedTasks: 0, totalTasks: 0` for a change
- **AND** a `tasks.md` file exists for that change
- **THEN** the system uses native parsing to determine actual task counts
- **AND** the TUI displays the native-parsed task counts

#### Scenario: CLI returns non-zero tasks
- **WHEN** openspec CLI returns non-zero task counts for a change
- **THEN** the system uses the CLI-provided task counts
- **AND** native parsing is not performed for that change

### Requirement: Version Display

The CLI SHALL support a `--version` flag to display the application version with UTC build number.

#### Scenario: Display version with --version flag
- **WHEN** user runs `cflx --version`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format
- **AND** the build number uses UTC time
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `cflx -V`
- **THEN** the application version is displayed in `v<semver>(YYYYMMDDHHmmss)` format

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version with UTC build number in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0(20260117113311)")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

### Requirement: Terminal Status Task Count Display

TUI は反復回数がある状態の表示を `status:iteration` 形式にしなければならない（SHALL）。apply/acceptance/archive/resolve の iteration 番号が更新された場合、Changes 一覧のステータス表示は最新の iteration に同期し続けなければならない（SHALL）。Applying中のChanges行では、ステータスは`[status:iteration]`のみを表示し、タスク進捗は`<completed>/<total>(<percent>%)`形式で表示しなければならない（SHALL）。

#### Scenario: Applying の iteration 表示
- **GIVEN** change が apply 実行中である
- **AND** apply の iteration 番号が 1 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `applying:1` となる

#### Scenario: Archiving の iteration 表示
- **GIVEN** change が archive 実行中である
- **AND** archive の iteration 番号が 2 である
- **WHEN** TUI が change 行を表示する
- **THEN** ステータス表示は `archiving:2` となる

#### Scenario: Applying の iteration 更新に追従する
- **GIVEN** change の queue_status が applying である
- **AND** iteration_number が 2 から 3 に更新される
- **WHEN** TUI が Changes 一覧を再描画する
- **THEN** ステータス表示は `applying:3` となる

#### Scenario: Applying の進捗表示フォーマット
- **GIVEN** change の queue_status が applying である
- **AND** iteration 番号が 1 である
- **AND** completed_tasks が 0 で total_tasks が 3 である
- **WHEN** TUI が change 行を表示する
- **THEN** Changes 行の進捗表示は `0/3(0%)` となる

### Requirement: TUI Archive Priority Processing

The TUI running mode SHALL archive all completed changes before starting the next apply operation.

#### Scenario: Archive before next apply
- **WHEN** TUI is in running mode
- **AND** one or more queued changes have reached 100% task completion
- **THEN** all complete changes are archived before any new apply command starts
- **AND** the archive process follows the same hooks (pre_archive, post_archive) as normal archiving

#### Scenario: Multiple complete changes
- **WHEN** TUI is in running mode
- **AND** multiple changes reach 100% completion simultaneously
- **THEN** all complete changes are archived in sequence
- **AND** processing continues only after all complete changes are archived

#### Scenario: Archive on loop iteration
- **WHEN** TUI orchestrator starts a new processing iteration
- **THEN** it first checks for any complete changes in the queue
- **AND** archives all complete changes before selecting the next change to apply

### Requirement: Remove Retry-Based Completion Check

The TUI SHALL NOT rely on retry loops to detect task completion for archiving purposes.

#### Scenario: Immediate archive attempt after apply success
- **WHEN** an apply command completes successfully
- **THEN** the orchestrator immediately returns to the main loop
- **AND** the main loop's archive phase handles completion detection
- **AND** no arbitrary retry delays are used for completion detection

#### Scenario: Completion detected on next iteration
- **WHEN** a change becomes 100% complete during another change's apply
- **THEN** the complete change is detected and archived on the next loop iteration
- **AND** no warning about "did not reach completion state" is logged

### Requirement: Reliable Archive Tracking

archive 検証は `openspec/changes/{change_id}` が存在する場合に未アーカイブとして扱わなければならない（SHALL）。

#### Scenario: changes が残っている場合は未アーカイブ扱い
- **WHEN** archive コマンドが成功する
- **AND** `openspec/changes/{change_id}` が存在している
- **THEN** archive 検証は未アーカイブとして扱われる
- **AND** archive コマンドは再実行される

### Requirement: TUI Uses Native Change Discovery

The TUI mode MUST use native directory scanning instead of external `openspec list` command for all change list operations.

#### Scenario: Initial change list uses native implementation

```
Given TUI mode is started
When initial changes are loaded
Then openspec/changes directory is read directly
And no external openspec command is executed for listing
```

#### Scenario: Auto-refresh uses native implementation

```
Given TUI is in running mode
When auto-refresh interval triggers
Then openspec/changes directory is read directly
And no external openspec command is executed for listing
```

#### Scenario: Archive phase uses native implementation

```
Given TUI orchestrator is processing changes
When checking for complete changes to archive
Then openspec/changes directory is read directly
And task progress is determined from tasks.md files
```

### Requirement: Log Panel Scroll Feature

The TUI log panel SHALL support scrolling to view older log entries.

#### Scenario: Page Down scroll in log panel
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user presses Page Down key
- **THEN** log view scrolls down by one page
- **AND** scroll position is limited to show the most recent entries at the bottom

#### Scenario: Page Up scroll in log panel
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user presses Page Up key
- **THEN** log view scrolls up by one page
- **AND** scroll position stops at the oldest log entry

#### Scenario: Scroll position indicator display
- **WHEN** log entries exceed visible area
- **THEN** the log panel title displays current scroll position (e.g., "Logs [1-10/50]")
- **AND** the indicator shows visible range and total count

#### Scenario: Auto-scroll on new log entry
- **WHEN** a new log entry is added
- **AND** user has not scrolled up manually (auto_scroll is true)
- **THEN** log view automatically scrolls to show the newest entry

#### Scenario: Disable auto-scroll when scrolling up
- **WHEN** user scrolls up in log panel (Page Up)
- **THEN** auto-scroll is disabled
- **AND** new log entries do not change scroll position
- **AND** user can review historical logs without interruption

#### Scenario: Re-enable auto-scroll at bottom
- **WHEN** user scrolls down to the bottom of logs
- **THEN** auto-scroll is re-enabled
- **AND** subsequent new entries will auto-scroll into view

#### Scenario: Home key jump to oldest log
- **WHEN** TUI is in running mode
- **AND** log entries exist
- **AND** user presses Home key
- **THEN** log view jumps to the oldest log entry
- **AND** auto-scroll is disabled

#### Scenario: End key jump to newest log
- **WHEN** TUI is in running mode
- **AND** log entries exist
- **AND** user presses End key
- **THEN** log view jumps to the newest log entry
- **AND** auto-scroll is re-enabled

#### Scenario: Mouse wheel scroll up
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user scrolls mouse wheel up
- **THEN** log view scrolls up by a few lines (e.g., 3 lines)
- **AND** auto-scroll is disabled

#### Scenario: Mouse wheel scroll down
- **WHEN** TUI is in running mode
- **AND** log entries exceed visible area
- **AND** user scrolls mouse wheel down
- **THEN** log view scrolls down by a few lines (e.g., 3 lines)
- **AND** if scroll position reaches the bottom, auto-scroll is re-enabled

#### Scenario: Analysis log header includes iteration
- **WHEN** the TUI receives analysis output logs with an iteration number
- **THEN** the log entry header is displayed as `[analysis:<iteration>]`
- **AND** the header is rendered even when no change_id is present

#### Scenario: Analysis log header without iteration
- **WHEN** the TUI receives analysis output logs without an iteration number
- **THEN** the log entry header is displayed as `[analysis]`

#### Scenario: Resolve log header includes change id and iteration
- **WHEN** the TUI receives resolve output logs with change_id and iteration
- **THEN** the log entry header is displayed as `[{change_id}:resolve:<iteration>]`

#### Scenario: Resolve log header without iteration
- **WHEN** the TUI receives resolve output logs with change_id but no iteration
- **THEN** the log entry header is displayed as `[{change_id}:resolve]`

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

### Requirement: Log Entry Limit

The TUI SHALL maintain a maximum limit on stored log entries to prevent unbounded memory growth.

#### Scenario: Log entry limit enforcement
- **WHEN** a new log entry is added
- **AND** the total log count exceeds 1000 entries
- **THEN** the oldest log entry is removed
- **AND** scroll offset is adjusted if necessary to prevent display issues

### Requirement: Log Entry Headers

TUI は analysis と resolve の operation ログに対して構造化ヘッダを表示し、追跡性を向上させなければならない (SHALL)。

#### Scenario: Analysis ログヘッダ形式
- **WHEN** analysis operation がログメッセージを出力する
- **THEN** ログエントリは `[analysis:N]` のヘッダで表示される
- **AND** N は analysis operation の iteration number を表す

#### Scenario: Resolve ログヘッダ形式
- **WHEN** resolve operation がログメッセージを出力する
- **THEN** ログエントリは `[resolve:N]` のヘッダで表示される
- **AND** N は resolve operation の iteration number を表す
- **AND** ヘッダには change_id が表示されない

#### Scenario: ログヘッダのカラーリングは一貫している
- **WHEN** ヘッダ付きログエントリが表示される
- **THEN** change_id が利用可能な場合、ヘッダは change_id hash に基づいた色分けで表示される
- **AND** 視認性のためヘッダは太字で表示される

### Requirement: TUI Status Transition on Apply Completion

The TUI SHALL transition change status from `Processing` to `Completed` when an apply operation succeeds and all tasks are complete.

#### Scenario: Apply succeeds with 100% task completion

- **GIVEN** a change is being processed in running mode
- **AND** the change has `Processing` status
- **WHEN** the apply command completes successfully
- **AND** all tasks for the change are marked complete (100%)
- **THEN** the change status transitions to `Completed`
- **AND** the status display shows `[completed]` instead of spinner
- **AND** a log entry "Completed: <change-id>" is added

#### Scenario: Apply succeeds with incomplete tasks

- **GIVEN** a change is being processed in running mode
- **AND** the change has `Processing` status
- **WHEN** the apply command completes successfully
- **AND** some tasks remain incomplete (< 100%)
- **THEN** the change status remains `Processing`
- **AND** the orchestrator continues to next apply iteration

#### Scenario: 100% complete change displays correctly before archive

- **GIVEN** a change has completed all tasks (100%)
- **AND** the change has `Completed` status
- **WHEN** the TUI renders the change list
- **THEN** the status shows `[completed]` (not `Processing...` with 100%)
- **AND** the progress column shows the task count (e.g., `29/29`)

### Requirement: Archive Phase Does Not Reset Status

The TUI archive phase SHALL NOT send redundant status transition events for changes that are already in `Completed` status.

#### Scenario: Archive already-completed change

- **GIVEN** a change has `Completed` status
- **WHEN** the archive phase processes the change
- **THEN** no `ProcessingStarted` event is sent
- **AND** no additional `ProcessingCompleted` event is sent
- **AND** only `ChangeArchived` event is sent upon successful archive

#### Scenario: Archive pre-complete change from queue

- **GIVEN** a change was 100% complete before being queued
- **AND** the change has `Queued` status (not yet processed)
- **WHEN** the archive phase identifies the change as complete
- **THEN** `ProcessingStarted` event is sent (status → Processing)
- **AND** `ProcessingCompleted` event is sent (status → Completed)
- **AND** archive command is executed
- **AND** `ChangeArchived` event is sent (status → Archived)

### Requirement: Apply Context History
オーケストレーターは、逐次/並列のどちらの apply でも共通ループで同一の履歴注入ロジックを使用し、各 apply 試行の最終サマリーメッセージを記録して同一 change の次回 apply プロンプトに含めなければならない（MUST）。さらに、acceptance が FAIL で apply ループへ戻る場合、次の apply 試行のプロンプトに直前の acceptance コマンド出力の stdout_tail/stderr_tail を `<last_acceptance_output>` ブロックで含めなければならない（MUST）。stdout_tail が空の場合は stderr_tail を使用し、両方空の場合はブロックを含めなくてもよい（MAY）。同一 acceptance 試行に由来する tail は最初の apply 試行にのみ注入し、以降の apply 試行では再注入してはならない（MUST NOT）。

#### Scenario: parallel の2回目 apply に履歴が含まれる
- **GIVEN** parallel mode で change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

#### Scenario: serial の2回目 apply に履歴が含まれる
- **GIVEN** 逐次モードで change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

#### Scenario: acceptance failure tail が次の apply に含まれる
- **GIVEN** acceptance が FAIL で終了して apply ループへ戻る
- **AND** AcceptanceHistory に stdout_tail または stderr_tail が記録されている
- **WHEN** 次の apply 試行が開始される
- **THEN** apply プロンプトは `<last_acceptance_output>` ブロックを含む
- **AND** stdout_tail が存在する場合は stdout_tail が含まれる
- **AND** stdout_tail が空の場合は stderr_tail が含まれる

#### Scenario: acceptance tail は 1 回だけ注入される
- **GIVEN** acceptance が FAIL で終了して apply ループへ戻る
- **AND** AcceptanceHistory に stdout_tail または stderr_tail が記録されている
- **WHEN** 連続して 2 回の apply 試行が実行される
- **THEN** 1 回目の apply プロンプトにのみ `<last_acceptance_output>` ブロックが含まれる

### Requirement: Apply History Context Format

apply 履歴コンテキストは、archive と resolve の履歴フォーマットと一貫性を持たなければならない（SHALL）。

#### Scenario: 履歴フォーマットの一貫性

- **GIVEN** システムが apply、archive、resolve の履歴を管理する
- **WHEN** 各履歴がフォーマットされる
- **THEN** すべての履歴で XML 風のタグ形式が使用される
- **AND** すべての履歴で `attempt`、`status`、`duration` フィールドが含まれる
- **AND** 各操作固有の追加情報（`error`、`verification_result`、`continuation_reason`）も含まれる

### Requirement: TUI Stop Processing with Escape Key
TUIはEsc二度押しによる強制停止時、現在のエージェントプロセスとその子プロセスを確実に終了しなければならない（SHALL）。

#### Scenario: 強制停止で子プロセスが残らない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** ログに「Force stopped - process terminated」が表示される
- **AND** 変更の状態はQueuedに戻る

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, queue_status SHALL be reset to NotQueued while preserving execution marks ([x]). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining queue_status as NotQueued. When resuming with F5, execution-marked changes SHALL be restored to queued and processing SHALL resume. Task progress updates in Stopped mode SHALL NOT trigger queuing.

#### Scenario: Stopped mode display
- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** execution-marked changes show "[x]" while their queue_status remains not queued

#### Scenario: Queue management in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on an execution-marked change
- **THEN** the execution mark is removed and queue_status remains not queued

#### Scenario: Queue addition in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-marked change
- **THEN** the execution mark is added and queue_status remains not queued

#### Scenario: Task completion in Stopped mode does not auto-queue
- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain not queued
- **AND** the change SHALL NOT be automatically added to the queue

#### Scenario: Resume processing from Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes after converting execution-marked changes to queued
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning
- **WHEN** TUI is in Stopped mode
- **AND** no changes are execution-marked
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Stopping mode help text
- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text includes "F5: continue"
- **AND** help text shows "Waiting for current process..."

### Requirement: Interrupted Change Handling
Changes interrupted by stop SHALL be handled according to the policy of holding queued status only during execution. When force-stopped, queue_status SHALL be reset to NotQueued while preserving execution marks. On resume, execution-marked changes SHALL be restored to queued and can be re-processed. Accepting status SHALL be treated as an in-flight execution state and MUST be reset to NotQueued when the user force-stops.

#### Scenario: Force-stopped accepting change returns to not queued
- **GIVEN** a change is in Accepting status
- **WHEN** the user force stops with second Esc press
- **THEN** the change status becomes not queued
- **AND** the execution mark remains set

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using git worktrees. Parallel mode is OFF by default.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `cflx run --parallel`
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode disabled by default
- **WHEN** user runs `cflx run` without `--parallel` flag
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: Parallel mode requires git directory
- **WHEN** user runs `cflx run --parallel`
- **AND** no `.git` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates git repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `cflx run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available

### Requirement: Parallel Mode TUI Display

The TUI SHALL display parallel execution progress when in parallel mode.

#### Scenario: Display parallel groups
- **WHEN** TUI is in running mode with parallel execution
- **THEN** changes are grouped by their parallel group assignment
- **AND** each group is visually distinguished

#### Scenario: Display workspace status
- **WHEN** changes are being processed in parallel
- **THEN** each change shows its workspace status (creating, running, completed, failed)
- **AND** multiple spinners can be active simultaneously

#### Scenario: Display merge progress
- **WHEN** a parallel group completes and merge begins
- **THEN** a merge progress indicator is displayed
- **AND** the merge result (success/conflict) is shown

### Requirement: Parallel Mode Dry Run

The CLI SHALL support `--dry-run` to preview parallelization groups without execution.

#### Scenario: Preview parallelization groups
- **WHEN** user runs `cflx run --parallel --dry-run`
- **THEN** the analyzer determines parallelization groups
- **AND** the groups are displayed without executing any changes
- **AND** no workspaces are created

### Requirement: VCS Backend Selection Flag

CLI SHALL allow explicit VCS backend selection via `--vcs` flag.

#### Scenario: Default auto detection

- **WHEN** `--parallel` flag is specified
- **AND** `--vcs` flag is not specified
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Explicit git selection

- **WHEN** `cflx run --parallel --vcs git` is executed
- **THEN** Git backend is used
- **AND** an error is displayed if Git is not available

#### Scenario: Explicit auto selection

- **WHEN** `cflx run --parallel --vcs auto` is executed
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Invalid VCS value

- **WHEN** `cflx run --parallel --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, git" is displayed
- **AND** exit code is non-zero

#### Scenario: --vcs without --parallel

- **WHEN** `cflx run --vcs git` is executed
- **AND** `--parallel` flag is not specified
- **THEN** `--vcs` option is ignored
- **AND** normal sequential execution proceeds

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

### Requirement: Archived 状態の checkbox 表示

TUI は archived 状態の change の checkbox をグレー色で表示しなければならない（SHALL）。

#### Scenario: 実行モードで archived 状態の change の checkbox がグレー表示

- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される
- **AND** checkbox のテキストは `[x]` のまま（内容は変わらない）

#### Scenario: 選択モードに戻った際も archived 状態は維持

- **GIVEN** 処理が完了し TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 部分は `Color::DarkGray` で表示される

### Requirement: Web Monitoring Flags

The CLI SHALL support flags to enable and configure web-based monitoring.

#### Scenario: Enable web monitoring
- **WHEN** user runs with `--web` flag
- **THEN** HTTP server starts for web monitoring
- **AND** server binds to default port 8080 on 127.0.0.1

#### Scenario: Configure web port
- **WHEN** user runs with `--web --web-port 3000`
- **THEN** HTTP server starts on port 3000 instead of default

#### Scenario: Configure bind address
- **WHEN** user runs with `--web --web-bind 0.0.0.0`
- **THEN** HTTP server accepts connections from any network interface
- **AND** warning is logged about exposing server to network

#### Scenario: Web flags without --web
- **WHEN** user runs with `--web-port 3000` but without `--web` flag
- **THEN** HTTP server does not start
- **AND** web-port flag is ignored

#### Scenario: Invalid port number
- **WHEN** user runs with `--web --web-port 99999`
- **THEN** error message is displayed about invalid port range
- **AND** orchestrator exits with non-zero status

#### Scenario: Web monitoring in TUI mode
- **WHEN** user runs TUI mode with `--web` flag
- **THEN** HTTP server starts in background
- **AND** TUI displays message indicating web server is running
- **AND** TUI shows web server URL (e.g., "Web monitoring: http://127.0.0.1:8080")

#### Scenario: Web monitoring in run mode
- **WHEN** user runs `cflx run --web`
- **THEN** HTTP server starts before orchestration begins
- **AND** server URL is logged to console
- **AND** orchestration proceeds normally

### Requirement: Archived change の一覧保持

TUI は archived 状態になった change をアプリ終了まで Changes 一覧に残さなければならない（SHALL）。

#### Scenario: archived change が即時に一覧から消えない
- **GIVEN** TUI が実行モードである
- **AND** ある change の `queue_status` が `Archived` に更新された
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: 選択モードでも archived change を維持
- **GIVEN** TUI が選択モードに戻った
- **AND** ある change の `queue_status` が `Archived` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change は Changes 一覧に表示されたままである

#### Scenario: TUI 再起動後は archived change が一覧から消える
- **GIVEN** archived change が Changes 一覧に残っている
- **WHEN** TUI を終了して再起動する
- **THEN** archived change は Changes 一覧に表示されない

### Requirement: Git Repository Detection

The CLI SHALL detect whether the current directory is a git-managed repository by checking for the `.git` directory.

#### Scenario: git repository detected
- **WHEN** a `.git` directory exists in the current working directory
- **THEN** git worktree parallel features are available

#### Scenario: git repository not detected
- **WHEN** no `.git` directory exists in the current working directory
- **AND** user runs `cflx run --parallel`
- **THEN** the command exits with a non-zero exit code
- **AND** an error message is displayed: "Error: --parallel requires a git repository (.git directory not found)"

### Requirement: TUIのChange一覧にworktree存在を表示する
TUIのChange一覧は、各changeに紐づくworktreeの有無を識別できるインジケータを表示しなければならない（SHALL）。

#### Scenario: worktreeが存在するchangeの表示
- **GIVEN** 対象changeにworktreeが存在する
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータが表示される

#### Scenario: worktreeが存在しないchangeの表示
- **GIVEN** 対象changeにworktreeが存在しない
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータは表示されない

### Requirement: 実行中changeのworktree削除を禁止する
TUIはProcessing/Running中のchangeに対してworktree削除を許可してはならない（MUST NOT）。
ただし、削除対象worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく場合は、実行中であっても削除を許可しなければならない（MUST）。

#### Scenario: 実行中の未関連worktreeを削除できる
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押して削除を確認する
- **THEN** worktree削除が実行される
- **AND** 削除後にworktree一覧が更新される

#### Scenario: 実行中のqueued/processing系worktreeは削除できない
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがQueued/Processing/Archiving/Resolving/Accepting/MergeWaitのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除は行われず、禁止メッセージが表示される

### Requirement: Serial Apply Iteration WIP Commits

逐次（非parallel）applyループでは、各イテレーション終了後に作業内容をWIPコミットとして保存しなければならない（MUST）。apply成功・失敗や進捗増加の有無に関わらず、最新状態をスナップショットとして残さなければならない（MUST）。

WIPコミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。Gitリポジトリで実行中の場合、`git add -A` と `git commit --no-verify --allow-empty` 相当の操作で新規WIPコミットを作成しなければならない（MUST）。既存WIPコミットの `--amend` を使用してはならない（MUST NOT）。

#### Scenario: WIP created after successful apply iteration
- Given: 逐次applyループが実行中である
- When: applyコマンドが正常に完了しイテレーションが終了する
- Then: WIPスナップショットが新規コミットとして作成される

#### Scenario: WIP created after failed apply iteration
- Given: 逐次applyループが実行中である
- When: applyコマンドが失敗してイテレーションが終了する
- Then: 失敗時点の作業内容がWIPスナップショットとして保存される

#### Scenario: WIP created when no progress is made
- Given: 逐次applyループが実行中である
- When: applyコマンドは成功したがタスク進捗が増加しない
- Then: 最新の作業内容を反映したWIPスナップショットが作成される

### Requirement: Archive Context History

オーケストレータは、逐次/並列のどちらの archive でも共通ループで同一の履歴注入ロジックを使用し、各 archive 試行の結果をキャプチャして同じ change に対する後続の archive プロンプトに含めなければならない（MUST）。

#### Scenario: 初回 archive 試行には履歴がない

- **WHEN** オーケストレータが change に対して初めて archive を実行する
- **THEN** プロンプトには設定からの基本 archive_prompt のみが含まれる
- **AND** `<last_archive>` タグは含まれない

#### Scenario: 2回目の archive には前回の試行結果が含まれる

- **GIVEN** change に対する archive の1回目の試行が検証失敗した
- **WHEN** オーケストレータが同じ change に対して2回目の archive を実行する
- **THEN** プロンプトには基本 archive_prompt が含まれる
- **AND** プロンプトには `<last_archive attempt="1">` ブロックが含まれる
- **AND** ブロックには試行回数、成功/失敗ステータス、所要時間、検証結果が含まれる

#### Scenario: 複数の前回試行が含まれる

- **GIVEN** change に対する archive が2回失敗している
- **WHEN** オーケストレータが同じ change に対して3回目の archive を実行する
- **THEN** プロンプトには `<last_archive attempt="1">` と `<last_archive attempt="2">` の両方のブロックが含まれる
- **AND** 各ブロックにはそれぞれの試行の詳細が含まれる

#### Scenario: 履歴は change 完了時にクリアされる

- **GIVEN** change に対する archive 履歴が存在する
- **WHEN** archive が成功し、change が完全に処理される
- **THEN** その change の archive 履歴はクリアされる
- **AND** 次に同じ change ID が処理される場合、履歴は空の状態から始まる

#### Scenario: parallel の2回目 archive に履歴が含まれる
- **GIVEN** parallel mode で change が archive 実行中である
- **AND** 1回目の archive が検証失敗している
- **WHEN** 2回目の archive が実行される
- **THEN** プロンプトは base archive_prompt を含む
- **AND** プロンプトは `<last_archive attempt="1">` ブロックを含む
- **AND** ブロックには 1回目の試行結果が含まれる

### Requirement: Archive History Context Format

archive 履歴コンテキストは、XML 風のタグ形式で構造化されなければならない（SHALL）。

各試行ブロックは以下の情報を含む：
- `attempt`: 試行回数（1-based）
- `status`: success または failed
- `duration`: 所要時間（秒単位）
- `verification_result`: 検証結果（検証失敗時の理由）
- `error`: エラーメッセージ（失敗時）
- `exit_code`: 終了コード

#### Scenario: 検証失敗時の履歴フォーマット

- **GIVEN** archive コマンドは成功したが検証が失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `verification_result` には「Change still exists at openspec/changes/{change_id}」などの具体的な理由が含まれる
- **AND** `exit_code: 0` が含まれる（コマンド自体は成功したため）

#### Scenario: コマンド失敗時の履歴フォーマット

- **GIVEN** archive コマンドが失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `error` には終了コードに関する情報が含まれる
- **AND** `exit_code` には非ゼロの値が含まれる

### Requirement: Resolve Continuation Context

resolve コマンドの再試行時、システムは前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

#### Scenario: 初回 resolve 試行にはコンテキストがない

- **WHEN** システムが conflict resolution のために resolve を初めて実行する
- **THEN** プロンプトには基本的な VCS 状態とコンフリクト情報のみが含まれる
- **AND** `<resolve_context>` ブロックは含まれない

#### Scenario: 2回目の resolve には前回の結果と継続理由が含まれる

- **GIVEN** resolve の1回目の試行後もコンフリクトが残っている
- **WHEN** システムが2回目の resolve を実行する
- **THEN** プロンプトには `<resolve_context>` ブロックが含まれる
- **AND** ブロックには現在の試行番号（"attempt 2 of 3"）が含まれる
- **AND** 前回の試行の結果（コマンド終了ステータス、検証結果）が含まれる
- **AND** 検証失敗の具体的な理由（"Conflicts still present: src/main.rs"）が含まれる
- **AND** 所要時間が含まれる

#### Scenario: マージ未完了による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし `MERGE_HEAD` が存在する（マージ未完了）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Merge still in progress (MERGE_HEAD exists)」という理由が含まれる

#### Scenario: マージコミット不足による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし必要なマージコミット（"Merge change: {change_id}"）が不足している
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Missing merge commits for change_ids」という理由が含まれる
- **AND** 不足している change_id のリストが含まれる

#### Scenario: Worktree マージ未完了による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし worktree でマージが未完了（worktree に `MERGE_HEAD` が存在）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Worktree merge still in progress for '{revision}'」という理由が含まれる

#### Scenario: Pre-sync コミットサブジェクト不正による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし pre-sync マージコミットのサブジェクトが期待と異なる
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Invalid pre-sync merge commit subject」という理由が含まれる
- **AND** 期待されるサブジェクトと実際のサブジェクトが含まれる

### Requirement: Resolve Context Format

resolve コンテキストは、人間とAIが読みやすい形式で構造化されなければならない（SHALL）。

コンテキストブロックには以下が含まれる：
- 現在の試行番号と最大試行回数
- 前回の試行の詳細（コマンド終了ステータス、検証結果、所要時間）
- 検証失敗の具体的な理由（継続理由）
- ループ継続の説明

#### Scenario: コンテキストの可読性

- **WHEN** resolve コンテキストがフォーマットされる
- **THEN** 「This is attempt X of Y」という形式で試行回数が示される
- **AND** 「Previous attempt (N):」というラベルで前回の結果が示される
- **AND** 「Continue resolving...」などの指示が含まれる

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

### Requirement: Deprecated Flags Removed

The CLI SHALL NOT accept --opencode-path or --openspec-cmd flags.

#### Scenario: --opencode-path flag rejected

- **WHEN** user runs `cflx --opencode-path /path/to/opencode`
- **THEN** CLI exits with error
- **AND** error message indicates flag is not recognized

#### Scenario: --openspec-cmd flag rejected

- **WHEN** user runs `cflx --openspec-cmd custom-cmd`
- **THEN** CLI exits with error
- **AND** error message indicates flag is not recognized

#### Scenario: OPENSPEC_CMD environment variable not used

- **WHEN** OPENSPEC_CMD environment variable is set
- **THEN** CLI does not read or use this environment variable
- **AND** configuration file settings are used instead

### Requirement: changes間のspec delta衝突検出コマンド
CLI SHALL provide a subcommand to detect conflicts between spec delta files across changes without using an LLM.

#### Scenario: 衝突なしの場合の成功
- **WHEN** user runs the new conflict detection command
- **AND** no conflicting spec deltas are found
- **THEN** the command exits with status code 0

#### Scenario: 衝突が検出された場合
- **WHEN** user runs the new conflict detection command
- **AND** conflicting spec deltas are found
- **THEN** the command outputs conflict details
- **AND** the command exits with a non-zero status code

#### Scenario: JSON出力の指定
- **WHEN** user runs the new conflict detection command with a JSON output flag
- **THEN** the command outputs a machine-readable JSON payload

### Requirement: Web Execution Control Availability
Web UIからの実行制御は、`--web` でHTTPサーバーが起動している場合にのみ有効でなければならない（SHALL）。TUIとRunモードのいずれでも同じ制御経路を提供しなければならない（MUST）。RunモードではTUIと同等のリトライ/停止挙動を提供しなければならない（SHALL）。

#### Scenario: TUIモードでのWeb制御
- **GIVEN** `cflx tui --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** TUIの実行状態が同等に変化する

#### Scenario: RunモードでのWeb制御
- **GIVEN** `cflx run --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** オーケストレーターの実行状態が同等に変化する

#### Scenario: Runモードでのリトライ制御
- **GIVEN** `cflx run --web` で実行中にエラーが発生している
- **WHEN** Web UI が制御APIへ retry 要求を送る
- **THEN** オーケストレーターは同一のエラー change を再実行する

### Requirement: 選択中worktreeの削除操作を提供する
TUIは選択中worktreeを削除する操作を提供し、削除前に確認を行わなければならない（SHALL）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** TUIがWorktreesビューである
- **AND** 選択中worktreeが削除可能である（main ではなく、処理中のchangeに紐づかない）
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除確認ダイアログが表示される

#### Scenario: 確認後にworktreeを削除する
- **GIVEN** 削除確認ダイアログで同意する
- **WHEN** 削除処理が実行される
- **THEN** 対象worktreeが削除され、Worktrees一覧から消える

#### Scenario: worktree一覧が空の場合の削除操作
- **GIVEN** TUIがWorktreesビューである
- **AND** worktree一覧が空である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 何も起こらない
| `[resolve pending]` | resolve実行開始待ち（操作ロックされる） |

### Requirement: Changes一覧ログプレビューの相対時間表記
TUIのChanges一覧に表示されるログプレビューは、相対時間を括弧で囲んだ形式で表示しなければならない（SHALL）。

#### Scenario: 相対時間を括弧で囲む
- **GIVEN** Changes一覧にログプレビューが表示される
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューの相対時間は括弧付き形式（例: `(2m ago)`）で表示される

### Requirement: カーソル行のログプレビュー視認性
TUIのChanges一覧でカーソル行が選択されている場合、ログプレビューの文字色は非選択行より明るく表示しなければならない（SHALL）。

#### Scenario: カーソル行でログプレビューが判読できる
- **GIVEN** Changes一覧のカーソル行が選択されている
- **AND** 該当行にログプレビューが表示されている
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューは選択背景上でも判読できる明るい文字色で表示される

### Requirement: Running Changes一覧の経過時間配置
TUIのRunningモードにおけるChanges一覧は、in-flight状態（Applying/Accepting/Archiving/Resolving）の行で、動作中スピナーの直後に経過時間を表示しなければならない（SHALL）。経過時間はステータス表示の前に配置しなければならない（SHALL）。

#### Scenario: in-flight行でスピナー直後に経過時間を表示する
- **GIVEN** TUIがRunningモードである
- **AND** changeのqueue_statusがApplyingである
- **AND** changeの開始時刻が取得できる
- **WHEN** TUIがChanges一覧を描画する
- **THEN** change行の表示はスピナーの直後に経過時間を含む
- **AND** 経過時間はステータス表示の前に配置される

### Requirement: Log Panel Visibility Toggle
TUI は Changes ビューで `l` キーによりログパネルの表示/非表示を切り替えられるようにしなければならない（SHALL）。
ログパネルの既定状態は表示（有効）でなければならない（SHALL）。

#### Scenario: Toggle off hides log panel while keeping logs
- **GIVEN** ログパネルが有効である
- **AND** ログが存在する
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログパネルは非表示になる
- **AND** 新しいログは引き続きログバッファに追加される

#### Scenario: Toggle on restores log panel
- **GIVEN** ログパネルが無効である
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログが存在する場合、ログパネルが表示される

### Requirement: TUIログファイルの常時出力

TUI のログファイル出力は常時有効でなければならず（MUST）、`tui --logs` オプションは提供してはならない（MUST NOT）。

#### Scenario: `tui --logs` は無効
- **WHEN** ユーザーが `cflx tui --logs /tmp/debug.log` を実行する
- **THEN** CLI は不明なオプションとしてエラーを表示する
- **AND** 終了コードは非0である

### Requirement: server サブコマンド
CLI は `cflx server` サブコマンドを提供し、サーバモードを起動しなければならない（SHALL）。

#### Scenario: server サブコマンドで起動する
- **WHEN** ユーザーが `cflx server` を実行する
- **THEN** サーバモードが起動する
- **AND** カレントディレクトリの変更一覧は読み込まない

### Requirement: リモートサーバ指定フラグ
CLI は `--server <endpoint>` を受け付け、TUI をリモートサーバ接続モードで起動しなければならない（SHALL）。

#### Scenario: リモートエンドポイントで TUI を起動する
- **WHEN** ユーザーが `cflx --server http://127.0.0.1:9876` を実行する
- **THEN** TUI はローカルの change 一覧を読まずにリモート状態を表示する

### Requirement: リモートサーバ認証トークン
CLI は bearer token を指定するための `--server-token` または `--server-token-env` を受け付けなければならない（SHALL）。

#### Scenario: bearer token を付与して接続する
- **GIVEN** `--server-token` が指定されている
- **WHEN** TUI がリモートサーバへ接続する
- **THEN** Authorization header に bearer token が付与される

### Requirement: server データディレクトリの CLI 上書き
CLI は `cflx server --data-dir <PATH>` を受け付け、サーバの `data_dir` を指定パスに上書きしなければならない（SHALL）。
`--data-dir` が未指定の場合、サーバはグローバル設定の `server.data_dir` または既定値を使用しなければならない（SHALL）。

#### Scenario: `--data-dir` を指定して起動する
- **WHEN** ユーザーが `cflx server --data-dir /var/lib/cflx` を実行する
- **THEN** サーバは `data_dir=/var/lib/cflx` を使用する

#### Scenario: `--data-dir` 未指定で起動する
- **GIVEN** グローバル設定に `server.data_dir=/tmp/cflx-server` がある
- **WHEN** ユーザーが `cflx server` を実行する
- **THEN** サーバは `data_dir=/tmp/cflx-server` を使用する

### Requirement: server の resolve_command フラグは受け付けない
CLI は `cflx server --resolve-command` を受け付けてはならない（MUST NOT）。

#### Scenario: `--resolve-command` は不明なオプションとして扱われる
- **WHEN** ユーザーが `cflx server --resolve-command "true"` を実行する
- **THEN** CLI は不明なオプションとしてエラーを表示する
- **AND** 終了コードは非0である

### Requirement: Project サブコマンドによるサーバプロジェクト管理
CLI は `cflx project` 配下でサーバのプロジェクト管理 API を操作できなければならない（SHALL）。
`add` は `remote_url` と `branch` を送信し、`remove` は `project_id` を削除し、`status` はプロジェクト一覧または指定 ID の情報を取得し、`sync` は git/sync を実行しなければならない（SHALL）。

#### Scenario: プロジェクト追加を実行する
- **WHEN** ユーザーが `cflx project add <remote_url> <branch>` を実行する
- **THEN** CLI は `POST /api/v1/projects` を呼び出す
- **AND** サーバが返す `project_id` を表示する

#### Scenario: プロジェクト一覧を取得する
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `GET /api/v1/projects` を呼び出す
- **AND** 取得したプロジェクト一覧を表示する

#### Scenario: プロジェクトの同期を実行する
- **WHEN** ユーザーが `cflx project sync <project_id>` を実行する
- **THEN** CLI は `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **AND** サーバの同期結果を表示する

### Requirement: Project サブコマンドの接続先解決と認証非対応
`cflx project` は `--server` 未指定時にグローバル設定の `server.bind` と `server.port` を用いて接続先を決定しなければならない（MUST）。
今回の `cflx project` はサーバクライアント認証を扱わず、認証が必要な設定が指定された場合は実行前にエラーで停止しなければならない（MUST）。

#### Scenario: --server 未指定時はグローバル設定から接続先を解決する
- **GIVEN** グローバル設定に `server.bind` と `server.port` がある
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `http://<bind>:<port>` を接続先として使用する

#### Scenario: 認証指定がある場合はエラーで停止する
- **GIVEN** `--server-token` または `--server-token-env` が指定されている
- **WHEN** ユーザーが `cflx project` サブコマンドを実行する
- **THEN** CLI は認証非対応である旨を示すエラーを表示し、リクエストを送信しない

### Requirement: service Subcommand Manages Background Server

The CLI SHALL provide a `service` subcommand group for managing `cflx server` as a background service.

Supported operations MUST include: `install`, `uninstall`, `status`, `start`, `stop`, `restart`.

#### Scenario: Service commands are discoverable
- **WHEN** a user runs `cflx service --help`
- **THEN** help text lists `install`, `uninstall`, `status`, `start`, `stop`, `restart`

#### Scenario: Service command rejects unknown operation
- **WHEN** a user runs `cflx service unknown`
- **THEN** the CLI reports an unknown subcommand error

### Requirement: project sync --all による全件同期
CLI は `cflx project sync --all` をサポートし、登録済みプロジェクトをすべて `git/sync` で同期しなければならない（SHALL）。
同期は各プロジェクトの結果を個別に出力し、1 件でも失敗があれば非 0 で終了しなければならない（MUST）。

#### Scenario: 全プロジェクトを同期する
- **GIVEN** サーバに 2 件以上のプロジェクトが登録されている
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は `GET /api/v1/projects` で一覧を取得する
- **AND** 各プロジェクトに対して `POST /api/v1/projects/{id}/git/sync` を順に呼び出す
- **AND** 各プロジェクトの同期結果が表示される

#### Scenario: 失敗が含まれる場合は非 0 で終了する
- **GIVEN** 同期対象のうち 1 件が失敗する
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は他のプロジェクトの同期を継続する
- **AND** 最終的な終了コードは非 0 になる

#### Scenario: 対象が 0 件の場合はスキップする
- **GIVEN** サーバに登録済みプロジェクトが存在しない
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は同期対象が無い旨を表示する
- **AND** 同期リクエストは送信されない

### Requirement: project add のブランチ表記を URL から解決する
CLI は `cflx project add` において、`https://github.com/<org>/<repo>/tree/<branch>` と `https://github.com/<org>/<repo>#<branch>` のブランチ表記を受け入れなければならない（SHALL）。
URL にブランチ表記が含まれる場合、CLI は `remote_url` をリポジトリのベース URL に正規化し、ブランチは抽出した値として扱わなければならない（MUST）。

#### Scenario: /tree/<branch> の URL を受け入れる
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo/tree/develop` を実行する
- **THEN** CLI は `remote_url=https://github.com/org/repo` と `branch=develop` を使用する

#### Scenario: #<branch> の URL を受け入れる
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo#develop` を実行する
- **THEN** CLI は `remote_url=https://github.com/org/repo` と `branch=develop` を使用する

### Requirement: project add のデフォルトブランチ解決
`cflx project add` でブランチが明示されない場合、CLI はリモートのデフォルトブランチを解決して使用しなければならない（MUST）。
明示的なブランチ引数が指定されている場合、URL 内のブランチ表記よりも引数の値を優先しなければならない（MUST）。

#### Scenario: ブランチ省略時にデフォルトブランチを使用する
- **GIVEN** リモートのデフォルトブランチが `main` である
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo` を実行する
- **THEN** CLI は `branch=main` を使用する

#### Scenario: 引数指定が URL のブランチ表記を上書きする
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo/tree/develop main` を実行する
- **THEN** CLI は `branch=main` を使用する

### Requirement: install-skills Subcommand

The CLI SHALL provide an `install-skills` subcommand for installing bundled or local agent skills into the standard `.agents/skills` locations.

#### Scenario: Install bundled skills in project scope

- **WHEN** the user runs `cflx install-skills self`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install local skills in project scope

- **WHEN** the user runs `cflx install-skills local:../my-skills`
- **THEN** the CLI starts an install flow using the local source path `../my-skills`
- **AND** installed skills are written under `./.agents/skills`
- **AND** the lock file is written to `./.agents/.skill-lock.json`

#### Scenario: Install bundled skills in global scope

- **WHEN** the user runs `cflx install-skills self --global`
- **THEN** the CLI starts an install flow using bundled skills sourced from the repository's top-level `skills/` layout
- **AND** installed skills are written under `~/.agents/skills`
- **AND** the lock file is written to `~/.agents/.skill-lock.json`

#### Scenario: Reject unsupported source schemes

- **WHEN** the user runs `cflx install-skills git:https://example.com/repo`
- **THEN** the command exits with an error
- **AND** the error message states that only `self` and `local:<path>` are supported

### Requirement: run Surfaces Hook Output

The `run` subcommand SHALL display hook execution details in the same user-visible CLI log stream used for other run progress messages.

#### Scenario: Hook command is logged before hook output

- **GIVEN** a hook is configured for a lifecycle stage reached during `cflx run`
- **WHEN** the hook starts
- **THEN** the CLI log first shows the hook type and expanded command string
- **AND** any captured hook output is displayed after the command log entry

#### Scenario: Hook output ordering includes failure result

- **GIVEN** a hook produces captured output and then fails during `cflx run`
- **WHEN** the run subcommand reports the failure
- **THEN** the CLI log shows hook command information first
- **AND** the CLI log shows captured hook output next
- **AND** the CLI log shows the hook failure result after the captured output

#### Scenario: CLI run preserves hook visibility parity with non-interactive execution

- **GIVEN** the same hook configuration is used in serial CLI run and another existing execution path that already emits hook logs
- **WHEN** the hook executes in serial CLI run
- **THEN** users can see the hook command and any captured output without needing debug-only tracing configuration
