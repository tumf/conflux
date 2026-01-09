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

#### Scenario: run サブコマンドの基本実行
- **WHEN** ユーザーが `openspec-orchestrator run` を実行する
- **THEN** オーケストレーションループが開始される

#### Scenario: 特定の変更を指定して実行
- **WHEN** ユーザーが `openspec-orchestrator run --change <id>` を実行する
- **THEN** 指定された変更のみを処理する

#### Scenario: opencode パスのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --opencode-path <path>` を実行する
- **THEN** 指定されたパスの opencode バイナリを使用する

#### Scenario: openspec コマンドのカスタマイズ
- **WHEN** ユーザーが `openspec-orchestrator run --openspec-cmd <cmd>` を実行する
- **THEN** 指定されたコマンドで openspec を実行する

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

実行モードでは、処理中の変更の進捗状況をダッシュボード形式で表示しなければならない（SHALL）。

#### Scenario: 変更一覧の進捗表示

- **WHEN** TUIが実行モードである
- **THEN** 全ての変更がキュー状態と共に表示される
- **AND** 各変更の完了タスク数/総タスク数とパーセンテージが表示される

#### Scenario: キュー状態の表示

- **WHEN** TUIが実行モードである
- **THEN** 処理中の変更は進捗バーと共に表示される
- **AND** キュー待機中の変更は「queued」と表示される
- **AND** 未選択の変更は「not queued」と表示される
- **AND** エラーが発生した変更は「error」と赤色で表示される

#### Scenario: 現在処理中の変更のハイライト

- **WHEN** 変更が処理中である
- **THEN** 処理中の変更が視覚的にハイライトされる（`►` マーカー）
- **AND** ステータスパネルに変更IDと処理状況が表示される

#### Scenario: ログのリアルタイム表示

- **WHEN** オーケストレーションが実行中である
- **THEN** ログメッセージがログパネルにリアルタイムで追加される
- **AND** 最新のログが常に表示される（自動スクロール）

#### Scenario: 処理完了時の表示

- **WHEN** 全てのキュー内変更の処理が完了する
- **THEN** ヘッダーのステータスが「Completed」に変更される
- **AND** TUIは表示を維持し、ユーザーが `q` キーで終了できる

#### Scenario: エラー発生時の表示

- **WHEN** 変更の処理中にエラーが発生する
- **THEN** ヘッダーのステータスが「Error」と赤色で表示される
- **AND** ステータスパネルにエラー情報と「Press F5 to retry」が表示される
- **AND** TUIは表示を維持し、ユーザーがF5でリトライまたは`q`キーで終了できる

### Requirement: TUIレイアウト構成

TUIは選択モードと実行モードで適切なレイアウトを表示しなければならない（SHALL）。

#### Scenario: 選択モードのレイアウト
- **WHEN** TUIが選択モードである
- **THEN** ヘッダー（タイトル、モード表示、自動更新インジケーター）が上部に表示される
- **AND** 操作ヘルプ（↑↓: move, Space: toggle, F5: run, q: quit）が表示される
- **AND** チェックボックス付き変更リストが中央に表示される
- **AND** 選択件数・新規件数がフッターに表示される
- **AND** アプリケーション状態に応じたガイダンスメッセージがフッターに表示される

#### Scenario: 実行モードのレイアウト
- **WHEN** TUIが実行モードである
- **THEN** ヘッダー（タイトル、Running/Completedステータス、自動更新インジケーター）が上部に表示される
- **AND** キュー状態付き変更リストが表示される
- **AND** 現在処理パネル（変更ID、ステータス）が表示される
- **AND** ログパネルが下部に表示される

### Requirement: 自動更新機能

TUIは定期的に変更一覧を自動更新しなければならない（SHALL）。

#### Scenario: 定期的な自動更新
- **WHEN** TUIが表示されている
- **THEN** 5秒間隔で `openspec list` が実行される
- **AND** 変更一覧の進捗状況が更新される

#### Scenario: 自動更新インジケーター
- **WHEN** TUIが表示されている
- **THEN** ヘッダーに自動更新間隔とインジケーター（`Auto-refresh: 5s ↻`）が表示される

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

実行モードで未選択の変更を選択するとキューに追加でき、キュー待機中の変更を解除できなければならない（SHALL）。

#### Scenario: 実行中のキュー追加

- **WHEN** TUIが実行モードである
- **AND** ユーザーが未選択の変更（NotQueued）にカーソルを合わせてSpaceキーを押す
- **THEN** その変更が実行キューに追加される
- **AND** 表示が「not queued」から「queued」に更新される

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
- **THEN** 総タスク数は選択された全変更の `total_tasks` の合計である
- **AND** 完了タスク数は選択された全変更の `completed_tasks` の合計である
- **AND** 進捗率は `completed_tasks / total_tasks * 100` で計算される

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

The system SHALL parse `tasks.md` files natively to determine task completion status, independent of the openspec CLI.

#### Scenario: Parse bullet list tasks
- **WHEN** a `tasks.md` file contains bullet list checkboxes (`- [ ]`, `- [x]`)
- **THEN** the system counts each `- [ ]` as an incomplete task
- **AND** the system counts each `- [x]` as a completed task
- **AND** case-insensitive matching is used for `[x]` and `[X]`

#### Scenario: Parse numbered list tasks
- **WHEN** a `tasks.md` file contains numbered list checkboxes (`1. [ ]`, `1. [x]`)
- **THEN** the system counts each numbered `[ ]` as an incomplete task
- **AND** the system counts each numbered `[x]` as a completed task

#### Scenario: Ignore non-task lines
- **WHEN** a `tasks.md` file contains markdown headers, plain text, or indented sub-items
- **THEN** those lines are not counted as tasks
- **AND** only top-level checkbox items are counted

#### Scenario: Fallback when tasks.md not found
- **WHEN** the `tasks.md` file does not exist for a change
- **THEN** the system uses the task count from openspec CLI output
- **AND** no error is raised

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

### Requirement: TUI Footer Version Display

The TUI selection mode footer SHALL display the application version.

#### Scenario: Version in selection mode footer
- **WHEN** TUI is in selection mode
- **THEN** the footer displays the application version (e.g., "v0.1.0")
- **AND** the version is displayed on the right side of the footer
- **AND** the version text uses a muted/gray color to avoid distraction

