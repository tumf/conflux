## Implementation Tasks

- [x] **Task 1: Add optional append prompt fields to `OrchestratorConfig`** (`src/config/types.rs`) (verification: unit - add config tests under `src/config/mod.rs` and run `cargo test config::` to prove deserialization, serialization where covered, merge precedence, and getters for `apply_append_prompt`, `acceptance_append_prompt`, `archive_append_prompt`, `analyze_append_prompt`, and `resolve_append_prompt`.)

- [x] **Task 2: Add a shared append helper with exact semantics** (`src/agent/prompt.rs` or a config/prompt utility module) (verification: unit - add tests for the helper and run `cargo test append_optional_prompt` to prove unset, empty, and whitespace-only values are no-ops; non-blank values are appended as a final section; placeholders such as `{change_id}` remain raw text.)

- [x] **Task 3: Wire apply, archive, and acceptance append prompts at their real prompt construction paths** (`src/execution/apply.rs`, `src/parallel/executor.rs`, `src/agent/prompt.rs`) (verification: integration - add tests in existing modules and run `cargo test apply_append_prompt acceptance_append_prompt archive_append_prompt` to prove each operation receives only its matching append prompt after built-in prompt content.)

- [x] **Task 4: Locate and wire analyze and resolve append prompts at their actual caller sites** (`src/parallel/`, `src/server/api/git_sync/resolve_command.rs`, and any existing analyze/resolve command modules) (verification: integration - add tests named `analyze_append_prompt` and `resolve_append_prompt`, then run `cargo test analyze_append_prompt resolve_append_prompt` to prove each path appends only the matching prompt and preserves existing command template expansion behavior.)

- [x] **Task 5: Prove append prompts do not change workflow control semantics** (`src/parallel/` and relevant parser tests) (verification: integration - run targeted tests that combine `acceptance_append_prompt` with acceptance output parsing and prove PASS/CONTINUE/FAIL marker parsing still depends only on command output, not append text; use `cargo test acceptance_append_prompt` with parser assertions.)

- [x] **Task 6: Update `cflx init` templates** (`src/templates.rs` or equivalent template module) (verification: unit - add template generation assertions and run `cargo test templates` to prove default, `claude`, `opencode`, and `codex` templates contain commented examples for all five append prompt fields while leaving them inactive by default.)

## Future Work

- `*_prepend_prompt` fields for users who need guidance before Conflux's built-in contract.
- Placeholder expansion inside append prompt values after operation-specific placeholder semantics are designed.
- Built-in tool auto-detection that emits a tailored append prompt only when `ocr` or similar tools are installed.
- `hook.command` injection (hooks are intentionally separate because they execute raw shell).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-optional-append-prompt --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] Behavior bug: analyze append prompts are appended twice on the LLM selection path. Fixed by making `src/orchestration/selection.rs` pass the generated analysis prompt to `AgentRunner` without pre-appending; the append now happens once in the real analyze command expansion path in `src/agent/runner.rs`. (verification: integration - `agent-exec run -- cargo test analyze_append_prompt` passed in job `25e323a8283cfaf9b22a67d7b926cebc`.)
- [x] Checklist/evidence mismatch: added real command construction coverage for analyze append prompts in `src/agent/runner.rs` and resolve append prompts in `src/parallel/conflict.rs`, proving tail placement, raw placeholder preservation, and single append behavior through `{prompt}` command expansion. (verification: integration - `agent-exec run -- cargo test analyze_append_prompt` passed in job `25e323a8283cfaf9b22a67d7b926cebc`; `agent-exec run -- cargo test resolve_append_prompt` passed in job `69281945cc53d779eb1bdac26a4f2300`.)
- [x] Verification run evidence: `agent-exec run -- cargo test analyze_append_prompt` passed (job `25e323a8283cfaf9b22a67d7b926cebc`), and `agent-exec run -- cargo test resolve_append_prompt` passed (job `69281945cc53d779eb1bdac26a4f2300`). An initial combined cargo invocation failed because Cargo accepts only one test-name positional filter; it was replaced with the two valid targeted runs above.

## Acceptance #2 Failure Follow-up
- [x] Archive commit-path blocker: the real pre-commit hook at `/Users/tumf/work/conflux/.git/hooks/pre-commit` runs `prek hook-impl`, and `.pre-commit-config.yaml:29-34` includes `cargo clippy --locked --all-targets --all-features -- -D warnings`. Fixed by moving the newly added `src/agent/runner.rs` test module after all non-test items so clippy no longer reports `items after a test module`. (verification: integration - `agent-exec run -- cargo fmt --all` passed in job `1516c472b2c4bf96f0bdf5978d3fcfbb`; `agent-exec run -- prek run --files openspec/changes/add-optional-append-prompt/tasks.md src/agent/runner.rs src/orchestration/selection.rs src/parallel/conflict.rs` passed in job `beee2d3b10e81f53da35106bbe0e9bcd`.)
- [x] Previous behavior bug remains addressed: `src/orchestration/selection.rs:89-96` passes the generated analysis prompt without pre-appending, while `src/agent/runner.rs:54-60` appends exactly once inside `expand_analyze_command_with_append`, and all analyze entrypoints call that helper (`src/agent/runner.rs:1109-1114`, `1152-1157`, `1212-1217`). Targeted tests passed after the clippy fix. (verification: integration - `agent-exec run -- cargo test analyze_append_prompt` passed in job `7929b426676db19418f599ac7d2b38e8`; `agent-exec run -- cargo test resolve_append_prompt` passed in job `789b87ec5961efe90f123a39c1ebed34`.)
- [x] Tasks check: active `tasks.md` items are complete after this follow-up, and the commit-path clippy blocker is resolved. (verification: integration - `agent-exec run -- prek run --files openspec/changes/add-optional-append-prompt/tasks.md src/agent/runner.rs src/orchestration/selection.rs src/parallel/conflict.rs` passed in job `beee2d3b10e81f53da35106bbe0e9bcd`; final validation re-run below.)
