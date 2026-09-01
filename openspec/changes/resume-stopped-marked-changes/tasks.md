## Implementation Tasks

- [ ] Add a production-equivalent RED regression for process mode `Stopped` with a preserved ordinary execution mark, `queue_intent=not_queued`, and target status `stopped`; exercise both TUI F5 and remote Start and require one fresh dependency-analysis edge. (verification: integration - `cargo test --locked stopped_marked_resume`; verification-id: stopped-marked-resume-regression)
- [ ] Implement the shared fail-atomic Start resume transition that clears only stop-owned terminal residue, preserves marks, admits eligible targets through ordinary queue semantics, and starts one scheduler boundary without owner restart. (verification: integration - `cargo test --locked stopped_marked_resume`; verification-id: stopped-marked-resume-regression)
- [ ] Cover mixed terminal evidence, worktree ineligibility, preparation failure, mark-only no-op behavior, adapter parity, and unchanged Error/Running settlement routes. (verification: integration - `cargo test --locked stopped_marked_resume`; verification-id: stopped-marked-resume-regression)

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate resume-stopped-marked-changes --archive-gate`.
