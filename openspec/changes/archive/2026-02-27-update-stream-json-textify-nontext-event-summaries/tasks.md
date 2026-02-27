## 1. Implementation

- [x] 1.1 `src/stream_json_textifier.rs` に `tool_use` / `tool_result` を 1行サマリへ変換する抽出ロジックを追加する（verification: unit tests cover both top-level events and assistant content blocks）
- [x] 1.2 主要フィールド抽出(B)の方針を実装する（例: `name`, `id/tool_use_id`, `input.command/url/path/query/selector/text` 等を優先し、長文は truncate）（verification: unit tests assert extracted summary includes expected keys and truncates long values）
- [x] 1.3 `stream_json_textify=true` 時に生 JSON を出さないことを維持しつつ、サマリが stdout へ出ることを確認する（verification: unit tests for `process_stdout_line` with tool events; existing suppression tests remain valid）
- [x] 1.4 `stream_json_textify=false` で JSON 行が素通しされることを回帰テストで確認する（verification: unit test asserts raw JSON line is forwarded unchanged when disabled in runner path）

## 2. Validation

- [x] 2.1 `openspec validate update-stream-json-textify-nontext-event-summaries --strict --no-interactive`（verification: passes）

## Future Work

- サマリ表示の verbosity 設定（quiet / tools-only / verbose など）
- `thinking` の軽量サマリ（必要性が出た場合）
