## MODIFIED Requirements
### Requirement: REQ-OBS-001 Command Execution Logging

オーケストレーターは外部コマンドを実行する前にコマンド情報をログ出力しなければならない（MUST）。

ログには以下を含めなければならない（MUST）。
- 実行ファイル名
- 引数一覧
- 作業ディレクトリ（設定されている場合）

apply/archive/resolveのAIエージェントコマンドは、`{change_id}`、`{prompt}`、`{conflict_files}`などのプレースホルダーを展開した完全なコマンド文字列を、実行前にTUI Logs Viewへ表示しなければならない（MUST）。このログはユーザー向けの`info`相当ログとして扱う（SHALL）。

**変更**: TUI Logs Viewに表示されるすべてのログエントリーは、常にデバッグログファイルにも出力されなければならない（MUST）。出力先は `XDG_STATE_HOME` が設定されていれば `XDG_STATE_HOME/cflx/logs/<project_slug>/<YYYY-MM-DD>.log`、未設定時は `~/.local/state/cflx/logs/<project_slug>/<YYYY-MM-DD>.log` とする（MUST）。ログは日付単位で分割し、`project_slug` ごとに最新7日分のみ保持しなければならない（MUST）。

#### Scenario: VCSコマンドの実行ログ
- **GIVEN** git worktreeを作成する
- **WHEN** `git worktree add`コマンドを実行する
- **THEN** コマンド全体が`debug!`レベルでログに記録される
- **AND** 作業ディレクトリが含まれる

#### Scenario: AIエージェントコマンドの実行ログ
- **GIVEN** change `alpha` をapplyする
- **WHEN** OpenCodeのapplyコマンドを実行する
- **THEN** コマンドラインが`info!`レベルでログに記録される
- **AND** TUI Logs Viewにはプレースホルダー展開後のコマンドが表示される

#### Scenario: Resolveコマンドのプレースホルダー展開ログ
- **GIVEN** resolve_commandに`{conflict_files}`を含む
- **WHEN** resolveを開始する
- **THEN** TUI Logs Viewには`{conflict_files}`を実際のファイル一覧に展開したコマンドが表示される
- **AND** 表示は実行前に行われる

#### Scenario: フック実行ログ
- **GIVEN** on_apply_startフックが設定されている
- **WHEN** フックコマンドを実行する
- **THEN** コマンドラインが`info!`レベルでログに記録される
- **AND** ログには"Running on_apply_start hook"のコンテキストが含まれる

#### Scenario: TUIログのデバッグファイル同期
- **GIVEN** TUIが起動している
- **WHEN** エージェント処理中にエラーが発生しTUI Logs Viewに表示される
- **THEN** 同じエラーメッセージがデバッグログファイルに`ERROR`レベルで記録される
- **AND** ログには`tui_log`ターゲットが含まれる

#### Scenario: Warningログの同期
- **GIVEN** TUIが起動している
- **WHEN** マージが延期され警告がTUI Logs Viewに表示される
- **THEN** 同じ警告メッセージがデバッグログファイルに`WARN`レベルで記録される

#### Scenario: Infoログの同期
- **GIVEN** TUIが起動している
- **WHEN** 処理開始時のinfoログがTUI Logs Viewに表示される
- **THEN** 同じメッセージがデバッグログファイルに`INFO`レベルで記録される

#### Scenario: CLI(run)のログもファイルに保存される
- **GIVEN** `cflx run` を実行する
- **WHEN** 実行ログが出力される
- **THEN** 同じメッセージがデバッグログファイルに記録される

#### Scenario: 日次ローテーションと7日保持
- **GIVEN** `project_slug` が `conflux-aaaa1111` である
- **AND** ログディレクトリに過去8日分の `YYYY-MM-DD.log` が存在する
- **WHEN** オーケストレーターを起動する
- **THEN** 最新7日分のログのみが保持される
- **AND** 当日分は `<YYYY-MM-DD>.log` に追記される
