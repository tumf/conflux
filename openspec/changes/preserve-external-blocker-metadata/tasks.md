## Implementation Tasks

- [x] Enforce monotonic blocker precedence in `src/orchestration/state.rs` so `WorkspaceStatusUpdated { Blocked }` confirms but does not replace an existing structured `ExternalBlocked` or Acceptance-owned stalled hold. Completion requires preserving wait state, blocker kind, category/detail, origin, prerequisite owner, unblock condition, Acceptance ownership, and resumability while retaining the generic stalled fallback when no structured blocker exists. (verification: unit - `cargo test --lib structured_blocker_metadata_survives_workspace_blocked`; verification-id: blocker-metadata-regressions)

- [x] Add reducer event-sequence tests for `AcceptanceGated` and `ExecutionBlocked` followed by generic blocked workspace status, including Acceptance-origin external, Apply-origin external, Acceptance-owned non-external stalled, and unstructured fallback cases. Completion requires assertions against `display_status`, `blocker_view`, held-set membership, and every routing-relevant metadata field rather than only the rendered status string. (verification: unit - `cargo test --lib structured_blocker_metadata_survives_workspace_blocked`; verification-id: blocker-metadata-regressions)

- [x] Cover the real external-blocker dispatch sequence through a TUI-style `EventDispatcher` harness that applies both the structured event and compatibility workspace-status event to reducer state, then prove it leaves one held change instead of an ordinary queued candidate. Completion requires a repository-local test that distinguishes this reducer-owning boundary from the headless Web-only forwarder and fails if the applied workspace is eligible for another automatic Acceptance dispatch without explicit retry. (verification: integration - `cargo test --lib external_blocker_hold_survives_dispatch_status`; verification-id: blocker-metadata-regressions)

- [x] Extend operator/API projection regressions so a preserved resumable hold allows the existing acceptance-only retry route, a preserved non-resumable hold remains refused without evidence loss, and projected blocker fields stay reducer-derived. Completion requires exercising command/projection code after the full two-event sequence, not after `AcceptanceGated` alone. (verification: integration - `cargo test --lib external_blocker_hold_survives_dispatch_status && cargo test --lib orchestration::operator_command`; verification-id: blocker-metadata-regressions)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate preserve-external-blocker-metadata --archive-gate`.

## Future Work

- Consider removing redundant generic blocked events only in a separate event-contract cleanup after all consumers are proven independent of them.
