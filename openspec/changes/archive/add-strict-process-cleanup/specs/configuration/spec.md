## ADDED Requirements

### Requirement: Strict Process Cleanup Configuration

The orchestrator configuration MAY define `command_strict_process_cleanup` as a boolean.

If the key is absent, the default MUST be `true`.

When `command_strict_process_cleanup` is `true`, the orchestrator MUST enforce strict post-completion cleanup of the spawned command's isolated process group/session.

When `command_strict_process_cleanup` is `false`, the orchestrator MAY skip strict post-completion cleanup after a successful command completion, but MUST still support cancellation/timeout cleanup.

#### Scenario: Default strict cleanup is enabled

- **GIVEN** no `command_strict_process_cleanup` key is present in the merged configuration
- **WHEN** cflx loads configuration
- **THEN** `command_strict_process_cleanup` is treated as `true`

#### Scenario: Strict cleanup can be disabled

- **GIVEN** `.cflx.jsonc` sets `command_strict_process_cleanup` to `false`
- **WHEN** cflx executes an agent command that completes successfully
- **THEN** cflx does not enforce strict post-completion cleanup
