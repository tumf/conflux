## Implementation Tasks

- [ ] 1. Remove duplicated fixed analyze guidance from `src/orchestration/selection.rs` so the Rust-side analyze prompt injects only variable runtime context while `skills/cflx-analyze/SKILL.md` remains the single source of selection rules and output contract (verification: unit - tests for `build_analysis_prompt` assert presence of `load skills: cflx-analyze` plus change-list context, and absence of duplicated selection-priority / output-contract text).
- [ ] 2. Remove duplicated fixed resolve guidance from all resolve prompt assembly paths in `src/parallel/conflict.rs` so Rust-side resolve prompts inject only variable runtime context while `skills/cflx-resolve/SKILL.md` remains the single source of safety rules, sequential merge protocol, and commit conventions (verification: unit - resolve prompt builder tests cover both normal conflict and sequential merge paths and assert absence of duplicated fixed guidance text in Rust prompt bodies).
- [ ] 3. Add or update tests that detect analyze / resolve prompt drift by failing when fixed guidance phrases are reintroduced into Rust prompt builders (verification: unit - targeted tests in `src/orchestration/selection.rs` and `src/parallel/conflict.rs` fail on duplicated fixed-guidance strings and pass with variable-context-only prompts).
- [ ] 4. Update canonical spec and any skill-facing docs needed so the source-of-truth boundary is explicit: dedicated skills own fixed rules, Rust prompt builders own runtime context injection (verification: integration - `cflx openspec validate fix-analyze-resolve-guidance-source --strict` passes with aligned spec deltas).
- [ ] 5. Run focused repository checks for the touched modules before handoff (verification: unit/integration - targeted Rust tests for `selection` / `conflict` / prompt-related modules pass, along with any affected install or prompt tests).

## Future Work

- Add reusable prompt-construction helper abstractions if similar fixed-guidance drift appears in other operations
