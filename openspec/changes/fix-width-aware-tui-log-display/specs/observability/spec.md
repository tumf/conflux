## MODIFIED Requirements

### Requirement: Stream-JSON Textify Emits Tool Event Summaries

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

このとき、ツール関連の非テキストイベントについては、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。
代わりに、`tool_use` / `tool_result` については 1 件の意味的な要約を表示しなければならない (MUST)。

要約は「できるだけ情報を出す」方針とし、イベントに含まれる `name` や `input` / `result` から主要フィールドを抽出して含めなければならない (MUST)。`tool_result` の結果本文は、TUI の実際の表示幅より前に 200 文字等の固定表示長で省略してはならず (MUST NOT)、共有の operator-facing log safety bound までは保持されなければならない (MUST)。共有安全上限を超える巨大な結果は UTF-8 境界を壊さずに省略され、省略量を示す明示的な marker を含まなければならない (MUST)。既存の tool-use body redaction と raw JSON suppression は維持されなければならない (MUST)。

#### Scenario: tool_use が 1行サマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` と `input` を含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の生 JSON 行は表示されない
- **AND** `[tool_use:<name>]` で始まる 1 件のサマリが表示される
- **AND** サマリには redaction policy で許可された `input` の主要フィールドが含まれる

#### Scenario: assistant message 内の tool_use ブロックもサマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `assistant` イベントを出力し、`message.content[]` に `tool_use` ブロックを含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** tool_use ブロックは生 JSON として表示されない
- **AND** tool_use の 1 件のサマリが表示される

#### Scenario: 200文字を超えるtool_resultは表示前に失われない

- **GIVEN** `stream_json_textify=true` である
- **AND** `tool_result` の結果本文が200文字を超え、共有の operator-facing log safety bound 未満である
- **WHEN** オーケストレーターがイベントを textify して `LogEntry` に渡す
- **THEN** `[tool_result:<name>]` で始まる要約が表示される
- **AND** 結果本文は200文字地点で `...` に置き換えられない
- **AND** TUI renderer が利用可能幅に応じた表示量を決定できる

#### Scenario: 巨大なtool_resultは共有安全上限で明示的に抑制される

- **GIVEN** `stream_json_textify=true` である
- **AND** `tool_result` の結果本文が共有の operator-facing log safety bound を超える
- **WHEN** 結果が operator-facing `LogEntry` になる
- **THEN** 生 JSON 行は表示されない
- **AND** メッセージは共有安全上限以内に収まる
- **AND** UTF-8 境界は壊れない
- **AND** marker が省略発生と省略量を明示する

#### Scenario: textify 無効時は JSON 行が素通しされる

- **GIVEN** `stream_json_textify=false` である
- **AND** 子プロセスの stdout が stream-json の JSON 行を出力する
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の JSON 行は変換されず、そのまま表示される
