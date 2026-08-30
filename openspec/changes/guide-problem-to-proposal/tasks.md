## Implementation Tasks

- [x] Add a problem-to-proposal policy directly after `## Scope Restrictions (Proposal-Only)` in `skills/cflx-proposal/SKILL.md`; keep investigation read-only within that scope, define the permanent change contract, and consolidate overlapping guardrail prose rather than duplicating it. (verification: integration - `cargo test --test install_skills_test`; verification-id: bundled-proposal-policy)
- [x] Add focused regression coverage in `tests/install_skills_test.rs` for policy ordering, problem-input, read-only evidence, temporary/permanent separation, `investigate and fix` rejection, permanent-transition, and no-unresolved-design-decision markers. (verification: integration - `cargo test --test install_skills_test`; verification-id: bundled-proposal-policy)

## Final Validation

Expected archive gate: `cflx openspec validate guide-problem-to-proposal --archive-gate`
