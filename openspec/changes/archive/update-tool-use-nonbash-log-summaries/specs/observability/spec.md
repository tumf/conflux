## MODIFIED Requirements

### Requirement: Stream-JSON Textify Emits Tool Event Summaries

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

このとき、ツール関連の非テキストイベントについては、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。
代わりに、`tool_use` / `tool_result` については 1行の要約を表示しなければならない (MUST)。

要約は「できるだけ情報を出す」方針とし、イベントに含まれる `name` や `input` / `result` から主要フィールドを抽出して含めなければならない (MUST)。
ただし、ログの肥大化を避けるため、長文の値や巨大な結果は省略(truncate)されなければならない (MUST)。

さらに、`tool_use` の要約整形はツール名の大文字小文字の差異で無効化されてはならない (MUST NOT)。
オーケストレーターは、主要な non-Bash ツールについて、操作対象や意図が分かるツール固有の主要フィールドを優先して要約へ含めなければならない (MUST)。

ファイル操作系または本文を伴うツールについては以下を満たさなければならない (MUST)：

- `name` が `read` / `write` / `edit` の `tool_use` イベントは、要約に対象ファイルパスを含めなければならない (MUST)。
  - 可能なら `input.filePath` を優先し、存在しない場合は一般的な同等フィールド(例: `input.path`)も用いてよい (MAY)。
- `name` が `write` / `edit` の `tool_use` イベントは、入力本文(例: `input.text`, `input.content`, `input.old_string`, `input.new_string`)の生内容を要約に含めてはならない (MUST NOT)。
- `write` / `edit` の要約は、本文の代わりに安全なメタ情報(例: 文字数/行数)を含めてもよい (MAY)。

専用の要約ルールを持たない未知のツールであっても、オーケストレーターは、入力から安全な短いスカラ値を上限付きで抽出した 1行要約を表示しなければならない (MUST)。

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

#### Scenario: Non-Bash tools emit actionable summaries

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` が `grep` / `glob` / `todowrite` / `webfetch` のいずれかである
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 表示される 1行サマリには、ツールごとの主要フィールド(例: `pattern`, `path`, `url`, `format`, `todo` 件数)が含まれる
- **AND** 操作対象または意図が raw JSON を開かずに判別できる

#### Scenario: Tool-specific formatting ignores name casing differences

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` が `Edit` や `TodoWrite` のように mixed case を含む
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 対応する dedicated summary ルールが適用される
- **AND** lower-case 名の同種ツールと同等の主要フィールドが表示される

#### Scenario: Read/Write/Edit の tool_use はファイルパスを含み、Write/Edit は本文を漏らさない

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` が `read` / `write` / `edit` のいずれかである
- **AND** `input` にファイルパス(例: `filePath`)が含まれる
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** tool_use の 1行サマリにファイルパスが含まれる
- **AND** `name` が `write` または `edit` の場合、サマリに入力本文(例: `input.text`)の内容が含まれない
- **AND** 必要に応じて本文の代替として安全なメタ情報が含まれる

#### Scenario: Unknown tools still emit bounded useful summaries

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout が stream-json の `tool_use` イベントを出力し、`name` に dedicated summary ルールが存在しない
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** 生 JSON 行は表示されない
- **AND** 1行サマリには安全な短いスカラ入力が上限付きで含まれる
- **AND** 巨大または本文系の値は省略または除外される

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
