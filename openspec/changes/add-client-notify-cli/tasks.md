## Implementation Tasks

- [ ] Add nested `cflx client notify set|get|clear` Clap arguments in `src/cli.rs`, including `--json`, optional `--instance-id`, `--blocked` for set, and a required non-empty argv after `--`. (verification-id: client-notify-cli) (verification: unit - `cargo test --test client_cli_tests client_notify_help_and_usage`)
- [ ] Route the three CLI intents through `src/client/mod.rs` to the existing `src/client/notify.rs` implementation while preserving connection resolution, operation names, envelopes, exit codes, and owner-binding checks. (verification-id: client-notify-cli) (verification: integration - `cargo test --test client_cli_tests client_notify_routes_through_existing_owner`)
- [ ] Add focused regressions in `tests/client_cli_tests.rs` for argv boundary preservation, blocked opt-in, set/get/clear, human and JSON output, empty command rejection, owner restart, stale execution binding, unsupported owner, and Unix-socket mutation restrictions. (verification-id: client-notify-cli) (verification: integration - `cargo test --test client_cli_tests client_notify`)
- [ ] Update `README.md`, `AGENTS.md`, CLI help examples, and embedded `skills/cflx-run/SKILL.md` to prefer direct CLI callback management for shell-capable agents, retain MCP guidance for MCP-only hosts, document execution-scoped identifiers and argv-not-shell safety, and state that TUI process exit is not completion. (verification-id: client-notify-cli) (verification: unit - `cargo test --test client_cli_tests client_notify_help_and_usage`)

## Future Work

- Automatically composing `enqueue` and notify registration remains separate because callback selection and execution binding are caller-owned.

## Final Validation

Expected archive gate: `cflx openspec validate add-client-notify-cli --archive-gate`
