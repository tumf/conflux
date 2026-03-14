## MODIFIED Requirements

### Requirement: Stream-JSON Textify Emits Tool Event Summaries

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

このとき、ツール関連の非テキストイベントについては、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。
代わりに、`tool_use` / `tool_result` については 1行の要約を表示しなければならない (MUST)。

要約は「できるだけ情報を出す」方針とし、イベントに含まれる `name` や `input` / `result` から主要フィールドを抽出して含めなければならない (MUST)。
ただし、ログの肥大化を避けるため、長文の値や巨大な結果は省略(truncate)されなければならない (MUST)。

さらに、`tool_use` の要約整形はツール名の大文字小文字の差異で無効化されてはならない (MUST NOT)。
オーケストレーターは、主要な non-Bash ツールについて、操作対象や意図が分かるツール固有の主要フィールドを優先して要約へ含めなければならない (MUST)。

`skill` ツールについては、要約に要求された skill 名を含めなければならない (MUST)。

#### Scenario: tool_use が 1行サマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` と `input` を含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の生 JSON 行は表示されない
- **AND** `[tool_use:<name>]` で始まる 1行サマリが表示される
- **AND** サマリには `input` から抽出された主要フィールドが含まれる

#### Scenario: assistant message 内の tool_use ブロックもサマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `assistant` イベントを出力し、`message.content[]` に `tool_use` ブロックを含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** tool_use ブロックは生 JSON として表示されない
- **AND** tool_use の 1行サマリが表示される

#### Scenario: Skill tool_use summary shows requested skill

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` が `skill` である
- **AND** `input.name` に要求された skill 名が含まれる
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 生 JSON 行は表示されない
- **AND** 1行サマリに要求された skill 名が含まれる
- **AND** ログ閲覧者はどの skill が読み込まれたかを raw JSON なしで判別できる

#### Scenario: Skill dedicated formatting ignores name casing differences

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` が `Skill` のように mixed case を含む
- **AND** `input.name` に要求された skill 名が含まれる
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** `skill` 用の dedicated summary ルールが適用される
- **AND** lower-case 名の同種ツールと同等に skill 名が表示される

#### Scenario: tool_result は巨大な内容を抑制したサマリとして表示される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_result` イベントを出力し、結果本文が非常に長い
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 生 JSON 行は表示されない
- **AND** `[tool_result:<name>]` で始まる 1行サマリが表示される
- **AND** 結果本文は必要に応じて省略(truncate)される

#### Scenario: textify 無効時は JSON 行が素通しされる

- **GIVEN** `stream_json_textify=false` である
- **AND** 子プロセスの stdout が stream-json の JSON 行を出力する
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の JSON 行は変換されず、そのまま表示される
