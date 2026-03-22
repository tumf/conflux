## Implementation Tasks

- [ ] Update the default server port constant and derived default configuration behavior to `39876` (verification: `src/config/defaults.rs`, `src/config/mod.rs`)
- [ ] Update CLI help text, examples, and default remote endpoint text that currently assume `127.0.0.1:9876` (verification: `src/cli.rs`, `src/main.rs`)
- [ ] Update tests that encode the old default port so they assert `39876` instead (verification: repository tests covering server/project default URL behavior)
- [ ] Update OpenSpec deltas and user-facing documentation that describe the default server port and example local server URL (verification: `openspec/specs/configuration/spec.md`, `openspec/specs/cli/spec.md`, README files)

## Future Work

- Consider a separate proposal if the project later wants automatic port selection or collision-aware fallback behavior for `cflx server`.
