## Implementation Tasks

- [x] Move dedicated Acceptance configuration semantics into validated configuration types and `CommandQueueConfig`, preserving standard load precedence and generated examples (verification: unit - `cargo test config:: --lib`; verification-id: acceptance-runtime-config-tests)
- [x] Resolve the effective runtime inside the common runner from `operation_type`, including common=30, common=0, and cleanup-review boundaries without changing the runner API (verification: unit - `cargo test config:: --lib`; verification-id: acceptance-runtime-config-tests)
- [x] Consume Acceptance runtime termination in the executor as a dedicated typed terminal outcome after cleanup proof (verification: unit - `cargo test parallel:: --lib`; verification-id: acceptance-runtime-routing-tests)
- [x] Route that outcome in dispatch without command retry, missing-verdict retry, Acceptance retry, counter increment, or Apply re-entry; use injected time in tests (verification: unit - `cargo test parallel:: --lib`; verification-id: acceptance-runtime-routing-tests)

## Future Work

Tune the default only from observed Acceptance duration evidence. A durable cross-restart budget requires a separate constitutional design.

## Notes

- evidence: `cargo test config:: --lib` — 179 passed, 0 failed. Covers the 300 floor
  (`acceptance_max_runtime_enforces_its_range_bounds` asserts 1/60/299/10801/86400 rejected and
  300/1800/10800 accepted), zero rejection, layered precedence, the 30-vs-1800 override
  (`a_shorter_common_limit_overrides_the_dedicated_floor`), and operation-type selection including
  cleanup-review and near-miss tokens (`command_queue_config_selects_the_limit_by_operation_type`).
- evidence: `cargo test parallel:: --lib` — 494 passed, 0 failed, including the five
  `parallel::tests::acceptance_runtime_limit` routing tests. They assert the consecutive
  command-failure count is neither incremented nor reset, no command-recovery context is created,
  and the outcome permits no Acceptance retry, no canonical verdict, and no PASS.
- evidence: validated-load wiring is `OrchestratorConfig::validate_required_commands`
  (src/config/types.rs:1116), the existing load-validation path; no new validation entrypoint.
- evidence: limit selection has exactly one home,
  `CommandQueueConfig::effective_max_runtime_secs` (src/command_queue.rs), called by the common
  runner (src/ai_command_runner.rs) from the operation type. The forked-runner API
  (`AiCommandRunner::with_max_runtime_secs`, `CommandQueue::with_max_runtime_secs`) and the
  duplicate `OrchestratorConfig::get_acceptance_runtime_limit_secs` accessor were removed, so the
  runner signature is unchanged and no call site can hand another class the Acceptance deadline.
- evidence: end-to-end terminal routing is proven on the production path by
  `parallel_acceptance_runtime_limit_is_terminal_and_never_re_enters_apply`, which drives
  `dispatch_change_to_workspace` with an injected 1-second dedicated limit and a disabled common
  budget: 1 acceptance invocation, 1 acceptance process, 1 apply, no final revision. Heavy tier
  (`cargo test --lib --features heavy-tests -- parallel_acceptance_runtime_limit_is_terminal_and_never_re_enters_apply`)
  — 1 passed. Integration evidence, not the unit evidence the tasks above claim.
- evidence: `cargo test --test process_cleanup_test --features heavy-tests -- absolute_runtime_limit`
  — 4 passed; `acceptance_stays_bounded_when_the_common_limit_is_disabled` now selects the bound
  through the common runner's operation type rather than a forked runner.
- evidence: `cargo fmt --check` clean; `cargo clippy --all-targets --all-features -- -D warnings` clean.
- The `parallel-execution` MODIFIED block initially restated only the new runtime-limit prose, which
  would have deleted the canonical `Acceptance command recovers without rerunning Apply` scenario and
  the bounded-retry prose on promotion. Both are restored in the delta; the bounded recovery is
  unchanged by this change and is not a retirement.
- Pre-existing and unrelated: `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario`
  fails at HEAD on the pending change `refine-verification-gate-policy`, which drops two undeclared
  canonical scenarios from `cflx-proposal-validation`. Verified by stashing this change entirely and
  re-running. With the three unrelated pending changes moved aside, the same test passes against this
  change alone. Not repaired here: editing another change's delta is outside this change's scope.

## Final Validation

Archive validation is authoritative. Expected command: `cflx openspec validate correct-acceptance-runtime-routing --archive-gate`.

Result: passed (one informational warning that the `bound-acceptance-runtime` dependency is an
archived reference). `cflx openspec validate correct-acceptance-runtime-routing --strict` also passed.
