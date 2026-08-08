## MODIFIED Requirements

### Requirement: Stream-JSON Textify Emits Tool Event Summaries

`stream_json_textify` が有効な場合、オーケストレーターは Claude Code の `--output-format stream-json` による stdout (NDJSON) を人間向けに textify しなければならない (MUST)。

ツール関連の非テキストイベントについて、生 JSON 行をユーザー向けログへ表示してはならない (MUST NOT)。代わりに、`tool_use` / `tool_result` ごとに1件の意味的な要約を表示しなければならない (MUST)。`tool_result` prefix は利用可能な場合に `[tool_result:<tool_use_id>]`、IDがない場合に `[tool_result]` としなければならない (MUST)。

要約はイベントの `name`、許可された `input` scalar、`result` から主要フィールドを抽出しなければならない (MUST)。表示可能な `tool_use` scalar と `tool_result` content は、TUI/CLI表示より前に60、80、100、200文字等の固定表示長で省略してはならない (MUST NOT)。write/edit系bodyは本文を含めず安全なmetadataへ置換しなければならず (MUST)、raw JSON suppression と既存のprivacy redactionを維持しなければならない (MUST)。

prefixを含む完成後のsummary全体は、CLI/TUIへ分岐する前に共有operator-facing sanitizationと8,192-byte safety boundを論理的に一度だけ適用されなければならない (MUST)。後続の `LogEntry` constructionは既にsanitized/boundedなsummaryに対してidempotentでなければならず (MUST)、二重truncateによってmarkerを置換してはならない (MUST NOT)。上限超過時、最終messageはUTF-8境界を壊さず、完成後summary全体から実際に省略されたbyte数を示すmarkerを含まなければならない (MUST)。同じ最終summary representationを非TUI CLI出力とTUI `LogEntry`に渡さなければならない (MUST)。

#### Scenario: tool_use が1件の幅非依存サマリとして保持される

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスの stdout がstream-jsonの `tool_use` eventを出力し、表示可能なscalar fieldが従来の固定長上限を超える
- **WHEN** オーケストレーターがstdoutをtextifyする
- **THEN** stdoutの生JSON行は表示されない
- **AND** `[tool_use:<name>]` で始まる1件のsummaryが生成される
- **AND** redaction policyで許可されたscalarは60、80、100文字等の固定位置で `...` に置換されない
- **AND** write/edit body contentはsummaryに含まれない

#### Scenario: assistant message 内の tool_use ブロックも同じポリシーを使う

- **GIVEN** `stream_json_textify=true` である
- **AND** 子プロセスのstdoutがstream-jsonの `assistant` eventを出力し、`message.content[]` に `tool_use` blockを含む
- **WHEN** オーケストレーターがstdoutをtextifyする
- **THEN** tool_use blockは生JSONとして表示されない
- **AND** top-level tool_useと同じretention、redaction、sanitization、bound policyの1件のsummaryが表示される

#### Scenario: 200文字を超えるtool_resultは表示前に失われない

- **GIVEN** `stream_json_textify=true` である
- **AND** `tool_result` contentが200文字を超え、完成後summary全体が共有safety bound未満である
- **WHEN** オーケストレーターがeventをtextifyしてoperator-facing outputへ渡す
- **THEN** `[tool_result:<tool_use_id>]` で始まるsummaryが生成される
- **AND** contentは200文字地点で `...` に置き換えられない
- **AND** CLI outputとTUI `LogEntry.message`は同じ保持済みcontentを含む

#### Scenario: 巨大な完成後summaryは一度だけ正確に抑制される

- **GIVEN** `stream_json_textify=true` である
- **AND** prefixを含む完成後tool-event summaryが共有operator-facing safety boundを超える
- **WHEN** summaryがCLI/TUI consumerへ渡され、TUIでは `LogEntry` が構築される
- **THEN** 最終messageは8,192 bytes以内に収まる
- **AND** UTF-8境界は壊れない
- **AND** markerは完成後summary全体から実際に省略されたbyte数を示す
- **AND** `LogEntry` constructionはmarkerを別の二次truncate markerへ置換しない

#### Scenario: textify 無効時は JSON 行が素通しされる

- **GIVEN** `stream_json_textify=false` である
- **AND** 子プロセスの stdout が stream-json の JSON 行を出力する
- **WHEN** オーケストレーターが stdout をストリーミング表示する
- **THEN** stdout の JSON 行は変換されず、そのまま表示される
