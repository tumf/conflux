## Implementation Tasks

- [x] Add config field `command_inactivity_timeout_max_retries` (default: 0) and wire it into runtime config (verification: unit test loads config and default is 0)
- [x] Implement inactivity-timeout retry loop in streaming runner (verification: test command that produces no output triggers inactivity timeout, then is re-run up to 3 times)
- [x] Emit user-facing retry messages for inactivity-timeout retries (verification: output includes `Retry` and `inactivity timeout` with attempt counts)
- [x] Ensure non-inactivity retries remain unchanged and do not regress (verification: existing retry tests pass)
- [x] Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (verification: all pass)

## Future Work

- Consider a separate `command_inactivity_timeout_retry_delay_ms` override if users want a different backoff for this class of failure.
