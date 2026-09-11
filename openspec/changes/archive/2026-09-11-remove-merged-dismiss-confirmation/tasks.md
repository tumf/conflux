## Implementation Tasks

- [x] Replace merged-row request/confirm/cancel state sequencing with immediate individual and bulk local dismissal using current exact-`merged` targets and the existing shared mutation/repair path (verification: unit - `cargo test tui:: --lib`; verification-id: immediate-dismissal-tests)
- [x] Remove merged-dismissal modal variants, validity checks, key ownership, popup rendering, confirmation logs, and confirmation-only tests without changing unrelated modal or Worktrees-delete behavior (verification: integration - `cargo test tui:: --lib`; verification-id: immediate-dismissal-tests)
- [x] Update key/state/render/refresh tests so one `d` or `D` event proves immediate removal, no modal, correct no-op behavior, projection-wide bulk scope, refresh suppression/reactivation, cursor/filter cleanup, and retained hints (verification: integration - `cargo test tui:: --lib`; verification-id: immediate-dismissal-tests)

## Future Work

- Durable cross-restart dismissal remains excluded.

## Final Validation

Archive validation is authoritative. Expected checks: `cflx openspec validate remove-merged-dismiss-confirmation --strict --evidence warn` and `cflx openspec validate remove-merged-dismiss-confirmation --archive-gate`.
