## Implementation Tasks

- [x] Add a production-equivalent regression that keeps one change active, marks another ordinary eligible target through the shared operator/API path, advances the stability window, and proves queue intent plus exactly one scheduler analysis wake. The test must fail against the pre-fix behavior rather than manually invoking settlement. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)
- [x] Repair the shared owner/application settlement lifecycle so every accepted changed mark during a live persistent scheduler retains its timer task and runtime binding until the target is reconciled or receives a stable observable exclusion. Do not enqueue in a frontend or synthesize Start. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)
- [x] Add stable observability for arm and completion failures, and verify TUI Space/bulk and client/API mark adapters converge on the same settlement path without changing unrelated queue intent. (verification: integration - `cargo test --locked running_mark_reanalysis`; verification-id: running-mark-settlement-regression)

## Notes

Root cause, from the live `diffusion-kkc` owner's own log rather than inference:
at `15:19:38.186` settlement reported
`Mark settlement changed no queue intent for: add-stt-ccr-loop-runner=not_loadable`,
and at `15:19:39.745` the same process logged
`Detected new change: add-stt-ccr-loop-runner`. The mark was accepted against the
catalog roughly ten seconds before the reducer had any runtime state for the
proposal, so the stability window expired one and a half seconds too early. The
classifier failed closed with `NotLoadable`, the batch was discarded, and the row
stayed marked, tracked, eligible and `not queued` for the rest of the process
lifetime with only a `DEBUG` line to explain it.

The repair keeps a batch until it is *answered*: `NotLoadable` is the absence of
evidence rather than a decision, so those targets stay in the batch and re-arm
the deadline for a bounded `MARK_SETTLEMENT_ATTEMPTS` budget. Every other
exclusion is a decision about a row the reducer holds and still ends the batch's
interest immediately, so no existing skip semantics moved.

Settlement lifecycle failures gained a stable reason each
(`runtime_unbound`, `runtime_gone`, `no_task_runtime`, `unreconciled_batch`); a
runtime lost under a pending deadline no longer takes the batch with it *and*
leaves observers waiting on a pass that can never arrive; and the write
boundary's own refusal reason now reaches the settled plan instead of being
dropped.

- evidence: `cargo test --locked running_mark_reanalysis` — 44 passed, exit 0
- evidence: `cargo test --locked` — all binaries green (4169 lib + 230 integration), 0 failed
- evidence: `cargo clippy --locked --all-targets --all-features` — no warnings
- evidence: with `MarkSettlementExclusion::is_stable` forced to `true` (the pre-fix
  behaviour) `running_mark_reanalysis_running_owner_admits_a_late_catalog_target`
  fails at the retained-batch assertion: `left: None, right: Some(["alpha"])`

## Final Validation

Archive validation is authoritative. Expected gate: `cflx openspec validate fix-running-mark-settlement-admission --archive-gate`.
