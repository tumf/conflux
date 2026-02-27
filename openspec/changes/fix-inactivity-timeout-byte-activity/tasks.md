## Implementation Tasks

- [ ] Update streaming readers to track activity on byte reception (verification: add unit test + `cargo test`)
- [ ] Preserve line-based log emission while using byte-based activity timestamps (verification: unit test covers newline-less output)
- [ ] Ensure both stdout and stderr update last-activity (verification: unit test covers stderr-only output)
- [ ] Run `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` (verification: all commands succeed)

## Future Work

- Consider adding a max wall-clock runtime setting to avoid indefinite runs for noisy processes.
