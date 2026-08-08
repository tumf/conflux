## MODIFIED Requirements

### Requirement: Log Entry Structure and Display

TUIログエントリは timestamp、message、color、および任意のコンテキスト情報（change ID、operation、iteration number）を含まなければならない (MUST)。
ログヘッダは利用可能なコンテキスト情報に応じて段階的に表示される。
apply/archive/acceptance/resolve の開始時には、対応する subcommand 文字列が TUI ログに表示されなければならない。
subcommand の出力ログは対応する operation を付与して記録されなければならない。

- Logsビュー（ログパネル）では、operation を持つログは change_id がある場合に iteration があれば `[{change_id}:{operation}:{iteration}]`、iteration がない場合に `[{change_id}:{operation}]` 形式で表示しなければならない。
- 変更一覧のログプレビューでは、operation を持つログは iteration がある場合に `[operation:{iteration}]`、iteration がない場合に `[operation]` 形式で表示し、change_id を表示してはならない。
- change_id を持たない analysis のログ出力は必ず iteration number を含み、ヘッダは `[analysis:{iteration}]` 形式で表示されなければならない。
- Logsビューの1行目は timestamp とログヘッダの直後から現在のパネル内側右端までを使用し、表示幅を超えるメッセージの2行目以降はインデントせずにパネル内側の横幅全体を用いて折り返さなければならない。
- Logsビューは保持済みmessageをproducer固定位置で再省略してはならない。`PgUp`、`PgDn`、`Home`、`End` の既存key assignmentは、entry境界だけでなく、viewportより高い単一entry内の折り返し表示行にも到達できなければならない (MUST)。source messageが200文字以上保持されている場合、狭い端末でもこれらの操作により先頭200文字を含む全折り返しsegmentを実際の描画bufferへ表示できなければならない (MUST)。
- Logsビューの表示範囲とnavigation rangeは折り返し後の表示行数で計算され、長文ログの折り返しによって最新ログがauto-scroll viewport外になることがあってはならない。
- auto-scrollが無効な場合、TUIは現在閲覧中のentryとsource-content位置をprocess-local anchorとして維持しなければならない (MUST)。新しいログ追加、filter変更、横幅変更、またはログバッファのtrim後は、そのanchorを現在のfiltered/wrapped sequenceへ決定的に再投影しなければならない (MUST)。anchor対象がtrimされた場合は最も古い残存表示行へclampし、auto-scrollを自動的に再有効化してはならない (MUST NOT)。
- 折り返し、表示行数、末尾省略は Unicode display width で計算され、CJKまたはemojiのUTF-8境界を壊してはならない。

#### Scenario: apply/archive/acceptance/resolve の command が表示される

- **GIVEN** change_id が設定され、apply/archive/acceptance/resolve の開始イベントに command が含まれている
- **WHEN** TUI が開始イベントを処理する
- **THEN** ログに `Command:` 行が追加される
- **AND** ログは対応する operation 付きで記録される

#### Scenario: LogsビューのArchiveログヘッダはchange_idとiterationを含む

- **GIVEN** `change_id="test-change"`、`operation="archive"`、`iteration=2` のログエントリが作成される
- **WHEN** TUI が Logs ビューのログを描画する
- **THEN** ログヘッダは `[test-change:archive:2]` として表示される
- **AND** retry の順序が判別できる

#### Scenario: Analysis ログは iteration 付きで表示される

- **GIVEN** `change_id=None`、`operation="analysis"`、`iteration=3` のログエントリが作成される
- **WHEN** TUI が Logs ビューのログを描画する
- **THEN** ログヘッダは `[analysis:3]` として表示される
- **AND** analysis の再実行が区別できる

#### Scenario: auto-scroll が無効なとき閲覧中content位置が維持される

- **GIVEN** ユーザーがログをスクロール済みで auto-scroll が無効になっている
- **AND** process-local anchorが現在のentryとsource-content位置を指している
- **WHEN** 新しいログ追加、filter変更、横幅変更、またはログバッファtrimにより表示行sequenceが再計算される
- **THEN** anchorは同じentryとsource-content位置を含む新しい表示行へ再投影される
- **AND** anchor対象がtrimされた場合、最も古い残存表示行へclampされる
- **AND** auto-scrollは自動的に再有効化されない

#### Scenario: 幅の広いLogsパネルは追加の内容を表示する

- **GIVEN** 200文字を超える保持済みログメッセージがある
- **WHEN** 同じログを狭いLogsパネルと広いLogsパネルで描画する
- **THEN** 各行はそれぞれの現在のパネル内側幅を使用する
- **AND** 広いパネルの1行は狭いパネルより多くのmessage contentを表示する
- **AND** 200文字等のproducer固定位置に不要なellipsisは表示されない

#### Scenario: 狭く低いLogsパネルで単一entry内を移動できる

- **GIVEN** 200文字以上の保持済みログメッセージがある
- **AND** 1行に200文字を表示できず、entry全体もviewport高に収まらないLogsパネルである
- **WHEN** TUIがログを描画し、ユーザーが `Home`、`PgDn`、`PgUp`、`End` を操作する
- **THEN** 先頭行はtimestampとheader後の利用可能幅を使用する
- **AND** 継続行はインデントせずパネル内側幅全体を使用する
- **AND** operation sequenceを通して少なくとも先頭200文字を含む全折り返しsegmentが実際の描画bufferに現れる
- **AND** `End` またはauto-scroll有効時には最新ログ行が表示範囲から外れない

#### Scenario: Unicodeログの折り返しは表示幅と文字境界を守る

- **GIVEN** CJKとemojiを含む長いログメッセージがある
- **WHEN** TUI が異なる幅のLogsパネルで折り返す
- **THEN** 各表示行はパネル内側のdisplay widthを超えない
- **AND** UTF-8文字境界は壊れない
- **AND** 折り返し部分を結合すると共有安全上限内の保持済みmessage contentが失われていない

### Requirement: Change List Log Preview

The TUI change list MUST display a single-line preview in the remaining space on the right side of each change row. For a change whose display status is `error`, the preview MUST prefer the retained final change-level diagnostic over every buffered log entry and MUST format it as `Error: <diagnostic>`. This error preview MUST remain available independently of bounded log retention. If the status is `error` but no diagnostic is available, the preview MUST use an explicit fallback such as `Error details unavailable` and MUST NOT present an unrelated ordinary log as the failure reason. For every non-error change, the preview MUST display the latest retained log entry and include its relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more), the shortened header format `[operation:{iteration}]` or `[operation]`, and the message.

Every preview MUST remain exactly one display line and MUST NOT wrap or increase the change-row height. The renderer MUST use all actual remaining row width and truncate with an ellipsis only when the retained preview does not fit that width. Producer retention is governed by the observability capability; this renderer MUST apply no additional fixed-position cutoff. Truncation MUST use Unicode display width, MUST NOT break UTF-8 character boundaries, and MUST NOT panic for CJK or emoji. Error previews MUST use readable error styling in both focused and unfocused rows.

- For relative times of 1 minute or more on non-error log previews, the display MUST include up to 2 units. Units MUST be `d` / `h` / `m`, formatted as space-separated units such as `1d 12h ago` or `3h 20m ago`. Values MUST be truncated (no rounding up).
- If no log entry exists for a non-error change, the preview MUST NOT be displayed.
- If the available width for the preview is less than 10 characters, the preview MUST NOT be displayed.
- The relative time for a non-error log preview MUST be computed at render time from the log entry creation time and the current time, and the display MUST update at 1-second granularity.

#### Scenario: Wider change row reveals more retained preview content

- **GIVEN** a non-error change has a latest retained log message longer than 200 characters
- **WHEN** the same change row is rendered at two widths that both leave at least 10 preview columns
- **THEN** each preview remains one display line
- **AND** the wider row displays more retained message content than the narrower row
- **AND** ellipsis appears only where that row's actual remaining width cannot contain the retained preview

#### Scenario: Narrow change row never wraps its preview

- **GIVEN** a retained log or error preview does not fit the remaining change-row width
- **WHEN** the TUI renders the Changes list
- **THEN** the preview is truncated to the remaining display width with an ellipsis
- **AND** no continuation line is created
- **AND** the following change or project header retains its expected visual row position

#### Scenario: Unicode preview truncation is width-safe

- **GIVEN** the retained preview contains CJK and emoji
- **AND** the available preview width cannot contain the full value
- **WHEN** the TUI renders the Changes list
- **THEN** the preview remains one line within the available display width
- **AND** truncation does not split a UTF-8 character or panic
