## Implementation Tasks

- [ ] Add structured completion-gate metadata to proposal frontmatter parsing with fixed phase/execution-class values, completion-blocking semantics, duplicate-ID detection, and backward-compatible absence handling. Completion requires malformed declarations to fail strict parsing without changing existing dependency parsing behavior. (verification: unit - extend `src/openspec.rs` parser tests and run `cargo test openspec --lib`)
- [ ] Extend native OpenSpec strict validation to inspect active dependency targets' structured completion gates and reject edges to completion-blocking `post-integration` or non-local gates. Completion requires actionable diagnostics naming the dependent change, target, gate ID, and split/remove-dependency remedy while performing no network access or prose inference. (verification: unit - extend `src/openspec_cmd.rs` and `src/openspec_cmd/validation.rs` tests and run `cargo test openspec_cmd --lib`)
- [ ] Add regression fixtures for valid local implementation dependencies, deployed-service and physical-device blockers, malformed and duplicate gate declarations, targets without gate metadata, and downstream graph impact. Completion requires seeded hazardous graphs to fail and ordinary local dependency graphs to pass. (verification: integration - repository fixtures under `tests/` or native validator test fixtures run by `cargo test openspec_cmd --lib`)
- [ ] Update `skills/cflx-proposal/SKILL.md` to define dependency eligibility, forbid release sequencing through hard dependencies, require downstream impact review, and prescribe implementation/release-acceptance proposal splitting. Completion requires a repository test to fail if the bundled guidance omits these rules. (verification: unit - add or extend skill-content assertions and run `cargo test --lib`)

## Future Work

- Existing downstream repositories may adopt structured completion-gate metadata incrementally; absence remains backward-compatible.
- Automated proposal rewriting remains a separate migration tool if operational demand appears.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate. Expected archive gate: `cflx openspec validate prevent-release-gate-dependency-blocking --archive-gate`.
