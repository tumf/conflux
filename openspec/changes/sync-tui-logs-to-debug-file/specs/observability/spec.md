# observability Spec Delta

## MODIFIED Requirements

### Requirement: REQ-OBS-001 すべてのコマンド実行のログ記録

オーケストレーターは外部コマンド（`tokio::process::Command`, `std::process::Command`）を実行する前に、コマンド情報をログに記録しなければならない (MUST)。

ログには以下の情報を含めなければならない：
- 実行可能ファイル名
- 引数リスト
- 作業ディレクトリ（設定されている場合）

**追加**: TUI Logs Viewに表示されるすべてのログエントリは、`--logs`オプション指定時にデバッグログファイルにも出力されなければならない (MUST)。

#### Scenario: VCSコマンド実行時のログ出力

- **GIVEN** git worktreeを作成する
- **WHEN** `git worktree add` コマンドが実行される
- **THEN** ログに `debug!` レベルでコマンドライン全体が記録される
- **AND** ログに作業ディレクトリが含まれる

#### Scenario: AIエージェントコマンド実行時のログ出力

- **GIVEN** changeをapplyする
- **WHEN** OpenCodeエージェントコマンドが実行される
- **THEN** ログに `info!` レベルでコマンドラインが記録される

#### Scenario: フック実行時のログ出力

- **GIVEN** on_apply_startフックが設定されている
- **WHEN** フックコマンドが実行される
- **THEN** ログに `info!` レベルでコマンドラインが記録される
- **AND** ログに "Running on_apply_start hook" というコンテキストが含まれる

#### Scenario: TUI LogsのデバッグログファイルへのSync

- **GIVEN** TUIが`--logs /tmp/debug.log`オプション付きで起動している
- **WHEN** エージェント処理中にエラーが発生し、TUI Logs Viewにエラーが表示される
- **THEN** 同じエラーメッセージがデバッグログファイルにも`ERROR`レベルで記録される
- **AND** ログには`tui_log`ターゲットが含まれる

#### Scenario: 警告ログのSync

- **GIVEN** TUIが`--logs /tmp/debug.log`オプション付きで起動している
- **WHEN** マージが延期され、TUI Logs Viewに警告が表示される
- **THEN** 同じ警告メッセージがデバッグログファイルにも`WARN`レベルで記録される

#### Scenario: 情報ログのSync

- **GIVEN** TUIが`--logs /tmp/debug.log`オプション付きで起動している
- **WHEN** 処理が開始され、TUI Logs Viewに情報ログが表示される
- **THEN** 同じメッセージがデバッグログファイルにも`INFO`レベルで記録される
