## Implementation Tasks

- [ ] Remove `ChangeArchived` and archive-only evidence from mark revocation while preserving the existing target-scoped Error, Rejected, refresh-ineligible, dequeue, and merge-hook-recovery revocations in `src/orchestration/mark_reconciliation.rs`. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib run_mark_intent_archive -- --nocapture`)
- [ ] Replace the reducer reconciliation archive-clear regression with a deterministic two-mark archive-preservation regression, run it before implementation to record RED, then after implementation to record GREEN. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib run_mark_intent_archive -- --nocapture`)
- [ ] Update the API event revision regression so the `change_archived` revision retains both target and unrelated marks, and later merge/push projections continue to retain them. (verification-id: archive-mark-preservation) (verification: integration - `cargo test --lib run_mark_intent_archive_revision -- --nocapture`)
- [ ] Run the focused execution-mark suite and API/TUI local-command convergence regression to prove unrelated target-scoped marks are preserved. (verification-id: archive-mark-preservation) (verification: integration - `cargo test --lib accepted_operator_command_tui_convergence_local_command_preserves_unrelated_remote_marks -- --nocapture`)
- [ ] Verify the promoted canonical diff changes only the archive behavior in `remote-control-api` and preserves every existing `operator-command-execution` scenario. (verification-id: archive-mark-preservation) (verification: integration - runnable commands `cflx openspec validate preserve-marks-after-archive --archive-gate` と `git diff -- openspec/specs/operator-command-execution/spec.md openspec/specs/remote-control-api/spec.md` で canonical source paths を確認する)

## Future Work

None.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate preserve-marks-after-archive --archive-gate`
