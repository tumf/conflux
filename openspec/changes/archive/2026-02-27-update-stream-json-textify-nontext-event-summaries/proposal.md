# Change: stream-json textify で非テキスト(ツール)イベントを要約表示する

## Problem / Context

Claude Code の `--output-format stream-json` は NDJSON 形式でイベントを出力します。

現在の `stream_json_textify` 実装は、テキストを抽出できない stream-json イベント (`tool_use`, `tool_result`, `thinking`, `system` など) を stdout に表示しないことで、ログの JSON ノイズを大きく削減しています。

しかし、この抑止が強すぎるため、特にツール実行(`tool_use`)が可視化されず、
「いま何をしているのか」「どのツールを呼んだのか」「何に対して実行したのか」が分からず、運用上のデバッグ/安心感が落ちています。

## Proposed Solution

`stream_json_textify=true` のとき、非テキストイベントの生 JSON 行は引き続き表示しない一方で、
ツール関連イベントについては **1行サマリ** を表示します。

対象は以下を最小スコープとします:

- `tool_use` (top-level event または `assistant.message.content[]` 内のブロック)
- `tool_result` (top-level event)

要約表示は「できるだけ情報を出す」方針(B)とし、`input`/`result` オブジェクトから主要フィールドを抽出して表示します。
ただし、ログの肥大化を防ぐため、値の長さや行数は制限し、必要に応じて省略(truncate)します。

`thinking` / `system` 等の非ツールイベントは従来通り抑止します。

## Acceptance Criteria

- `stream_json_textify=true` のとき
  - `tool_use` が stdout に「生 JSON 行」としては表示されない
  - 代わりに、`[tool_use:<name>] <key=value...>` のような 1行サマリが表示される
  - サマリは `input` から可能な限り主要フィールドを抽出する(B)
  - `tool_result` も同様に 1行サマリが表示され、巨大な内容は抑制される
- `stream_json_textify=true` のとき、`thinking` / `system` などツール以外の非テキストイベントは引き続き表示されない
- `stream_json_textify=false` のときは従来通り stdout 行が素通しされる (debug/troubleshooting の逃げ道)
- 既存の「テキスト抽出可能イベント」(text_delta / assistant text / result text) の表示は変えない

## Out of Scope

- 全 stream-json イベントの完全サマリ対応
- サマリ表示の詳細なカスタマイズ(verbosityレベルや出力形式の設定追加)
- stderr の扱い変更
