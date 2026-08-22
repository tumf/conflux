## Implementation Tasks

- [ ] Define and parse the versioned repository-relative evidence envelope with exact identity and digest fields, rejecting malformed or partial records (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Capture successful Apply verification evidence only after the command exits, artifacts are digested, and index/worktree cleanliness is proven (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Validate every binding against current Git and proposal state and reuse only exact successful matches; otherwise rerun with an actionable reason (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Add cheap-command rerun policy, per-verification observability, adversarial mismatch tests, and operator documentation (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)

## Future Work

Evidence exchange with repository CI requires a separate trust model and is not part of local Apply-to-Acceptance reuse.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate reuse-bound-verification-evidence --archive-gate`.
