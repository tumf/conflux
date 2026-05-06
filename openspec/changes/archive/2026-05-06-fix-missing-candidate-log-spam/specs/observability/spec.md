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

TUI Logs Viewに表示されるすべてのログエントリーは、常にデバッグログファイルにも出力されなければならない（MUST）。出力先は `XDG_STATE_HOME` が設定されていれば `XDG_STATE_HOME/cflx/logs/<project_slug>/<YYYY-MM-DD>.log`、未設定時は `~/.local/state/cflx/logs/<project_slug>/<YYYY-MM-DD>.log` とする（MUST）。ログは日付単位で分割し、`project_slug` ごとに最新7日分のみ保持しなければならない（MUST）。

ただし、scheduler loop 等から発生する同一状態・同一理由の診断ログは、user-visible TUI Logs View と debug log file のどちらでも連続して同一内容が大量表示されないよう、dedupe、rate-limit、または summary 化してよい（MAY）。この抑制は観測性のためだけに使われ、workflow-control input として使ってはならない（MUST NOT）。

#### Scenario: repetitive scheduler diagnostics are bounded in TUI logs and debug files

- **GIVEN** a scheduler diagnostic has the same change id, reason, and message across repeated loop iterations
- **WHEN** the diagnostic is emitted repeatedly without any relevant state change
- **THEN** the TUI Logs View does not show an unbounded sequence of identical entries
- **AND** the debug log file does not show an unbounded sequence of identical WARN-level entries for the same scheduler diagnostic
- **AND** the diagnostic remains available at least once or through a summary/rate-limited entry
- **AND** suppression state is not used to decide scheduling, resume routing, acceptance, archive, or next-action behavior
