## Implementation Tasks

- [x] Reconcile the current unresolved dependency set into reducer/runtime state on every scheduler classification, independently of diagnostic deduplication, and clear it only from repository-visible resolution evidence. (verification: unit - `cargo test dependency_blocker_projection_initial && cargo test dependency_blocker_projection_rebuild && cargo test dependency_blocker_projection_resolution` covers first classification, unchanged fingerprints, projection reconstruction, changed blocker sets, and resolution; verification-id: dependency-projection-tests)

- [x] Project dependency-ineligible admitted work consistently through runtime snapshots and `/api/v2`: `display_status=blocked`, `execution_state=queued`, retained queue intent, `parallel_eligible=false`, and structured dependency IDs. (verification: integration - `cargo test dependency_blocker_projection_initial && cargo test dependency_blocker_projection_capacity_only` asserts the complete snapshot contract and a capacity-only queued control; verification-id: dependency-projection-tests)

- [x] Render the shared dependency projection as `[blocked:dependency]` in the TUI without duplicating scheduler classification logic. (verification: unit - `cargo test dependency_blocker_projection_tui_badge` asserts badge rendering from shared projected state; verification-id: dependency-projection-tests)

- [x] Preserve deduplicated operator diagnostics and existing dispatch/concurrency behavior while adding regression coverage that repeated classification remains non-spamming and state-complete. (verification: integration - `cargo test dependency_blocker_projection_rebuild` asserts one diagnostic per unchanged fingerprint while coherent snapshots stay blocked; verification-id: dependency-projection-tests)

## Notes

- Reconciliation lives in `OrchestratorState::reconcile_dependency_blocker` / `clear_dependency_blocker` (`src/orchestration/state.rs`) and is called from `ParallelExecutor::select_changes_for_dispatch` and `observe_failed_dependency_blocks` (`src/parallel/queue_state.rs`) *before* the `should_emit_dependency_blocked_transition` gate, so the fingerprint store keeps deciding diagnostics and never decides current state.
- The structured dependency IDs travel as `BlockedMetadata::dependency_ids` -> `BlockerView::dependencies` -> `ChangeBlocker.dependencies`, so no consumer parses operator prose to learn what a change is waiting on.
- `parallel_eligible=false` is folded in at the `/api/v2` snapshot projection (`fold_dependency_wait_into_eligibility`) with the new `dependency_blocked` blocked reason. The TUI's own workspace-derived eligibility is deliberately untouched: it drives mark/queue cleanup, and a transient dependency wait must not clear an operator's marks. An actionable workspace reason (`not_committed`, `uncommitted_changes`) still wins over the self-clearing dependency reason.
- `execution_state` stays `queued` for a dependency wait (`project_execution_state`) because no execution episode has started; every other wait state keeps `waiting`.
- Regression proof: reverting only the `reconcile_dependency_blocker_projection` call in `select_changes_for_dispatch` makes `dependency_blocker_projection_rebuild_survives_deduplicated_diagnostics` fail with `left: "queued" right: "blocked"` — the reported defect.
- evidence: `cargo test --all-features --lib dependency_blocker_projection` — 10 passed, 0 failed.
- Pre-existing failures unrelated to this change, confirmed identical on the unmodified tree before any edit: `execution::apply::tests::apply_process_group_barrier_finalizes_after_descendant_releases_index_lock`, `parallel::tests::conflict::sequential_resolve_tracks_the_full_reported_regression`, and five `parallel::tests::executor::test_merge_*` cases.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected command: `cflx openspec validate fix-dependency-blocker-projection --archive-gate`.
