## Implementation Tasks

- [ ] Update acceptance prompt specification to replace hardcoded quality-gate language with archive-commitability semantics (verification: manual - review `openspec/changes/refine-acceptance-archive-commitability/specs/agent-prompts/spec.md` against `openspec/specs/agent-prompts/spec.md`)
- [ ] Update parallel execution specification so archive handoff is gated by actual final-commit blockers, not inferred toolchain checks (verification: manual - review `openspec/changes/refine-acceptance-archive-commitability/specs/parallel-execution/spec.md` against `openspec/specs/parallel-execution/spec.md`)
- [ ] Define implementation follow-up for prompt-builder changes in `src/agent/prompt.rs` and template wording in `src/templates.rs` (verification: not-testable - proposal task references concrete implementation targets)
- [ ] Run strict proposal validation and fix any structural errors (verification: manual - `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate refine-acceptance-archive-commitability --strict`)

## Future Work

- Implement the runtime prompt-builder changes in `src/agent/prompt.rs`
- Update related acceptance/parallel executor tests that currently assume Rust-specific archive-readiness findings
- Reconcile template comments in `src/templates.rs` with the finalized prompt-source architecture
