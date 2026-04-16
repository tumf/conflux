## Implementation Tasks

- [x] Update acceptance prompt specification to replace hardcoded quality-gate language with archive-commitability semantics (verification: manual - review `openspec/changes/refine-acceptance-archive-commitability/specs/agent-prompts/spec.md` against `openspec/specs/agent-prompts/spec.md`)
- [x] Update parallel execution specification so archive handoff is gated by actual final-commit blockers, not inferred toolchain checks (verification: manual - review `openspec/changes/refine-acceptance-archive-commitability/specs/parallel-execution/spec.md` against `openspec/specs/parallel-execution/spec.md`)
- [x] Define implementation follow-up for prompt-builder changes in `src/agent/prompt.rs` and template wording in `src/templates.rs` (verification: not-testable - proposal task references concrete implementation targets)
- [x] Run strict proposal validation and fix any structural errors (verification: manual - `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate refine-acceptance-archive-commitability --strict`)

## Future Work

- Remove or replace `ARCHIVE_READINESS_CONTEXT` constant in `src/agent/prompt.rs:146-158` — it hardcodes `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and pre-commit hook assumptions; replace with commitability-only language that does not assume specific toolchain gates
- Remove `parts.push(ARCHIVE_READINESS_CONTEXT.to_string())` call at `src/agent/prompt.rs:189` or replace with a commitability context block that references only the repository's actual commit path
- Update test assertions in `src/agent/prompt.rs:388-419` that expect `<archive_readiness_context>` to match the new block name and content
- Update serial-mode test helpers in `src/serial_run_service.rs:870-901` (`test_process_acceptance_result_archive_readiness_fail_blocks_archive_progression` and `..._pass_allows_archive_progression`) to not assume Rust-specific findings
- Remove or update "hardcoded acceptance prompt" comments in `src/templates.rs:33,107,181` to reflect the context-only prompt architecture
