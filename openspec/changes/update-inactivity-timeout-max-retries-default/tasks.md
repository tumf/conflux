## Implementation Tasks

- [ ] Update default config constant for `command_inactivity_timeout_max_retries` to 3 (verification: `cargo test` passes)
- [ ] Update/adjust unit tests that assert the default inactivity-timeout max retries value (verification: `cargo test`)
- [ ] Update any user-facing documentation/comments that claim the default is 0 (verification: `rg "inactivity timeout max retries" -n src openspec` shows no stale statements)
- [ ] Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (verification: all commands succeed)

## Future Work

- Consider improving inactivity-timeout detection so near-completion output does not get killed (would be a separate change).
