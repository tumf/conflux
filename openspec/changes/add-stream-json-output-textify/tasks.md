## 1. Implementation

- [ ] 1.1 Add stream-json textification utility (verification: unit tests cover supported event shapes and line splitting)
- [ ] 1.2 Integrate textification into `src/ai_command_runner.rs` stdout streaming (verification: existing streaming tests + new tests for line-oriented emission)
- [ ] 1.3 Add configuration toggle (default enabled) to disable stream-json textification (verification: config parsing test and behavior test)
- [ ] 1.4 (Optional) Apply the same textification in legacy streaming paths in `src/agent/runner.rs` for consistency (verification: targeted unit test)
- [ ] 1.5 Add/adjust logging tests to ensure multi-line assistant content is emitted as separate log lines (verification: test asserts output lines)

## 2. Validation

- [ ] 2.1 Run `openspec validate add-stream-json-output-textify --strict --no-interactive` (verification: passes)

## Future Work

- Consider supporting additional Claude stream-json event types (tool-use deltas, etc.) behind a debug flag to avoid log noise.
