## Implementation Tasks

- [ ] Add an early problem-to-proposal policy to `skills/cflx-proposal/SKILL.md` that requires evidence-backed investigation before proposal drafting and defines the permanent change contract produced from that investigation. (verification: integration - `cargo test --test install_skills_test`; verification-id: bundled-proposal-policy)
- [ ] Add focused regression coverage in `tests/install_skills_test.rs` for the policy's problem-input, evidence, permanent-transition, and no-unresolved-design-decision markers. (verification: integration - `cargo test --test install_skills_test`; verification-id: bundled-proposal-policy)

## Final Validation

Expected archive gate: `cflx openspec validate guide-problem-to-proposal --archive-gate`
