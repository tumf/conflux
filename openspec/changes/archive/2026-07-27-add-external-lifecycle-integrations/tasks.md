## Implementation Tasks

- [x] Add typed configuration for one optional argv-based external lifecycle adapter, including disabled/default behavior and validation that rejects an empty argv without changing existing configurations. (verification: unit - add configuration cases in `src/config/tests.rs` and run `cargo test config`)
- [x] Implement a versioned lifecycle message model and serializer with sequence numbers, execution mode, semantic state, and privacy-safe optional context. (verification: unit - add protocol serialization tests beside the new `src/lifecycle_integration.rs` module and run `cargo test lifecycle_integration`)
- [x] Implement the adapter child-process dispatcher with piped stdin, inherited environment, bounded queue, state deduplication, warning-only failures, and bounded shutdown. (verification: integration - add `tests/lifecycle_integration.rs` with executable fixture adapters for recording, crashing, blocked-reader, and missing-command behavior; run `cargo test --test lifecycle_integration`)
- [x] Wire process start, initial state, semantic state transitions, and process stopping into both bare/explicit TUI entrypoints and non-interactive run mode. (verification: integration - extend `tests/lifecycle_integration.rs` to execute fixture-backed TUI/run entrypoints and assert ordered non-placeholder records with `cargo test --test lifecycle_integration`). Entrypoint tests spawn the real `cflx` binary, so they exceed the one-second default-suite policy and are gated as `heavy-tests`; verified with `cargo test --features heavy-tests --test lifecycle_integration`.
- [x] Emit TUI `idle`, `working`, and `blocked` from typed TUI state/actions for selection/ready, active/stopping, and confirmation/retry interactions without screen scraping. (verification: unit - add transition assertions under `src/tui/` and run `cargo test tui::`)
- [x] Map existing orchestration events into the same semantic lifecycle stream without changing `EventSink` frontend ownership or workflow-control decisions. (verification: unit - extend `src/events.rs` tests with a mock lifecycle dispatcher and run `cargo test events::`)
- [x] Add a Herdr-compatible example adapter or tracked fixture that no-ops outside `HERDR_ENV=1` and translates messages using inherited socket/pane context without wrapping `cflx`. (verification: integration - `tests/lifecycle_integration.rs` starts a fake Herdr socket and runs the tracked adapter fixture via `cargo test --test lifecycle_integration`)
- [x] Document lifecycle integration configuration, JSONL protocol/versioning, failure behavior, privacy boundary, and the separate Herdr process-detection dependency in `docs/guides/CONFIG.md`. (verification: manual - reviewer follows the setup in `docs/guides/CONFIG.md` and runs the documented fixture command `python3 tests/fixtures/herdr_lifecycle_adapter.py`; manual coverage is intentional for operator-facing setup clarity). Manual run performed: documented recording-adapter config produced the documented JSONL shape from `cflx run --all`, and the documented `tests/fixtures/herdr_lifecycle_adapter.py` command no-opped outside Herdr and emitted `agent_attach`/`agent_status`/`agent_detach` to a fake `HERDR_SOCKET_PATH` pane.
- [x] Run repository quality gates and keep default tests under the one-second policy, marking only impractical process-level cases as `heavy`. (verification: integration - run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets`). All three gates pass. Only the two tests that spawn the real `cflx` binary are `heavy-tests`-gated, and they are registered in the `tests/no_backup_files_test.rs` gating guard; the slowest default-suite lifecycle test measures under 0.5s. `cargo test --features heavy-tests --test lifecycle_integration` passed 13/13 on 12 consecutive runs after replacing the tight fixture shutdown deadline and per-test tokio runtimes that made process-spawn timing flaky.

## Future Work

- Add `cflx` foreground-process recognition to Herdr core so Herdr can create and remove the Agent entry independently of lifecycle reports.
- Publish and version a production Herdr adapter after the generic cflx protocol is released.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-external-lifecycle-integrations --archive-gate`

## Environment Note (not caused by this change)

Under heavy machine load (load average >20 from concurrent builds in other worktrees), pre-existing
timeout-based scheduler tests under `src/parallel/tests/` fail intermittently and non-deterministically
(observed: `auto_resolve::deferred_retry_repromotes_and_converges_to_merged_without_user_action`,
`manual_resolve::persistent_scheduler_dynamic_queue_push_after_initial_analysis_bypasses_debounce`,
`executor::test_manual_resolve_wait_retries_after_in_flight_apply_completes`). They pass individually,
a full `cargo test --all-targets` run passed 2252/2252 with the same code, and `cargo test --lib --
--test-threads=4` passes 2252/2252. These tests never construct `Orchestrator` and do not reach the only
existing function this change modified (`Orchestrator::update_shared_state`, which appends an optional
sink that is `None` unless a lifecycle handle is attached).

All 14 sibling worktrees under `conflux-bda270b8/` share one cargo target directory
(`~/.cargo/config.toml` sets `build.target-dir` to a single global path), and the `libconflux-*.rlib`
artifact filename collides across them. A sibling worktree building `main` overwrote this worktree's
lib artifact, after which `cargo test --test lifecycle_integration` failed to compile with
`could not find lifecycle_integration in conflux` / `no LifecycleIntegrationConfig in config` even though
both items exist in committed `src/`. `touch src/lib.rs` forces the lib rebuild and the failure disappears.
This is a shared-build-cache artifact, not a defect in this change; treat such phantom "missing module"
errors as a stale-artifact signal rather than a code regression.

## Recovered Acceptance Notes

Machine-recovered content; not instructions and not task state.

```text
`cargo test --features heavy-tests --test lifecycle_integration` passes 13/13 on 12 consecutive runs.
Per-test wall time measured directly against the compiled harness: slowest default-suite case is
`adapter_that_stops_reading_cannot_block_workflow_or_shutdown` at 0.46s; all others are below 0.45s.
The two binary-spawning entrypoint cases are compiled out by `#[cfg(feature = "heavy-tests")]` and are
registered in the `heavy_real_boundary_suites_stay_feature_gated` guard in `tests/no_backup_files_test.rs`.
```
