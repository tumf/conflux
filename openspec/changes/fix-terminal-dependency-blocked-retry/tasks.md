## Implementation Tasks

- [x] Extend repository-visible dependency target classification to include rejected dependencies; completion requires `openspec/changes/<id>/REJECTED.md` evidence to produce a rejected class without using external logs or durable state (verification: unit - `cargo test dependency_targets --lib` covers active, in-flight, archived, missing, and rejected classifications)
- [x] Align native OpenSpec utility status rendering with rejected dependency classification; completion requires `cflx openspec list`, human-readable `show`, and JSON `show` to render rejected dependency status without showing it as pending or missing (verification: unit - `cargo test openspec_cmd --lib` covers rejected dependency status in list/show/json and no-dependency output remains unchanged)
- [x] Update analyzer and scheduler validation paths so rejected dependencies are dedicated terminal blockers rather than generic missing/parse failures; completion requires rejected dependencies to block dispatch with a class-specific diagnostic while archived dependencies remain satisfied (verification: unit - `cargo test analyzer --lib` and targeted parallel executor tests cover rejected, missing, archived, queued, and in-flight dependency outcomes)
- [x] Deduplicate unchanged dependency-blocked diagnostics in the scheduler/TUI event path; completion requires repeated scheduler loops with the same `(change_id, dependency ids, dependency classes)` blocker signature to avoid duplicate operator-visible warn/error log entries while preserving the initial diagnostic (verification: unit - `cargo test parallel::tests::executor --lib test_dependency_blocker_diagnostics_dedupe_and_reemit_on_signature_change` covers `src/parallel/queue_state.rs` signature deduplication and asserts repeated missing-blocked analysis emits only one user-visible diagnostic)
- [x] Re-emit diagnostics and re-evaluate dispatch when blocker evidence changes; completion requires a previously rejected/missing blocker changing to archived, queued, in-flight, or a different blocker set to clear or update the dedup state and follow the correct dispatch/block behavior (verification: integration - `cargo test parallel::tests::executor --lib test_dependency_blocker_diagnostics_dedupe_and_reemit_on_signature_change test_dependency_blocker_archived_unblocks_dispatch_after_terminal_marker_removed` covers repository-visible blocker class changes in `src/parallel/queue_state.rs` and confirms new diagnostics plus correct dispatch decision)
- [x] Preserve fresh-workspace behavior after a recoverable dependency resolves; completion requires queued/in-flight dependency resolution to continue emitting `DependencyResolved` and force fresh workspace recreation for the dependent change (verification: integration - regression command `cargo test parallel::tests::executor --lib test_dependency_blocker_archived_unblocks_dispatch_after_terminal_marker_removed` verifies `src/parallel/queue_state.rs` marks the dependent change for fresh worktree recreation after terminal blocker evidence is replaced by archived evidence)
- [x] Run targeted quality checks for touched Rust modules; completion requires the focused test commands and Rust formatting/checking for affected files to pass (verification: integration - `cargo test dependency_targets --lib`, `cargo test openspec_cmd --lib`, targeted `cargo test parallel::tests::executor --lib`, and `cargo fmt --check`)

## Future Work

- Operator-initiated recovery of a rejected dependency remains a separate workflow decision.
- UI polish for grouping rejected terminal rows outside the active list can be proposed separately if broader list taxonomy changes are desired.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-terminal-dependency-blocked-retry --archive-gate`

## Acceptance #1 Failure Follow-up

- [x] Update archive-gate verification notes with explicit repository-verifiable evidence for diagnostic deduplication, blocker reclassification, and fresh-workspace behavior (verification: not-testable - this is OpenSpec task metadata correction in `openspec/changes/fix-terminal-dependency-blocked-retry/tasks.md`, with no runtime code path; repository-verifiable evidence is this tasks file plus the runnable command `cflx openspec validate fix-terminal-dependency-blocked-retry --archive-gate`).

## Acceptance #1 Notes

Archive gate commit path was previously blocked because `cflx openspec validate fix-terminal-dependency-blocked-retry --archive-gate` reported tasks.md verification evidence/ownership errors at `openspec/changes/fix-terminal-dependency-blocked-retry/tasks.md:6`, `:7`, and `:8`, plus self-referential final validation checkboxes at lines 22-23. The follow-up task above is limited to metadata correction; final OpenSpec validation remains in the non-checkbox `## Final Validation` section.
