## Implementation Tasks

- [x] Remove `ChangeArchived` and archive-only evidence from mark revocation while preserving the existing target-scoped Error, Rejected, refresh-ineligible, dequeue, and merge-hook-recovery revocations in `src/orchestration/mark_reconciliation.rs`. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib run_mark_intent_archive -- --nocapture`)
- [x] Replace the reducer reconciliation archive-clear regression with a deterministic two-mark archive-preservation regression, run it before implementation to record RED, then after implementation to record GREEN. (verification-id: archive-mark-preservation) (verification: unit - `cargo test --lib run_mark_intent_archive -- --nocapture`)
- [x] Update the API event revision regression so the `change_archived` revision retains both target and unrelated marks, and later merge/push projections continue to retain them. (verification-id: archive-mark-preservation) (verification: integration - `cargo test --lib run_mark_intent_archive_revision -- --nocapture`)
- [x] Run the focused execution-mark suite and API/TUI local-command convergence regression to prove unrelated target-scoped marks are preserved. (verification-id: archive-mark-preservation) (verification: integration - `cargo test --lib accepted_operator_command_tui_convergence_local_command_preserves_unrelated_remote_marks -- --nocapture`)
- [x] Verify the promoted canonical diff changes only the archive behavior in `remote-control-api` and preserves every existing `operator-command-execution` scenario. (verification-id: archive-mark-preservation) (verification: integration - runnable commands `cflx openspec validate preserve-marks-after-archive --archive-gate` と `git diff -- openspec/specs/operator-command-execution/spec.md openspec/specs/remote-control-api/spec.md` で canonical source paths を確認する)

## Future Work

None.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate preserve-marks-after-archive --archive-gate`

## Notes

- RED evidence: with `ChangeArchived` still classified as `RevokingEdge::Archive`, `cargo test --lib run_mark_intent_archive -- --nocapture` failed 5/5 — `archive must revoke nothing, but reported ["alpha"]`, `MergeCompleted { .. } disturbed a mark it must preserve  left: ["beta"] right: ["alpha", "beta"]`, `archive must not classify as a revoking edge`, and the API `the archive revision revoked the mark on its own target`.
- GREEN evidence: after removing the archive edge, the same command reported `test result: ok. 5 passed; 0 failed`.
- Focused suite: `cargo test --lib execution_mark -- --nocapture` -> `17 passed; 0 failed`; `cargo test --lib accepted_operator_command_tui_convergence_local_command_preserves_unrelated_remote_marks -- --nocapture` -> `1 passed; 0 failed`.
- Whole default suite: `cargo test` -> every binary `ok`, lib `3959 passed; 0 failed; 17 ignored`. `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean.
- Canonical source paths: the change owns a delta for `remote-control-api` only (`openspec/changes/preserve-marks-after-archive/specs/remote-control-api/spec.md`), so promotion cannot touch `openspec/specs/operator-command-execution/spec.md`. Diffing the delta's MODIFIED block against the canonical requirement shows exactly three edits — the archive sentence, the archive scenario title, and its two archive `THEN`/`AND` lines — with every other scenario byte-identical. `git diff -- openspec/specs/operator-command-execution/spec.md openspec/specs/remote-control-api/spec.md` is empty in the working tree; promotion happens at archive.
- `cflx openspec validate preserve-marks-after-archive --strict` and `--archive-gate` both pass.
