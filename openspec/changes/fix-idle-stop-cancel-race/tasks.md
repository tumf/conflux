## Implementation Tasks

- [ ] Recheck the existing shared graceful-stop flag after safe-boundary qualification in `settle_graceful_stop_when_no_work` and return false without success logging or terminal emission when it was withdrawn. Keep the existing entry check, classification predicates, flags, cleanup, and one-event loop-exit path unchanged; introduce no await before the existing terminal send. Done when the new cancellation test rejects the old behavior and all existing guards pass. (verification: integration - `src/parallel/tests/blocked_idle_graceful_stop.rs` production helper/classifier and scheduler-loop regressions; verification-id: idle-stop-cancel-race)
- [ ] Add the two exact named archived-dependency barrier tests and later-Stop control described in `design.md` using the actual awaited classifier, shared run-control cancellation, real base evidence, and test-only workspace manager. Done when entry precedes cancel acknowledgement, cancel precedes release, the withdrawn evaluation returns false with zero Stopped, the pending/later request emits exactly one Stopped, and selected test evidence excludes sleeps, fabricated events, and zero-test passes. Keep existing positive/negative loop tests intact. (verification: integration - `src/parallel/tests/blocked_idle_graceful_stop.rs` deterministic real boundary plus workspace suspension fixture; verification-id: idle-stop-cancel-race)

## Acceptance Notes

Run both declared verification commands once and inspect selected/nonignored test names and counts. `idle-stop-existing-boundaries` preserves empty/no-request/shared-lifecycle behavior without changing frontend production code. No repository-wide-only checkbox is delegated to a staged-file hook.

## Final Validation

Use `cflx openspec validate fix-idle-stop-cancel-race --strict --evidence warn` while authoring. Archive validation is the authoritative final OpenSpec gate: `cflx openspec validate fix-idle-stop-cancel-race --archive-gate`. All automation paths point to stable src files and remain valid after archive; no active-change-relative runner is introduced.

## Future Work

No release, owner restart, deployment, broad event-send serialization, or physical/credentialed observation is required to complete this repository-local change. Any such operation is separately authorized and is not implied by proposal acceptance.
