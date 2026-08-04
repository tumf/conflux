## Implementation Tasks

- [ ] Introduce a metadata-only first-parent commit representation and `UpstreamGit` observation for bounded recovery discovery while retaining the evidence-bearing spine API for full validation. Completion requires separate typed contracts whose documentation states ordering, bounds, and tree-evidence ownership. (verification: unit - compile-time trait implementations and focused upstream contract tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [ ] Implement the native metadata-only recovery walk as one bounded `git log --first-parent` parse that returns SHA, parents, and raw message oldest-first without invoking per-commit tree reads. Completion requires Git-operation regression coverage that fails on `git ls-tree` use, incorrect ordering, malformed record handling, or ignored limits. (verification: integration - native Git fixture tests exercised by `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [ ] Rewire pending-publication and unpushed-upstream-merge scans to the metadata-only observation while preserving trailer identity, merge-parent binding, remote-tracking reachability, 500-commit bounds, and refusal diagnostics. Completion requires positive, negative, contradicted-trailer, incorporated-remote, and bound-edge test cases. (verification: unit - coordinator recovery tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [ ] Preserve evidence-bearing first-parent observation for enabled upstream spine validation and prove cumulative integration classification still requires archived change evidence and rejects still-active change directories. Completion requires existing spine behavior tests to remain green and explicit regression coverage against default or omitted tree evidence. (verification: unit - spine classification tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [ ] Add a repository-local heavy benchmark or ignored performance regression test using short and 500-commit histories. Completion requires machine-readable assertion of constant no-match recovery Git subprocess count and diagnostic elapsed-time output, without a hardware-dependent wall-clock pass threshold. (verification: benchmark - `cargo test upstream --lib --features heavy -- --ignored`; verification-id: startup-performance-benchmark)
- [ ] Run repository quality gates and confirm both local TUI and finite run resolve through the optimized shared recovery path without changing startup refusal behavior. Completion requires `cargo fmt --check`, configured lint/type checks, default tests, focused upstream tests, and the heavy benchmark to pass. (verification: integration - repository-local quality gates plus `cargo test upstream --lib`; verification-id: upstream-recovery-tests)

## Future Work

- Investigate the independent first-frame increase introduced between v0.6.204 and v0.6.209 after this dominant startup regression is removed.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate eliminate-recovery-tree-scan --archive-gate`
