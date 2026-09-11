## Implementation Tasks

- [x] Make the workspace-observation merge helper private, route `refresh_from_disk_at` through it, and expose only an explicitly named `#[doc(hidden)]` test-seeding entry point for integration crates. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)
- [x] Make the existing unit-test `update` helper and integration-test seed entry point delegate to the same merge implementation. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)
- [x] Repoint every unit and integration test caller, then run dispatcher single-owner regressions and both affected integration-test crates. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate retire-webstate-direct-updates --archive-gate`.

## Notes

- The declared `focused-tests` command needs `--lib` and a `--` separator to pass two filters to one test binary; it was run as `cargo test --lib -- web::state tui::command_handlers::cross_adapter_tests` (42 passed, 0 failed) followed by `cargo test --test client_cli_tests --test client_completion_sink` (102 passed / 1 ignored, and 19 passed).
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean.
- `seed_workspace_observation_for_tests` carries `#[allow(dead_code)]`: it is `pub` for the `tests/` crates, and the binary target that compiles this module without them would otherwise report the intentional absence of a production caller as dead code.
- `merge_workspace_observation` keeps both prior merge policies rather than unifying them, because periodic disk refresh and mode-preserving test seeding differ in how they carry `persistent_scheduler_idle` and `iteration_number`; collapsing them would be a behavior change the proposal excludes.

## Future Work

- None.
