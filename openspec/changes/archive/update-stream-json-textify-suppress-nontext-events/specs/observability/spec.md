## ADDED Requirements

### Requirement: Stream-JSON Textify Filters Non-Text Events

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

このとき、stdout の各行が JSON として parse でき、かつ stream-json イベントとして認識できるが、人間向けテキストを抽出できないイベント（例: `thinking`, `tool_use`, `system`）については、その JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。

JSON ではない行（通常のコマンド出力など）は、従来通りそのまま表示しなければならない (MUST)。

#### Scenario: 非テキストイベントは stdout に表示されない

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `assistant` イベントを出力し、`message.content[]` に `thinking` ブロックのみを含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** `thinking` を含む JSON 行は表示されない

#### Scenario: テキストイベントは人間向けテキストとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `stream_event` を出力し、`delta.type = "text_delta"` を含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** JSON 行ではなく、抽出されたテキストが表示される

#### Scenario: textify 無効時は JSON 行が素通しされる

- **GIVEN** `stream_json_textify=false` である
- **AND** 子プロセスの stdout が stream-json の JSON 行を出力する
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の JSON 行は変換されず、そのまま表示される
