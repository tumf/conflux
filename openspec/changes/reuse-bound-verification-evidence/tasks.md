## Implementation Tasks

- [ ] Define and parse the versioned Git-excluded repository-relative evidence sidecar with exact identity and digest fields, rejecting malformed, partial, or agent-authored records (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Implement the Conflux-runtime-owned verification executor with injected Git/process adapters for tests; capture evidence only after directly supervised success, artifact hashing, unchanged pre/post bindings, and evidence-path-excluded cleanliness (verification: integration - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Validate every binding against current Git and proposal state and reuse only exact successful matches; otherwise rerun with an actionable reason (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [ ] Add a repository-tracked minimum-duration rerun policy, per-verification observability, adversarial mismatch and self-reference tests, and operator documentation (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)

## Future Work

Evidence exchange with repository CI requires a separate trust model and is not part of local Apply-to-Acceptance reuse.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate reuse-bound-verification-evidence --archive-gate`.
