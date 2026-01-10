# Tasks

## 1. Implementation

- [ ] 1.1 Add `APPLY_SYSTEM_PROMPT` constant to `src/agent.rs`
- [ ] 1.2 Modify `run_apply_streaming()` to append system prompt after user prompt
- [ ] 1.3 Modify `run_apply()` to append system prompt after user prompt
- [ ] 1.4 Update `apply_prompt` default to empty string in `src/templates.rs` (all 3 templates)

## 2. Testing

- [ ] 2.1 Add unit test for prompt construction order
- [ ] 2.2 Verify existing tests pass with `cargo test`

## 3. Documentation

- [ ] 3.1 Update README.md and README.ja.md to document the prompt structure
