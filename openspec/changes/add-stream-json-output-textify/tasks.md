## 1. Implementation

- [x] 1.1 Add stream-json textification utility (verification: unit tests cover supported event shapes and line splitting)
- [x] 1.2 Integrate textification into `src/ai_command_runner.rs` stdout streaming (verification: existing streaming tests + new tests for line-oriented emission)
- [x] 1.3 Add configuration toggle (default enabled) to disable stream-json textification (verification: config parsing test and behavior test)
- [x] 1.4 (Optional) Apply the same textification in legacy streaming paths in `src/agent/runner.rs` for consistency (verification: targeted unit test)
- [x] 1.5 Add/adjust logging tests to ensure multi-line assistant content is emitted as separate log lines (verification: test asserts output lines)

## 2. Validation

- [x] 2.1 Run `openspec validate add-stream-json-output-textify --strict --no-interactive` (verification: passes)

## Future Work

- Consider supporting additional Claude stream-json event types (tool-use deltas, etc.) behind a debug flag to avoid log noise.
