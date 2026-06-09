## Implementation Tasks

- [ ] **Task 1: Add optional append prompt fields to `OrchestratorConfig`** (`src/config/types.rs`) (verification: unit - `cargo test --lib config` in `src/config/mod.rs` checks deserialize/serialize, merge behavior, and getters for `apply_append_prompt`, `acceptance_append_prompt`, `archive_append_prompt`, `analyze_append_prompt`, and `resolve_append_prompt`.)

- [ ] **Task 2: Wire append prompt injection into operation prompt builders** (`src/agent/prompt.rs` and caller sites as needed) (verification: unit - `cargo test --lib agent` in `src/agent/prompt.rs` proves configured append text is appended after built-in Conflux prompt content for apply, acceptance, archive, analyze, and resolve; missing or empty values produce unchanged prompts.)

- [ ] **Task 3: Add command-path coverage for real append prompt assembly** (`src/parallel/`, `src/execution/`, or `tests/`) (verification: integration - `cargo test --lib parallel -- test_acceptance_append_prompt` in `src/parallel/tests/executor.rs` constructs config with `acceptance_append_prompt = "OCR evidence only"` and proves the actual prepared command prompt includes that text; apply receives equivalent coverage if it uses a separate assembly path.)

- [ ] **Task 4: Update `cflx init` templates** (`src/templates.rs` or equivalent template module) (verification: unit - template generation tests prove the default, `claude`, `opencode`, and `codex` templates contain commented examples for all five append prompt fields while leaving them inactive by default.)

## Future Work

- `*_prepend_prompt` fields for users who need guidance before Conflux's built-in contract.
- Built-in tool auto-detection that emits a tailored append prompt only when `ocr` or similar tools are installed.
- `hook.command` injection (hooks are intentionally separate because they execute raw shell).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-optional-append-prompt --archive-gate`
