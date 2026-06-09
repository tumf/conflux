## Implementation Tasks

- [ ] **Task 1: Add optional append prompt fields to `OrchestratorConfig`** (`src/config/types.rs`) (verification: unit - add config tests under `src/config/mod.rs` and run `cargo test config::` to prove deserialization, serialization where covered, merge precedence, and getters for `apply_append_prompt`, `acceptance_append_prompt`, `archive_append_prompt`, `analyze_append_prompt`, and `resolve_append_prompt`.)

- [ ] **Task 2: Add a shared append helper with exact semantics** (`src/agent/prompt.rs` or a config/prompt utility module) (verification: unit - add tests for the helper and run `cargo test append_optional_prompt` to prove unset, empty, and whitespace-only values are no-ops; non-blank values are appended as a final section; placeholders such as `{change_id}` remain raw text.)

- [ ] **Task 3: Wire apply, archive, and acceptance append prompts at their real prompt construction paths** (`src/execution/apply.rs`, `src/parallel/executor.rs`, `src/agent/prompt.rs`) (verification: integration - add tests in existing modules and run `cargo test apply_append_prompt acceptance_append_prompt archive_append_prompt` to prove each operation receives only its matching append prompt after built-in prompt content.)

- [ ] **Task 4: Locate and wire analyze and resolve append prompts at their actual caller sites** (`src/parallel/`, `src/server/api/git_sync/resolve_command.rs`, and any existing analyze/resolve command modules) (verification: integration - add tests named `analyze_append_prompt` and `resolve_append_prompt`, then run `cargo test analyze_append_prompt resolve_append_prompt` to prove each path appends only the matching prompt and preserves existing command template expansion behavior.)

- [ ] **Task 5: Prove append prompts do not change workflow control semantics** (`src/parallel/` and relevant parser tests) (verification: integration - run targeted tests that combine `acceptance_append_prompt` with acceptance output parsing and prove PASS/CONTINUE/FAIL marker parsing still depends only on command output, not append text; use `cargo test acceptance_append_prompt` with parser assertions.)

- [ ] **Task 6: Update `cflx init` templates** (`src/templates.rs` or equivalent template module) (verification: unit - add template generation assertions and run `cargo test templates` to prove default, `claude`, `opencode`, and `codex` templates contain commented examples for all five append prompt fields while leaving them inactive by default.)

## Future Work

- `*_prepend_prompt` fields for users who need guidance before Conflux's built-in contract.
- Placeholder expansion inside append prompt values after operation-specific placeholder semantics are designed.
- Built-in tool auto-detection that emits a tailored append prompt only when `ocr` or similar tools are installed.
- `hook.command` injection (hooks are intentionally separate because they execute raw shell).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-optional-append-prompt --archive-gate`
