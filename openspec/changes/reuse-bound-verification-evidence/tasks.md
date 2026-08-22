## Implementation Tasks

- [x] Define and parse the versioned Git-excluded repository-relative evidence sidecar with exact identity and digest fields, rejecting malformed, partial, or agent-authored records (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [x] Implement the Conflux-runtime-owned verification executor with injected Git/process adapters for tests; capture evidence only after directly supervised success, artifact hashing, unchanged pre/post bindings, and evidence-path-excluded cleanliness (verification: integration - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [x] Validate every binding against current Git and proposal state and reuse only exact successful matches; otherwise rerun with an actionable reason (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)
- [x] Add a repository-tracked minimum-duration rerun policy, per-verification observability, adversarial mismatch and self-reference tests, and operator documentation (verification: unit - `cargo test orchestration::acceptance --lib`; verification-id: bound-evidence-tests)

## Future Work

Evidence exchange with repository CI requires a separate trust model and is not part of local Apply-to-Acceptance reuse.

## Notes

- Implementation lives in `src/orchestration/acceptance/verification_evidence.rs`, a submodule of the declared automation file `src/orchestration/acceptance.rs`, so the declared command `cargo test orchestration::acceptance --lib` actually executes its tests.
- Request path: `cflx openspec verify <change-id> [--verification-id ID] [--plan] [--json]` (`src/openspec_cmd/verify.rs`). The runtime — not the calling agent — supervises the declared argv and writes the envelope.
- Reuse path: `src/parallel/executor.rs` computes the per-verification plan before each acceptance invocation, logs `reused`/`rerun` plus reason per verification ID, and injects the same report as acceptance prompt context (`build_verification_reuse_context`).
- The repository-tracked reuse threshold is `DEFAULT_MIN_REUSE_SECONDS = 60`; the runtime measures elapsed duration itself.
- evidence: `cargo test orchestration::acceptance --lib` — 111 passed, 0 failed.
- evidence: `cargo test --lib` — 4079 passed, 0 failed, 17 ignored.
- evidence: `cargo clippy --all-targets` — no warnings; `cargo fmt --all` applied.
- evidence: real-adapter smoke check in a scratch repository captured an envelope, wrote the artifact, kept `git status --porcelain -u` empty (self-ignoring evidence directory), and correctly reported `below_reuse_duration_threshold` on the next plan.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate reuse-bound-verification-evidence --archive-gate`.
