## MODIFIED Requirements
### Requirement: REQ-OBS-001 Command Execution Logging

オーケストレーターは外部コマンドを実行する前にコマンド情報をログ出力しなければならない（MUST）。

ログには以下を含めなければならない（MUST）。
- 実行ファイル名
- 引数一覧
- 作業ディレクトリ（設定されている場合）

apply/archive/resolveのAIエージェントコマンドは、`{change_id}`、`{prompt}`、`{conflict_files}`などのプレースホルダーを展開した完全なコマンド文字列を、実行前にTUI Logs Viewへ表示しなければならない（MUST）。このログはユーザー向けの`info`相当ログとして扱う（SHALL）。

hookコマンドは、実行前にコマンド文字列をTUI Logs Viewへ表示しなければならない（MUST）。
hookコマンドのstdout/stderrは取得可能な範囲でTUI Logs Viewへ表示しなければならない（MUST）。

**追加**: TUI Logs Viewに表示されるすべてのログエントリーは、`--logs`オプションが指定されている場合にデバッグログファイルにも出力されなければならない（MUST）。

#### Scenario: hook実行のコマンドと出力がLogs Viewに表示される
- **GIVEN** `hooks.pre_apply` が `echo 'hello'` に設定されている
- **WHEN** pre_apply hook が実行される
- **THEN** Logs View に `Running pre_apply hook: echo 'hello'` が表示される
- **AND** Logs View に `hello` が表示される
