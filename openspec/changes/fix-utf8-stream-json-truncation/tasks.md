# Tasks

## Implementation

1. [ ] Replace `src/stream_json_textifier.rs` `truncate_string()` byte slicing with UTF-8-safe truncation that preserves current bounded summary behavior (verification: unit - `src/stream_json_textifier.rs` no longer uses `&s[..max_len]` inside `truncate_string()`)

2. [ ] Add regression coverage for assistant tool summaries that truncate multi-byte UTF-8 values in summary fields such as `filePath`, `pattern`, or `url` (verification: unit - add/update `src/stream_json_textifier.rs` tests, then run `cargo test stream_json_textifier`)

3. [ ] Add regression coverage for tool-result summary truncation with multi-byte UTF-8 content (verification: unit - `cargo test stream_json_textifier` passes with assertions covering truncated tool-result output)

4. [ ] Update validation spec coverage for UTF-8-safe stream-json summary truncation (verification: unit - `openspec/changes/fix-utf8-stream-json-truncation/specs/cflx-proposal-validation/spec.md` captures the behavior and `cflx openspec validate fix-utf8-stream-json-truncation --strict` passes)
