## Implementation Tasks

- [ ] Make the workspace-observation merge helper private, route `refresh_from_disk_at` through it, and expose only an explicitly named `#[doc(hidden)]` test-seeding entry point for integration crates. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)
- [ ] Make the existing unit-test `update` helper and integration-test seed entry point delegate to the same merge implementation. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)
- [ ] Repoint every unit and integration test caller, then run dispatcher single-owner regressions and both affected integration-test crates. (verification: integration - `cargo test web::state tui::command_handlers::cross_adapter_tests --lib && cargo test --test client_cli_tests --test client_completion_sink`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate retire-webstate-direct-updates --archive-gate`.

## Future Work

- None.
