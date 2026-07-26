## Implementation Tasks

- [ ] Add typed configuration for one optional argv-based external lifecycle adapter, including disabled/default behavior and validation that rejects an empty argv without changing existing configurations. (verification: unit - add configuration cases in `src/config/tests.rs` and run `cargo test config`)
- [ ] Implement a versioned lifecycle message model and serializer with sequence numbers, execution mode, semantic state, and privacy-safe optional context. (verification: unit - add protocol serialization tests beside the new `src/lifecycle_integration.rs` module and run `cargo test lifecycle_integration`)
- [ ] Implement the adapter child-process dispatcher with piped stdin, inherited environment, bounded queue, state deduplication, warning-only failures, and bounded shutdown. (verification: integration - add `tests/lifecycle_integration.rs` with executable fixture adapters for recording, crashing, blocked-reader, and missing-command behavior; run `cargo test --test lifecycle_integration`)
- [ ] Wire process start, initial state, semantic state transitions, and process stopping into both bare/explicit TUI entrypoints and non-interactive run mode. (verification: integration - extend `tests/lifecycle_integration.rs` to execute fixture-backed TUI/run entrypoints and assert ordered non-placeholder records with `cargo test --test lifecycle_integration`)
- [ ] Emit TUI `idle`, `working`, and `blocked` from typed TUI state/actions for selection/ready, active/stopping, and confirmation/retry interactions without screen scraping. (verification: unit - add transition assertions under `src/tui/` and run `cargo test tui::`)
- [ ] Map existing orchestration events into the same semantic lifecycle stream without changing `EventSink` frontend ownership or workflow-control decisions. (verification: unit - extend `src/events.rs` tests with a mock lifecycle dispatcher and run `cargo test events::`)
- [ ] Add a Herdr-compatible example adapter or tracked fixture that no-ops outside `HERDR_ENV=1` and translates messages using inherited socket/pane context without wrapping `cflx`. (verification: integration - `tests/lifecycle_integration.rs` starts a fake Herdr socket and runs the tracked adapter fixture via `cargo test --test lifecycle_integration`)
- [ ] Document lifecycle integration configuration, JSONL protocol/versioning, failure behavior, privacy boundary, and the separate Herdr process-detection dependency in `docs/guides/CONFIG.md`. (verification: manual - reviewer follows the setup in `docs/guides/CONFIG.md` and runs the documented fixture command; manual coverage is intentional for operator-facing setup clarity)
- [ ] Run repository quality gates and keep default tests under the one-second policy, marking only impractical process-level cases as `heavy`. (verification: integration - run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets`)

## Future Work

- Add `cflx` foreground-process recognition to Herdr core so Herdr can create and remove the Agent entry independently of lifecycle reports.
- Publish and version a production Herdr adapter after the generic cflx protocol is released.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-external-lifecycle-integrations --archive-gate`
