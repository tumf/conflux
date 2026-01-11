# cli Specification

## Purpose
TBD - created by archiving change add-run-subcommand. Update Purpose after archive.
## Requirements
### Requirement: サブコマンド構造

CLI はサブコマンド構造を持ち、将来的なコマンド拡張に対応できなければならない（SHALL）。

#### Scenario: サブコマンドなしで実行
- **WHEN** ユーザーが引数なしで `openspec-orchestrator` を実行する
- **THEN** 利用可能なサブコマンド一覧を含むヘルプメッセージを表示する

#### Scenario: 不明なサブコマンドで実行
- **WHEN** ユーザーが存在しないサブコマンドで実行する
- **THEN** エラーメッセージと利用可能なサブコマンド一覧を表示する

### Requirement: run サブコマンド

`run` サブコマンドは OpenSpec 変更ワークフローのオーケストレーションループを実行しなければならない（SHALL）。

#### Scenario: 特定の変更を指定して実行
- **WHEN** ユーザーが `openspec-orchestrator run --change <id>` を実行する
- **THEN** 指定された変更のみを処理する
- **AND** スナップショットログには指定された変更のみが表示される

#### Scenario: 複数の変更をカンマ区切りで指定
- **WHEN** ユーザーが `openspec-orchestrator run --change a,b,c` を実行する
- **THEN** `a`, `b`, `c` の変更のみを処理する
- **AND** スナップショットログには `a`, `b`, `c` のみが表示される

#### Scenario: 存在しない変更を指定した場合
- **WHEN** ユーザーが `openspec-orchestrator run --change nonexistent` を実行する
- **AND** `nonexistent` という変更が存在しない
- **THEN** 警告メッセージ "Specified change 'nonexistent' not found, skipping" が出力される
- **AND** 「No changes found」と表示されて終了する

#### Scenario: 有効な変更と無効な変更を混在して指定
- **WHEN** ユーザーが `openspec-orchestrator run --change a,nonexistent,c` を実行する
- **AND** `a` と `c` は存在するが `nonexistent` は存在しない
- **THEN** 警告メッセージ "Specified change 'nonexistent' not found, skipping" が出力される
- **AND** `a` と `c` のみを処理する
- **AND** スナップショットログには `a` と `c` のみが表示される

### Requirement: デフォルトTUI起動

サブコマンドなしで起動した場合、インタラクティブTUIを表示しなければならない（SHALL）。

#### Scenario: サブコマンドなしでの起動
- **WHEN** ユーザーが `openspec-orchestrator` を引数なしで実行する
- **THEN** インタラクティブTUIが起動する
- **AND** 選択モードで変更一覧が表示される

#### Scenario: runサブコマンドでの起動（後方互換性）
- **WHEN** ユーザーが `openspec-orchestrator run` を実行する
- **THEN** 従来通りオーケストレーションループが直接実行される

### Requirement: 変更選択モード

TUI起動時、変更選択モードを表示し、ユーザーが処理する変更を選択できなければならない（SHALL）。

#### Scenario: 終了
- **WHEN** ユーザーが `q` キーまたは `Ctrl+C` を押す
- **THEN** TUIが終了し、ターミナルが元の状態に復元される

### Requirement: 選択変更の実行開始

選択モードでF5キーを押すと、選択された変更の処理を開始しなければならない（SHALL）。

#### Scenario: F5キーで実行開始
- **WHEN** ユーザーがF5キーを押す
- **AND** 1つ以上の変更が選択されている
- **THEN** TUIが実行モードに切り替わる
- **AND** 選択された変更がキューに追加される

#### Scenario: 選択なしでF5キー
- **WHEN** ユーザーがF5キーを押す
- **AND** 変更が1つも選択されていない
- **THEN** 実行は開始されない
- **AND** 警告メッセージが表示される

### Requirement: 実行モードダッシュボード

TUIは実行モードでダッシュボード形式のUIを表示しなければならない（SHALL）。

#### Scenario: 処理完了時の表示

- **WHEN** 全てのキュー内変更の処理が完了する
- **THEN** ヘッダーのステータスが「Completed」に変更される
- **AND** ステータスパネルの左側に「Done」が緑色で表示される
- **AND** TUIは表示を維持し、ユーザーが `q` キーで終了できる

#### Scenario: 完了後のキュー変更

- **WHEN** AppModeがCompletedである
- **AND** ユーザーがSpaceキーを押す
- **THEN** NotQueued状態の変更はQueuedに変更できる
- **AND** Queued状態の変更はNotQueuedに変更できる
- **AND** Completed/Archived/Error状態の変更は変更できない

#### Scenario: 完了後の再実行

- **WHEN** AppModeがCompletedである
- **AND** キューに変更が追加されている
- **AND** ユーザーがF5キーを押す
- **THEN** AppModeがRunningに変更される
- **AND** キュー内の変更の処理が開始される

### Requirement: TUIレイアウト構成

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** log panel is visible with stop messages

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Stopped" status in gray
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)

### Requirement: 自動更新機能

TUIは定期的に変更一覧を自動更新しなければならない（SHALL）。

#### Scenario: 定期的な自動更新
- **WHEN** TUIが表示されている
- **THEN** 5秒間隔で `openspec list` が実行される
- **AND** 変更一覧の進捗状況が更新される

#### Scenario: 更新中の表示継続
- **WHEN** 自動更新が実行中である
- **THEN** TUIの表示は中断されない
- **AND** 更新完了後に変更一覧が反映される

### Requirement: 新規変更検出

自動更新時に新しい変更が検出された場合、適切に表示しなければならない（SHALL）。

#### Scenario: 新規変更の検出
- **WHEN** 自動更新により新しい変更が検出される
- **THEN** 新しい変更が変更一覧に追加される
- **AND** 「NEW」バッジが表示される
- **AND** ログに「Discovered new change: <id>」と出力される

#### Scenario: 新規変更のデフォルト状態
- **WHEN** 新しい変更が検出される
- **THEN** その変更はデフォルトで未選択状態（`[ ]`）である
- **AND** フッターの新規件数が更新される

#### Scenario: NEWバッジの表示
- **WHEN** 変更が新規検出されたものである
- **THEN** 変更名の横に「NEW」バッジが表示される
- **AND** バッジは視覚的に目立つ色で表示される

### Requirement: 動的実行キュー

実行モードで未選択の変更を選択するとキューに追加でき、キュー待機中の変更を解除できなければならない（SHALL）。追加された変更はオーケストレータによって実際に処理されなければならない。

#### Scenario: 実行中のキュー追加

- **WHEN** TUIが実行モードである
- **AND** ユーザーが未選択の変更（NotQueued）にカーソルを合わせてSpaceキーを押す
- **THEN** その変更が実行キューに追加される
- **AND** 表示が「not queued」から「queued」に更新される
- **AND** 共有キューにその変更IDがプッシュされる

#### Scenario: キュー待機中の変更を解除

- **WHEN** TUIが実行モードである
- **AND** ユーザーがキュー待機中（Queued）の変更にカーソルを合わせてSpaceキーを押す
- **THEN** その変更がキューから取り除かれる
- **AND** 表示が「queued」から「not queued」に更新される
- **AND** 選択状態が解除される

#### Scenario: キュー追加後の処理順序

- **WHEN** 変更が動的にキューに追加される
- **THEN** その変更は現在処理中の変更の完了後に処理される
- **AND** 既にキュー内にある変更の順序は変わらない

#### Scenario: 処理中の変更は変更不可

- **WHEN** 変更が処理中（Processing）である
- **THEN** その変更の選択状態は変更できない
- **AND** Spaceキーを押しても何も起こらない

#### Scenario: アーカイブ中の変更は変更不可

- **WHEN** 変更がアーカイブ処理中である
- **THEN** その変更の選択状態は変更できない
- **AND** Spaceキーを押しても何も起こらない

#### Scenario: Waiting状態での動的キュー追加

- **WHEN** TUIが実行モードであり「Waiting...」と表示されている
- **AND** 現在処理中の変更がない状態である
- **AND** ユーザーが未選択の変更（NotQueued）にカーソルを合わせてSpaceキーを押す
- **THEN** その変更が実行キューに追加される
- **AND** オーケストレータがその変更を検出して処理を開始する
- **AND** ログに「Processing dynamically added: <change-id>」と表示される

#### Scenario: 動的に追加された変更の処理完了

- **WHEN** 動的に追加された変更の処理が完了する
- **THEN** その変更のステータスが「completed」または「archived」に更新される
- **AND** 残りの動的キューがあれば続けて処理される
- **AND** 初期キューと動的キューの両方が空になったら「AllCompleted」イベントが送信される

#### Scenario: 重複追加の防止

- **WHEN** 既にキューに存在する変更を再度追加しようとする
- **THEN** 追加は無視される
- **AND** 警告ログが表示される

### Requirement: エラー状態の表示

エラー発生時、TUIはエラー状態を明示的に表示しなければならない（SHALL）。

#### Scenario: エラー発生時のモード遷移

- **WHEN** opencode実行がエラー（LLMエラー、料金不足等）で失敗する
- **THEN** TUIのモードが「Error」に遷移する
- **AND** ヘッダーのステータスが「Error」と赤色で表示される

#### Scenario: ステータスパネルのエラー表示

- **WHEN** TUIがエラー状態である
- **THEN** ステータスパネルに「Error in <change_id>」と表示される
- **AND** 「Press F5 to retry」のガイダンスが表示される

#### Scenario: エラー状態でのChange表示

- **WHEN** TUIがエラー状態である
- **THEN** エラーが発生したChangeのステータスは「[error]」と赤色で表示される
- **AND** 他のqueued状態のChangeはそのまま維持される

### Requirement: F5キーでのエラーリトライ

エラー状態でF5キーを押すと、エラーが発生したChangeの処理をリトライできなければならない（SHALL）。

#### Scenario: F5キーでリトライ開始

- **WHEN** TUIがエラー状態である
- **AND** ユーザーがF5キーを押す
- **THEN** エラー状態のChangeが再度キューに追加される
- **AND** TUIが「Running」モードに遷移する
- **AND** 処理が再開される

#### Scenario: リトライ時のログ表示

- **WHEN** ユーザーがF5キーでリトライを開始する
- **THEN** ログパネルに「Retrying: <change_id>」と表示される

#### Scenario: リトライ成功後の状態

- **WHEN** リトライした処理が成功する
- **THEN** Changeのステータスが「completed」または「archived」に更新される
- **AND** 残りのキュー内Changeがあれば続けて処理される

### Requirement: init Subcommand

`init` subcommand SHALL generate a `.openspec-orchestrator.jsonc` configuration template file in the current directory.

#### Scenario: Generate default template (claude)

- **WHEN** user runs `openspec-orchestrator init`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** the template includes apply_command, archive_command, analyze_command, and hooks

#### Scenario: Generate opencode template

- **WHEN** user runs `openspec-orchestrator init --template opencode`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with OpenCode template
- **AND** commands use `opencode run` pattern

#### Scenario: Generate claude template explicitly

- **WHEN** user runs `openspec-orchestrator init --template claude`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Claude Code template
- **AND** commands use `claude --dangerously-skip-permissions -p` pattern

#### Scenario: Generate codex template

- **WHEN** user runs `openspec-orchestrator init --template codex`
- **AND** no `.openspec-orchestrator.jsonc` exists in the current directory
- **THEN** a `.openspec-orchestrator.jsonc` file is created with Codex template
- **AND** commands use `codex` pattern

#### Scenario: Config file already exists without force flag

- **WHEN** user runs `openspec-orchestrator init`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the command exits with an error
- **AND** an error message indicates the file already exists
- **AND** suggests using `--force` to overwrite

#### Scenario: Overwrite existing config with force flag

- **WHEN** user runs `openspec-orchestrator init --force`
- **AND** `.openspec-orchestrator.jsonc` already exists in the current directory
- **THEN** the existing file is overwritten with the new template
- **AND** a success message is displayed

#### Scenario: Invalid template name

- **WHEN** user runs `openspec-orchestrator init --template invalid`
- **THEN** the command exits with an error
- **AND** an error message lists valid template options (opencode, claude, codex)

### Requirement: フッターの動的ガイダンス表示

選択モードのフッターは、アプリケーションの状態に応じて適切なガイダンスメッセージを表示しなければならない（SHALL）。

#### Scenario: 変更がない場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 変更リストが空である
- **THEN** フッターに "Add new proposals to get started" と表示される

#### Scenario: 変更が未選択の場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 変更が1つ以上存在する
- **AND** 選択されている変更が0件である
- **THEN** フッターに "Select changes with Space to process" と表示される

#### Scenario: 変更が選択済みの場合のガイダンス
- **WHEN** TUIが選択モードである
- **AND** 1つ以上の変更が選択されている
- **THEN** フッターに "Press F5 to start processing" と表示される

### Requirement: 実行中フッターの進捗バー表示

実行モードのフッターには、全体の処理進捗をバーで表示しなければならない（SHALL）。

#### Scenario: 実行中の進捗バー表示
- **WHEN** TUIが実行モードである
- **THEN** フッターにキュー内全タスクの進捗バーが表示される
- **AND** 進捗バーは完了タスク数/総タスク数に基づいて計算される
- **AND** パーセンテージが数値で表示される

#### Scenario: 進捗バーの計算方法
- **WHEN** 進捗バーを表示する
- **THEN** 総タスク数は処理対象全変更（Queued, Processing, Completed, Archived）の `total_tasks` の合計である
- **AND** 完了タスク数は処理対象全変更の `completed_tasks` の合計である
- **AND** 進捗率は `completed_tasks / total_tasks * 100` で計算される
- **AND** NotQueued および Error 状態の変更は進捗計算に含まれない

#### Scenario: 完了タスクの進捗保持
- **WHEN** 変更が Completed または Archived 状態に遷移する
- **THEN** その変更のタスク進捗は引き続き進捗バーの計算に含まれる
- **AND** 進捗パーセンテージは減少しない（単調増加）

#### Scenario: タスク数が0の場合
- **WHEN** 進捗バーを表示する
- **AND** 総タスク数が0である
- **THEN** 進捗バーは0%として表示される

### Requirement: Processing Item Spinner Animation

The TUI SHALL display an animated spinner next to items with `Processing` status in running mode.

#### Scenario: Spinner display for processing items
- **WHEN** TUI is in running mode
- **AND** an item has `QueueStatus::Processing`
- **THEN** an animated spinner character is displayed before the progress percentage
- **AND** the spinner cycles through Braille dot characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
- **AND** the display format is "⠋ [XX%]" where ⠋ is the current spinner character

#### Scenario: Spinner animation timing
- **WHEN** TUI is rendering in running mode
- **THEN** the spinner character advances to the next frame approximately every 100ms
- **AND** the spinner cycles continuously until processing completes

#### Scenario: Spinner not shown for non-processing items
- **WHEN** TUI is in running mode
- **AND** an item has status other than `Processing` (Queued, Completed, Error)
- **THEN** no spinner is displayed for that item

### Requirement: 完了検出のリトライ設定

完了状態の検出においてリトライ動作を実装しなければならない（SHALL）。

#### Scenario: デフォルトのリトライ設定

- **WHEN** 設定ファイルにリトライ設定がない
- **THEN** 最大リトライ回数は3回である
- **AND** リトライ間隔は500ミリ秒である

#### Scenario: キャンセル時のリトライ中断

- **WHEN** リトライループ実行中である
- **AND** キャンセルトークンがキャンセルされる
- **THEN** リトライループは即座に終了する
- **AND** プロセスは適切にクリーンアップされる

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

The CLI SHALL support a `--version` flag to display the application version.

#### Scenario: Display version with --version flag
- **WHEN** user runs `openspec-orchestrator --version`
- **THEN** the application version from Cargo.toml is displayed
- **AND** the program exits with code 0

#### Scenario: Display version with -V short flag
- **WHEN** user runs `openspec-orchestrator -V`
- **THEN** the application version is displayed (same as `--version`)

### Requirement: TUI Header Version Display

The TUI header SHALL display the application version in both selection and running modes.

#### Scenario: Version in selection mode header
- **WHEN** TUI is in selection mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

#### Scenario: Version in running mode header
- **WHEN** TUI is in running mode
- **THEN** the header displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the header
- **AND** the version text uses a muted/gray color to avoid distraction

### Requirement: Terminal Status Task Count Display

The TUI running mode SHALL display terminal states with status-only text and task counts in a separate column, avoiding redundant display.

#### Scenario: Completed state display format
- **WHEN** a change is in `completed` status in running mode
- **THEN** the status text SHALL be displayed as `[completed]` (without task count)
- **AND** the status is displayed in green color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Archived state display format
- **WHEN** a change is in `archived` status in running mode
- **THEN** the status text SHALL be displayed as `[archived]` (without task count)
- **AND** the status is displayed in blue color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Error state display format
- **WHEN** a change is in `error` status in running mode
- **THEN** the status text SHALL be displayed as `[error]` (without task count)
- **AND** the status is displayed in red color
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

#### Scenario: Processing state keeps progress percentage with task count
- **WHEN** a change is in `processing` status in running mode
- **THEN** the status text SHALL continue to display progress percentage as `⠋ [ XX%]`
- **AND** task counts SHALL be displayed in a separate column as `X/Y`

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

The TUI SHALL track archived changes reliably and report accurate final status.

#### Scenario: All changes archived successfully
- **WHEN** all queued changes have been processed and archived
- **THEN** the final verification reports "All processed changes have been archived"
- **AND** no unarchived warnings are displayed

#### Scenario: Archive failure handling
- **WHEN** an archive command fails for a change
- **THEN** the change is marked as errored
- **AND** the error is logged with details
- **AND** the change is not removed from tracking until explicitly handled

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

### Requirement: approve Subcommand

The CLI SHALL provide an `approve` subcommand to manage change approval status.

#### Scenario: Approve a change with set action

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory `openspec/changes/{change_id}/` exists
- **THEN** an `approved` file is created in the change directory
- **AND** the file contains MD5 checksums of all `.md` files (except `tasks.md`)
- **AND** a success message is displayed

#### Scenario: Approve a change that doesn't exist

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory does not exist
- **THEN** an error message is displayed
- **AND** exit code is non-zero

#### Scenario: Unapprove a change with unset action

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file exists
- **THEN** the `approved` file is deleted
- **AND** a success message is displayed

#### Scenario: Unapprove a change that is not approved

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file does not exist
- **THEN** a message indicates the change was not approved
- **AND** exit code is zero (no-op)

#### Scenario: Check approval status

- **WHEN** user runs `openspec-orchestrator approve status {change_id}`
- **THEN** the approval status is displayed
- **AND** if approved, shows "approved" with file count
- **AND** if not approved, shows reason (file missing, hash mismatch, etc.)

### Requirement: TUI Approval Toggle

The TUI SHALL allow users to toggle approval status using the `@` key, with different auto-queue behavior based on orchestrator state.

#### Scenario: Approve unapproved change in Running mode (approve only)

- **WHEN** TUI is in Running mode (orchestrator actively processing)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved but NOT queued (`[@]`)
- **AND** checkbox transitions from `[ ]` to `[@]`
- **AND** log message indicates approval only

#### Scenario: Approve unapproved change in Select mode adds to queue automatically

- **WHEN** TUI is in Select mode (orchestrator stopped)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Approve unapproved change in Completed mode adds to queue automatically

- **WHEN** TUI is in Completed mode (orchestrator stopped, all queued changes done)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Unapprove approved-but-not-queued change

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on an approved but not queued change (`[@]`)
- **THEN** the change becomes unapproved (`[ ]`)
- **AND** checkbox transitions from `[@]` to `[ ]`

#### Scenario: Unapprove queued change removes from queue

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on a queued change (`[x]`) that is NOT processing
- **THEN** the change becomes unapproved AND removed from queue (`[ ]`)
- **AND** checkbox transitions from `[x]` to `[ ]`
- **AND** log message indicates both unapproval and queue removal

#### Scenario: Toggle approval blocked for processing change

- **WHEN** TUI is in Running mode
- **AND** user presses `@` key
- **AND** highlighted change is in `Processing` state
- **THEN** approval status is NOT changed
- **AND** a warning message is displayed: "Cannot change approval for processing change"

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL automatically queue approved changes when starting in TUI mode.

#### Scenario: TUI startup with approved changes

- **WHEN** user starts the TUI
- **AND** one or more changes have valid `approved` files
- **THEN** those changes are automatically selected and queued
- **AND** a log message indicates "Auto-queued N approved changes"

#### Scenario: TUI startup with no approved changes

- **WHEN** user starts the TUI
- **AND** no changes have valid `approved` files
- **THEN** no changes are automatically queued
- **AND** the user can manually select and approve changes

### Requirement: Unapproved Changes Cannot Be Queued

The system SHALL prevent unapproved changes from being added to the execution queue.

#### Scenario: Attempt to queue unapproved change in TUI

- **WHEN** TUI is in selection mode
- **AND** user presses Space to select an unapproved change
- **THEN** the change can be selected for viewing
- **AND** pressing F5 with only unapproved changes selected shows warning
- **AND** the warning suggests approving changes first

#### Scenario: CLI run with unapproved change

- **WHEN** user runs `openspec-orchestrator run --change {change_id}`
- **AND** the change is not approved
- **THEN** a warning message is displayed
- **AND** the change is NOT added to the queue
- **AND** processing continues with any remaining approved changes

#### Scenario: CLI run with mixed approved/unapproved changes

- **WHEN** user runs `openspec-orchestrator run --change a,b,c`
- **AND** change `a` is approved, `b` is not approved, `c` is approved
- **THEN** warning is displayed for change `b`
- **AND** only changes `a` and `c` are processed

### Requirement: Log Entry Limit

The TUI SHALL maintain a maximum limit on stored log entries to prevent unbounded memory growth.

#### Scenario: Log entry limit enforcement
- **WHEN** a new log entry is added
- **AND** the total log count exceeds 1000 entries
- **THEN** the oldest log entry is removed
- **AND** scroll offset is adjusted if necessary to prevent display issues

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

The orchestrator MUST capture the agent's final summary message from each apply attempt and include it in subsequent apply prompts for the same change.

#### Scenario: First apply attempt has no history

- **WHEN** the orchestrator executes apply for a change for the first time
- **THEN** the prompt contains only the base apply_prompt from configuration
- **AND** no `<last_apply>` tags are included

#### Scenario: Second apply includes previous attempt summary

- **WHEN** the orchestrator executes apply for a change for the second time
- **AND** the first attempt returned a summary message from the agent
- **THEN** the prompt contains the base apply_prompt
- **AND** the prompt contains a `<last_apply attempt="1">` block
- **AND** the block contains the agent's summary message from the first attempt

#### Scenario: Multiple previous attempts are included

- **WHEN** the orchestrator executes apply for a change for the third time
- **THEN** the prompt contains `<last_apply attempt="1">` and `<last_apply attempt="2">` blocks
- **AND** blocks are ordered by attempt number (oldest first)
- **AND** each block contains the agent's summary message from that attempt

#### Scenario: History is cleared on archive

- **WHEN** a change is successfully archived
- **THEN** the apply history for that change is cleared from memory
- **AND** subsequent apply attempts for the same change_id (if unarchived) start fresh

### Requirement: Apply History Context Format

The apply history context MUST be formatted as XML-like tags containing the agent's summary message.

#### Scenario: Context format structure

- **GIVEN** a previous apply attempt where the agent returned the summary:
  "Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing."
- **WHEN** the context is formatted for the next prompt
- **THEN** the output is:
  ```
  <last_apply attempt="1">
  Implemented task 1.1 and 1.2. Found issue with type conversion in auth.rs:42 that needs fixing.
  </last_apply>
  ```

#### Scenario: Context appended to base prompt

- **GIVEN** base apply_prompt is "スコープ外タスクは削除せよ"
- **AND** there is one previous attempt with agent summary "Task 1.1 completed."
- **WHEN** the full prompt is built
- **THEN** the prompt format is:
  ```
  スコープ外タスクは削除せよ

  <last_apply attempt="1">
  Task 1.1 completed.
  </last_apply>
  ```

#### Scenario: Agent summary captured from apply response

- **WHEN** the openspec:apply skill completes execution
- **THEN** the agent returns a summary message describing work done
- **AND** the orchestrator captures this summary message for history

### Requirement: TUI Stop Processing with Escape Key

The TUI SHALL allow users to stop ongoing processing using the Escape key.

#### Scenario: First Esc press initiates graceful stop

- **WHEN** TUI is in Running mode
- **AND** an agent process is actively running
- **AND** user presses Escape key
- **THEN** the TUI transitions to Stopping mode
- **AND** header status displays "Stopping..." in yellow
- **AND** log displays "Stopping after current change completes..."
- **AND** current agent process continues to completion
- **AND** no new changes are picked up for processing

#### Scenario: Second Esc press forces immediate stop

- **WHEN** TUI is in Stopping mode
- **AND** user presses Escape key again
- **THEN** the current agent process is terminated immediately (SIGTERM)
- **AND** the TUI transitions to Stopped mode
- **AND** log displays "Force stopped - process terminated"
- **AND** the interrupted change status becomes "queued" (not error)

#### Scenario: Graceful stop completes naturally

- **WHEN** TUI is in Stopping mode
- **AND** the current agent process completes successfully
- **THEN** the TUI transitions to Stopped mode
- **AND** the completed change transitions to appropriate status (completed/archived)
- **AND** log displays "Stopped - processing halted"

#### Scenario: Esc has no effect in selection mode

- **WHEN** TUI is in Selecting mode
- **AND** user presses Escape key
- **THEN** nothing happens
- **AND** the TUI remains in Selecting mode

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode where users can review progress and manage the queue before resuming.

#### Scenario: Stopped mode display

- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Stopped" in gray color
- **AND** the change list remains visible with current statuses
- **AND** completed changes show "[completed]" or "[archived]"
- **AND** remaining queued changes show "queued"

#### Scenario: Queue management in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a queued change
- **THEN** the change is removed from queue (becomes not queued)

#### Scenario: Queue addition in Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-queued change
- **THEN** the change is added to the queue

#### Scenario: Resume processing from Stopped mode

- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are queued
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes with the queued changes
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning

- **WHEN** TUI is in Stopped mode
- **AND** no changes are queued
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode

### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Running mode help text

- **WHEN** TUI is in Running mode
- **THEN** help text includes "Esc: stop"
- **AND** help text continues to show "q: quit"

#### Scenario: Stopping mode help text

- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text shows "Waiting for current process..."

#### Scenario: Stopped mode help text

- **WHEN** TUI is in Stopped mode
- **THEN** help text includes "F5: resume"
- **AND** help text includes "Space: toggle queue"
- **AND** help text includes "q: quit"

### Requirement: Interrupted Change Handling

Changes interrupted by force stop SHALL be handled gracefully.

#### Scenario: Force-stopped change returns to queued

- **WHEN** a change is being processed
- **AND** user force stops with second Esc press
- **THEN** the change status becomes "queued" (not error)
- **AND** the change can be re-processed on resume
- **AND** no error message is displayed for the interruption

#### Scenario: Partial progress preserved

- **WHEN** a change had some tasks completed before force stop
- **THEN** the completed tasks remain completed
- **AND** the tasks.md file reflects actual progress
- **AND** resuming continues from the partial state

### Requirement: jj Repository Detection

The CLI SHALL detect whether the current directory is a jj-managed repository by checking for the `.jj` directory.

#### Scenario: jj repository detected
- **WHEN** a `.jj` directory exists in the current working directory
- **THEN** jj features (parallel mode) are available

#### Scenario: jj repository not detected
- **WHEN** no `.jj` directory exists in the current working directory
- **AND** user runs `openspec-orchestrator run --parallel`
- **THEN** the command exits with a non-zero exit code
- **AND** an error message is displayed: "Error: --parallel requires a jj repository (.jj directory not found)"

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using jj workspaces. Parallel mode is OFF by default.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** a `.jj` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode disabled by default
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: Parallel mode requires jj directory
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `.jj` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates jj repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
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
- **WHEN** user runs `openspec-orchestrator run --parallel --dry-run`
- **THEN** the analyzer determines parallelization groups
- **AND** the groups are displayed without executing any changes
- **AND** no workspaces are created
