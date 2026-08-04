## Implementation Tasks

- [x] Introduce a metadata-only first-parent commit representation and `UpstreamGit` observation for bounded recovery discovery while retaining the evidence-bearing spine API for full validation. Completion requires separate typed contracts whose documentation states ordering, bounds, and tree-evidence ownership. (verification: unit - compile-time trait implementations and focused upstream contract tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [x] Implement the native metadata-only recovery walk as one bounded `git log --first-parent` parse that returns SHA, parents, and raw message oldest-first without invoking per-commit tree reads. Completion requires Git-operation regression coverage that fails on `git ls-tree` use, incorrect ordering, malformed record handling, or ignored limits. (verification: integration - native Git fixture tests exercised by `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [x] Rewire pending-publication and unpushed-upstream-merge scans to the metadata-only observation while preserving trailer identity, merge-parent binding, remote-tracking reachability, 500-commit bounds, and refusal diagnostics. Completion requires positive, negative, contradicted-trailer, incorporated-remote, and bound-edge test cases. (verification: unit - coordinator recovery tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [x] Preserve evidence-bearing first-parent observation for enabled upstream spine validation and prove cumulative integration classification still requires archived change evidence and rejects still-active change directories. Completion requires existing spine behavior tests to remain green and explicit regression coverage against default or omitted tree evidence. (verification: unit - spine classification tests via `cargo test upstream --lib`; verification-id: upstream-recovery-tests)
- [x] Add a repository-local heavy regression test using short and 500-commit histories, gated with `#[cfg_attr(not(feature = "heavy-tests"), ignore)]` so enabling `heavy-tests` executes it. Completion requires a serialized PATH-shim observation of native Git commands, a machine-readable assertion of constant no-match recovery subprocess count, and diagnostic elapsed-time output without a hardware-dependent wall-clock pass threshold. (verification: benchmark - `cargo test upstream --lib --features heavy-tests`; verification-id: startup-performance-benchmark)
- [x] Run repository quality gates and confirm local TUI, finite run, and enabled-mode recovery/finalization calls resolve through the optimized shared scanners without changing refusal or recovery behavior. Completion requires `cargo fmt --check`, configured lint/type checks, default tests, focused upstream tests, existing restart-recovery cases in `tests/e2e_git_worktree_tests.rs`, and the heavy benchmark to pass. (verification: integration - repository-local quality gates plus `cargo test upstream --lib`; verification-id: upstream-recovery-tests)

## Future Work

- Investigate the independent first-frame increase introduced between v0.6.204 and v0.6.209 after this dominant startup regression is removed.

## Notes

Shared recovery scanners: `ensure_no_unpushed_upstream_recovery` (bare `cflx`, `cflx tui`,
finite `cflx run`) and the enabled-mode `UpstreamCoordinator::finalize` path both call
`scan_pending_publications` / `scan_unpushed_upstream_merges`, so routing those two through
`UpstreamGit::first_parent_recovery_metadata` covers every entrypoint at once.

- evidence: `cargo fmt --check` clean.
- evidence: `cargo clippy --all-targets -- -D warnings` and the same with `--features heavy-tests` clean.
- evidence: `cargo test` (default suite) 3300 lib tests plus every integration binary green, 0 failed.
- evidence: `cargo test --features heavy-tests --lib upstream -- --include-ignored` 182 passed, 0 failed.
- evidence: `cargo test --features heavy-tests --test e2e_git_worktree_tests upstream_integration` 14 passed, including `upstream_integration_e2e_restart_recovery_identifies_unpushed_merge`.
- evidence: benchmark output run in isolation — `cflx-recovery-benchmark commits_short=5 commits_deep=500 subprocesses_short=4 subprocesses_deep=4 tree_reads_short=0 tree_reads_deep=0 elapsed_ms_short=117 elapsed_ms_deep=301`. The subprocess count is flat at 4 across a 100x deeper history and no commit tree is read; elapsed time is diagnostic only and is inflated when the benchmark runs alongside the rest of the heavy suite.
- evidence: `make pre-commit` and `make audit` pass.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate eliminate-recovery-tree-scan --archive-gate`
`cflx openspec validate eliminate-recovery-tree-scan --strict` passes.
