## Implementation Tasks

- [x] Classify structured external blockers in `src/client/wait.rs` as manual-action wait results while retaining observation for generic and dependency-driven blocked rows. (verification: integration - `cargo test --test client_cli_tests enabled::wait_releases_external_blocker_without_waiting_for_operator -- --exact`; verification-id: client-wait-external-blocker)
- [x] Preserve the existing observation-only envelope and update CLI-facing documentation for the external-blocker exception. (verification: integration - assertions in `tests/client_cli_tests.rs` verify outcome, exit status, detail, zero commands, and unchanged repository; verification-id: client-wait-external-blocker)

## Notes

- The shared classifier now takes the structured blocker kind: `classify(display_status, blocker_kind)` in `src/client/completion.rs` releases only `blocked` + `BlockerKind::External`, and every other blocked row — `dependency`, `none`, or no published blocker at all — keeps observing. Gating on the status as well as the kind keeps a stale or future blocker on a live row from releasing a waiter.
- The release reuses the existing `change_requires_action` outcome and exit status `27`. `detail.blocker` carries the serialized snapshot blocker so the released caller reads `unblock_condition` and `prerequisite_owner` as data rather than parsing the message; `detail.error_detail` falls back to the blocker's own one-line detail when the change published no error.
- The declared verification command was refined from the bare test name to `enabled::wait_releases_external_blocker_without_waiting_for_operator`. libtest's `--exact` matches the full module path, and the wait suite lives inside the `web-monitoring`-gated `enabled` module, so the bare name selected zero tests. The proposal's rerun command and completion condition were updated to the runnable form.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-client-wait-external-blocker --archive-gate`.

- `cargo test --test client_cli_tests enabled::wait_releases_external_blocker_without_waiting_for_operator -- --exact` → 1 passed, 0 failed, 88 filtered out.
- `cargo test --test client_cli_tests` → 88 passed, 0 failed, 1 ignored.
- `cargo test --lib client::` → 106 passed, 0 failed; `cargo test --lib client::completion` → 8 passed, 0 failed.
- `cargo clippy --all-targets --all-features` → no warnings; `cargo fmt --all -- --check` → clean.
