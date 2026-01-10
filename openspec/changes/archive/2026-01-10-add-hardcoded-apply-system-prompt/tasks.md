# Tasks

## 1. Implementation

- [x] 1.1 Add `APPLY_SYSTEM_PROMPT` constant to `src/agent.rs`
- [x] 1.2 Modify `run_apply_streaming()` to append system prompt after user prompt
- [x] 1.3 Modify `run_apply()` to append system prompt after user prompt
- [x] 1.4 Update `apply_prompt` default to empty string in `src/templates.rs` (all 3 templates)

## 2. Testing

- [x] 2.1 Add unit test for prompt construction order
- [x] 2.2 Verify existing tests pass with `cargo test`
