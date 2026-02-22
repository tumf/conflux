## Implementation Tasks

- [ ] Add `cflx service` CLI subcommand group (`install|uninstall|status|start|stop|restart`) (verification: `cargo test` and `cflx --help` shows service commands)
- [ ] Implement `src/service/mod.rs` with platform-specific operations (verification: unit tests for service file generation; `cargo test`)
- [ ] Wire command dispatch in `src/main.rs` (verification: `cargo test`)
- [ ] Ensure `cflx service start/restart/install` validates server config security policy (verification: add tests around validation path; manual: non-loopback bind without token fails)
- [ ] Add documentation/help text for new commands (verification: `cflx service --help` output includes examples)
- [ ] Run `cargo fmt` and `cargo clippy -- -D warnings` (verification: commands succeed)

## Future Work

- Add OpenRC support (Linux) if needed for target environments.
- Add integration tests that exercise actual service managers on CI runners (if available).
