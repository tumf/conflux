## Implementation Tasks

- [ ] Add `src/orchestration/net_active_clock.rs` (`NetActiveClock` with injected-instant open/close/observe/total/accumulated/active_since), make `ChangeState` in `src/tui/state.rs` delegate to it without behavior change, and freeze `orchestration_elapsed` in `handle_error` (`src/tui/state/event_handlers/output.rs`). Completion: the clock unit cases, the unchanged row tests, the delegation-equivalence case, and the fatal-error freeze case pass with nonzero counts (verification: unit - `src/orchestration/net_active_clock.rs`, `src/tui/state.rs`; `cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture`; verification-id: remote-progress-timing-clock).
- [ ] Observe display statuses and `Stopped` through per-change clocks and track the run clock boundaries in `src/web/operator_facts.rs` and `src/web/state.rs`; read `archived_changes` in `apply_reducer_derived_queue_statuses`; add `archive_complete`, `net_active`, and `run_clock` as `#[serde(default)]` `Option` members with `#[schema(required, nullable = false)]` in `src/web/remote_control_api/dto.rs` and write them in `projection.rs`; add the clarifying `ChangeTiming` description. Completion: the `progress_timing_tests.rs` module registered in `tests/mod.rs` passes every case in `design.md`, including TUI/API equality, revision discipline, unchanged `ChangeTiming`, older-owner decoding, and restart-from-zero (verification: integration - `src/web/remote_control_api/tests/mod.rs`; `cargo test --lib --features web-monitoring remote_progress_timing -- --nocapture`; verification-id: remote-progress-timing-api).
- [ ] Extend `tests/openapi_contract_tests.rs` with the required-and-non-null publication case for the three members and its omission negative control. Completion: both cases pass and the CLI export equals the live document (verification: integration - `tests/openapi_contract_tests.rs`; `cargo test --features web-monitoring --test openapi_contract_tests remote_progress_timing -- --nocapture`; verification-id: remote-progress-timing-contract).
- [ ] Document the three members in `docs/guides/USAGE.md` (`/api/v2` notes) and `AGENTS.md`: archive completion versus status, per-run `ChangeTiming` versus cumulative `net_active`, run-clock boundaries including persistent idle and fatal error, derivation of live values from `active_since`/`started_at`, process-local lifetime, and "unpublished" on older owners. Completion: the text matches the implemented boundaries and never describes a server-side live counter (verification: manual - documentation review of `docs/guides/USAGE.md` and `AGENTS.md` against `src/orchestration/net_active_clock.rs` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-progress-timing-contract).
- [ ] Launch the independent implementation review exactly per the "Independent review launch contract" in `design.md` (`agent-exec run --until 100 --stdin-file /tmp/<UUID>.md -- claude-auto-fable …`, no `--model`), and report job ID, raw log path, argv, responding model, and final answer without killing or duplicating a running job. Completion: the review artifacts are attached to the handoff and no push, install, or owner replacement was performed (verification: manual - review artifacts recorded against `openspec/changes/add-remote-progress-timing/design.md` and the change-scoped `git diff --stat -- src/ tests/ docs/`; verification-id: remote-progress-timing-api).

## Final Validation

All three declared gates must select and execute their
`remote_progress_timing` cases; zero selected tests is a failure. Archive
validation is the authoritative OpenSpec structural/evidence gate:
`cflx openspec validate add-remote-progress-timing --archive-gate`.
Authoring uses `cflx openspec validate add-remote-progress-timing --strict`.
Neither command implements or archives this change, and structural PASS is
not implementation PASS.

## Future Work

Web rendering of archive completion and clocks and the live TUI/API/Web
comparison (CFSRV-59, CFSRV-56) are the Server's acceptance against this
contract. Per-phase cumulative clocks are not planned.
