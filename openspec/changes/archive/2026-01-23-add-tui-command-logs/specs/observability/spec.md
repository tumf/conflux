## MODIFIED Requirements
### Requirement: REQ-OBS-001 Command Execution Logging

オーケストレーターは外部コマンドを実行する前にコマンド情報をログ出力しなければならない（MUST）。

ログには以下を含めなければならない（MUST）。
- 実行ファイル名
- 引数一覧
- 作業ディレクトリ（設定されている場合）

apply/archive/resolveのAIエージェントコマンドは、`{change_id}`、`{prompt}`、`{conflict_files}`などのプレースホルダーを展開した完全なコマンド文字列を、実行前にTUI Logs Viewへ表示しなければならない（MUST）。このログはユーザー向けの`info`相当ログとして扱う（SHALL）。

**追加**: TUI Logs Viewに表示されるすべてのログエントリーは、`--logs`オプションが指定されている場合にデバッグログファイルにも出力されなければならない（MUST）。

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
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** エージェント処理中にエラーが発生しTUI Logs Viewに表示される
- **THEN** 同じエラーメッセージがデバッグログファイルに`ERROR`レベルで記録される
- **AND** ログには`tui_log`ターゲットが含まれる

#### Scenario: Warningログの同期
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** マージが延期され警告がTUI Logs Viewに表示される
- **THEN** 同じ警告メッセージがデバッグログファイルに`WARN`レベルで記録される

#### Scenario: Infoログの同期
- **GIVEN** TUIが`--logs /tmp/debug.log`オプションで起動されている
- **WHEN** 処理開始時のinfoログがTUI Logs Viewに表示される
- **THEN** 同じメッセージがデバッグログファイルに`INFO`レベルで記録される
