## Implementation Tasks

- [x] Extend repository-visible dependency target classification to include rejected dependencies; completion requires `openspec/changes/<id>/REJECTED.md` evidence to produce a rejected class without using external logs or durable state (verification: unit - `cargo test dependency_targets --lib` covers active, in-flight, archived, missing, and rejected classifications)
- [x] Align native OpenSpec utility status rendering with rejected dependency classification; completion requires `cflx openspec list`, human-readable `show`, and JSON `show` to render rejected dependency status without showing it as pending or missing (verification: unit - `cargo test openspec_cmd --lib` covers rejected dependency status in list/show/json and no-dependency output remains unchanged)
- [x] Update analyzer and scheduler validation paths so rejected dependencies are dedicated terminal blockers rather than generic missing/parse failures; completion requires rejected dependencies to block dispatch with a class-specific diagnostic while archived dependencies remain satisfied (verification: unit - `cargo test analyzer --lib` and targeted parallel executor tests cover rejected, missing, archived, queued, and in-flight dependency outcomes)
- [x] Deduplicate unchanged dependency-blocked diagnostics in the scheduler/TUI event path; completion requires repeated scheduler loops with the same `(change_id, dependency ids, dependency classes)` blocker signature to avoid duplicate operator-visible warn/error log entries while preserving the initial diagnostic (verification: unit - targeted `parallel::tests::executor` coverage asserts repeated blocked analysis emits only one user-visible diagnostic for an unchanged signature)
- [x] Re-emit diagnostics and re-evaluate dispatch when blocker evidence changes; completion requires a previously rejected/missing blocker changing to archived, queued, in-flight, or a different blocker set to clear or update the dedup state and follow the correct dispatch/block behavior (verification: integration - targeted parallel executor test simulates blocker signature changes and confirms new diagnostics plus correct dispatch decision)
- [x] Preserve fresh-workspace behavior after a recoverable dependency resolves; completion requires queued/in-flight dependency resolution to continue emitting `DependencyResolved` and force fresh workspace recreation for the dependent change (verification: regression - existing dependency-resolved worktree recreation tests continue to pass with the new rejected/missing terminal blocker handling)
- [x] Run targeted quality checks for touched Rust modules; completion requires the focused test commands and Rust formatting/checking for affected files to pass (verification: integration - `cargo test dependency_targets --lib`, `cargo test openspec_cmd --lib`, targeted `cargo test parallel::tests::executor --lib`, and `cargo fmt --check`)

## Future Work

- Operator-initiated recovery of a rejected dependency remains a separate workflow decision.
- UI polish for grouping rejected terminal rows outside the active list can be proposed separately if broader list taxonomy changes are desired.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-terminal-dependency-blocked-retry --archive-gate`
