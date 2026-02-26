# Change: stream-json textify 時に非テキストイベントを抑止する

## Problem / Context

Claude Code の `--output-format stream-json` は NDJSON 形式でイベントを出力します。
現在の stream-json textify は、`assistant.message.content[].type == "text"` などの「人間向けテキスト」を抽出できるイベントのみを変換し、それ以外のイベント（例: `thinking`, `tool_use`, `system`）は JSON 行をそのまま出力します。

その結果、ログが JSON で埋まり、ユーザーが読みたい「実際のテキスト出力」を見つけづらくなります。

## Proposed Solution

`stream_json_textify` が有効な場合、stdout の stream-json (NDJSON) 行について以下の挙動に変更します。

- 人間向けテキストを抽出できるイベントは、これまで通り人間向けテキストとして表示する
- JSON として parse でき、かつ stream-json イベントとして認識できるが、人間向けテキストを抽出できないイベントは stdout へ出力しない
- JSON ではない行（通常のログ/コマンド出力など）はこれまで通り出力する

## Acceptance Criteria

- `stream_json_textify=true` のとき、`thinking` / `tool_use` / `system` 等の非テキストイベントが stdout に JSON 行として表示されない
- `stream_json_textify=true` のとき、`text_delta` / `assistant` text ブロック / `result` 等のテキストイベントは人間向けテキストとして表示される
- textify 無効時 (`stream_json_textify=false`) は、stdout 行が従来通り素通しされる
- 既存の出力（非JSONの通常出力、stderr 等）の取り扱いは変えない

## Out of Scope

- 非テキストイベントの要約表示（例: `[tool_use:bash]`）
- stream-json の全イベントスキーマ対応（必要に応じて別変更で追加）
