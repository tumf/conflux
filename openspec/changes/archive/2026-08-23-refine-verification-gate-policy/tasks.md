## Implementation Tasks

- [x] Survey active proposal frontmatter with the candidate exact-token matcher and record counts and per-proposal reasons in `design.md` (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [x] Remove task-note cohesion enforcement and task-prose heavyweight scanning so structured frontmatter remains the single command authority (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [x] Implement warning-only exact-token heavyweight detection for `evidence` and `rerun`, including explicit `docker build` and substring-safe boundaries (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [x] Update diagnostics, bundled guidance, and regression tests for migration behavior and the runtime boundary (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)

## Future Work

Promote proven warning classes to errors only through a separate reviewed proposal after migration evidence exists. `design.md` records the precondition: the `--all-features` token is wrong about 12 of the 15 archived declarations it matches, so promotion must first narrow it to a test-command context or drop it.

## Notes

- Survey method and results are in `design.md` under `## Active-proposal survey`. Both were produced by the shipped matcher through `cflx openspec validate --strict`, not by a re-implementation: 0 warnings across the 2 active proposals (3 change-blocking declarations), and 15 warned / 82 clean across the 97 archived change-blocking proposals.
- Native validation now inspects only `verifications[].evidence` and `verifications[].rerun`. `evaluate_verification_cohesion`, `parse_verification_note`, `note_declares_heavy_command`, and the `TaskVerificationReference` cohesion key were removed from `src/openspec_cmd/validation.rs`; `heavy_declaration_findings` became `heavy_declaration_warnings` and its output is pushed to the warning channel, so strict and archive-gate validation both stay passing.
- The change's spec delta gained a `## REMOVED Requirements` block retiring `Change-blocking verification declarations remain cohesive and bounded`, because the modified requirement supersedes it; `Non-local verification cannot be hidden in task prose` was restored to the modified requirement's scenarios, and `Heavy repository gate is not an Apply checkbox` is declared under `## Retired Scenarios` in `proposal.md`.
- evidence: `cargo test openspec_cmd --lib` — 148 passed, 1 failed. The single failure is `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario`, and it is **not** about this change: it reports `correct-acceptance-runtime-routing/parallel-execution` dropping the canonical scenario `Acceptance command recovers without rerunning Apply`. That is a different pending proposal (added by commit `d1345b28` on `main`, never applied), and its delta is owned by its own Apply. The same test failed on this branch before any edit here, then naming this change's own two dropped scenarios; those are now declared, so this change's delta is clean under the guard.
- evidence: `cargo test embedded_skills --lib` — 38 passed, 0 failed, including `proposal_skill_documents_declared_command_authority_and_migration_warnings`.
- evidence: `cargo fmt --all -- --check` clean.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate refine-verification-gate-policy --archive-gate`.
