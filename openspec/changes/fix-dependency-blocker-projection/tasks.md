## Implementation Tasks

- [ ] Reconcile the current unresolved dependency set into reducer/runtime state on every scheduler classification, independently of diagnostic deduplication, and clear it only from repository-visible resolution evidence. (verification: unit - `cargo test dependency_blocker_projection_initial && cargo test dependency_blocker_projection_rebuild && cargo test dependency_blocker_projection_resolution` covers first classification, unchanged fingerprints, projection reconstruction, changed blocker sets, and resolution; verification-id: dependency-projection-tests)

- [ ] Project dependency-ineligible admitted work consistently through runtime snapshots and `/api/v2`: `display_status=blocked`, `execution_state=queued`, retained queue intent, `parallel_eligible=false`, and structured dependency IDs. (verification: integration - `cargo test dependency_blocker_projection_initial && cargo test dependency_blocker_projection_capacity_only` asserts the complete snapshot contract and a capacity-only queued control; verification-id: dependency-projection-tests)

- [ ] Render the shared dependency projection as `[blocked:dependency]` in the TUI without duplicating scheduler classification logic. (verification: unit - `cargo test dependency_blocker_projection_tui_badge` asserts badge rendering from shared projected state; verification-id: dependency-projection-tests)

- [ ] Preserve deduplicated operator diagnostics and existing dispatch/concurrency behavior while adding regression coverage that repeated classification remains non-spamming and state-complete. (verification: integration - `cargo test dependency_blocker_projection_rebuild` asserts one diagnostic per unchanged fingerprint while coherent snapshots stay blocked; verification-id: dependency-projection-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected command: `cflx openspec validate fix-dependency-blocker-projection --archive-gate`.
