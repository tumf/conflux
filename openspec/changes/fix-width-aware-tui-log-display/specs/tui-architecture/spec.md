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
- Logsビューは producer 側の固定表示長省略に依存してはならず、source message が200文字以上保持されている場合、狭い端末でも先頭200文字以上が折り返し後の表示行集合に保持され、既存スクロール操作で到達可能でなければならない。
- Logsビューの表示範囲は折り返し後の表示行数で計算され、長文ログの折り返しによって最新ログが画面外になることがあってはならない。
- auto-scroll が無効な場合、TUI はユーザーが閲覧しているログ範囲を維持し、表示行は新しいログ追加やログバッファのトリミングで移動してはならない。表示行がトリミングされた場合は、最も古い残存ログ行にクランプされなければならず、auto-scroll は自動的に再有効化されてはならない。
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

#### Scenario: auto-scroll が無効なとき表示範囲が固定される

- **GIVEN** ユーザーがログをスクロール済みで auto-scroll が無効になっている
- **WHEN** 新しいログが追加される（必要に応じて古いログがトリミングされる）
- **THEN** 表示範囲は同じログ行を指し続ける
- **AND** 表示範囲がトリミングされた場合、最も古い残存ログ行にクランプされる
- **AND** auto-scroll は自動的に再有効化されない

#### Scenario: 幅の広いLogsパネルは追加の内容を表示する

- **GIVEN** 200文字を超える保持済みログメッセージがある
- **WHEN** 同じログを狭いLogsパネルと広いLogsパネルで描画する
- **THEN** 各行はそれぞれの現在のパネル内側幅を使用する
- **AND** 広いパネルの1行は狭いパネルより多くのmessage contentを表示する
- **AND** 200文字等のproducer固定位置に不要なellipsisは表示されない

#### Scenario: 狭いLogsパネルは最低表示量を複数行に保持する

- **GIVEN** 200文字以上の保持済みログメッセージがある
- **AND** 1行に200文字を表示できない幅のLogsパネルである
- **WHEN** TUI がログを描画する
- **THEN** 先頭行はtimestampとheader後の利用可能幅を使用する
- **AND** 継続行はインデントせずパネル内側幅全体を使用する
- **AND** 少なくとも先頭200文字は折り返し後の表示行集合に保持され、既存スクロールで到達可能である
- **AND** 最新ログはauto-scroll表示範囲から外れない

#### Scenario: Unicodeログの折り返しは表示幅と文字境界を守る

- **GIVEN** CJKとemojiを含む長いログメッセージがある
- **WHEN** TUI が異なる幅のLogsパネルで折り返す
- **THEN** 各表示行はパネル内側のdisplay widthを超えない
- **AND** UTF-8文字境界は壊れない
- **AND** 折り返し部分を結合すると共有安全上限内の保持済みmessage contentが失われていない

### Requirement: Change List Log Preview

The TUI change list MUST display a single-line preview in the remaining space on the right side of each change row. For a change whose display status is `error`, the preview MUST prefer the retained final change-level diagnostic over every buffered log entry and MUST format it as `Error: <diagnostic>`. This error preview MUST remain available independently of bounded log retention. If the status is `error` but no diagnostic is available, the preview MUST use an explicit fallback such as `Error details unavailable` and MUST NOT present an unrelated ordinary log as the failure reason. For every non-error change, the preview MUST display the latest retained log entry and include its relative time (`just now` for less than 1 minute; `<n><unit> ago` for 1 minute or more), the shortened header format `[operation:{iteration}]` or `[operation]`, and the message.

Every preview MUST remain exactly one display line and MUST NOT wrap or increase the change-row height. The renderer MUST use all actual remaining row width and truncate with an ellipsis only when the retained preview does not fit that width. The producer MUST NOT pre-truncate a retained message at a fixed display length such as 200 characters. Truncation MUST use Unicode display width, MUST NOT break UTF-8 character boundaries, and MUST NOT panic for CJK or emoji. Error previews MUST use readable error styling in both focused and unfocused rows.

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
