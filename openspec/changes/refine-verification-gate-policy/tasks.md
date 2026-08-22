## Implementation Tasks

- [ ] Survey active proposal frontmatter with the candidate exact-token matcher and record counts and per-proposal reasons in `design.md` (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [ ] Remove task-note cohesion enforcement and task-prose heavyweight scanning so structured frontmatter remains the single command authority (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [ ] Implement warning-only exact-token heavyweight detection for `evidence` and `rerun`, including explicit `docker build` and substring-safe boundaries (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)
- [ ] Update diagnostics, bundled guidance, and regression tests for migration behavior and the runtime boundary (verification: unit - `cargo test openspec_cmd --lib`; verification-id: verification-policy-tests)

## Future Work

Promote proven warning classes to errors only through a separate reviewed proposal after migration evidence exists.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate refine-verification-gate-policy --archive-gate`.
