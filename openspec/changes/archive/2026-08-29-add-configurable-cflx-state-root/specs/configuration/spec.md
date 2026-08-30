## ADDED Requirements

### Requirement: Conflux State Base Directory Configuration

The orchestrator SHALL support an optional `state_base_dir` configuration key that selects the root for Conflux-owned persistent state, so an operator can move that state off the internal disk without changing process-wide XDG variables.

`state_base_dir` SHALL be parsed from JSONC, SHALL merge under the standard configuration-source precedence, and SHALL treat an empty string as unset exactly as `workspace_base_dir` does. When it is set, Conflux SHALL own `<state_base_dir>/cflx/`. Resolution precedence for the Conflux-owned state root SHALL be `state_base_dir`, then `XDG_STATE_HOME`, then the existing platform default `~/.local/state`.

The key SHALL scope Conflux-owned paths only. Conflux SHALL NOT set, change, or inject `XDG_STATE_HOME` or `XDG_DATA_HOME` into the environment inherited by the commands it starts, and SHALL NOT rewrite an operator's own configured environment values.

An explicitly configured `state_base_dir` SHALL be an absolute path. Shell expansion and relative paths SHALL NOT be supported. Each orchestration entrypoint SHALL load and validate configuration before logging initialization, and a configured root that is relative, unavailable, uncreatable, or unwritable SHALL fail startup with an actionable path diagnostic on stderr before any listener, lifecycle adapter, or AI child process starts. Conflux SHALL NOT silently fall back to another root in that case.

Conflux SHALL NOT migrate, copy, or clean up files under a previously resolved state root when the configured root changes.

#### Scenario: Configure the Conflux state root

- **WHEN** config contains `"state_base_dir": "/Volumes/BigDisk/cflx/state"`
- **THEN** Conflux owns `/Volumes/BigDisk/cflx/state/cflx/` as its persistent state root
- **AND** the value is parsed from JSONC alongside `workspace_base_dir` without either key affecting the other

#### Scenario: Absent or empty state root preserves existing behavior

- **GIVEN** `state_base_dir` is absent, or is set to an empty string
- **WHEN** the orchestrator resolves the Conflux-owned state root
- **THEN** it uses `${XDG_STATE_HOME}/cflx` when `XDG_STATE_HOME` is set to a non-empty value
- **AND** otherwise falls back to the platform default `~/.local/state/cflx`

#### Scenario: State root merges under standard source precedence

- **GIVEN** a lower-priority configuration source sets `state_base_dir`
- **WHEN** a higher-priority source omits the key
- **THEN** the lower-priority value is preserved
- **AND** a higher-priority source that sets the key overwrites it
- **AND** a higher-priority source that sets it to an empty string normalizes back to unset

#### Scenario: Configured state root does not change the child command environment

- **GIVEN** `state_base_dir` and `XDG_STATE_HOME` are both set
- **WHEN** Conflux starts an Apply, Acceptance, Archive, Resolve, or lifecycle child command
- **THEN** Conflux uses `state_base_dir` for its own state root
- **AND** the child command's environment overlay gains no `XDG_STATE_HOME` or `XDG_DATA_HOME` entry from the storage configuration
- **AND** an `XDG_STATE_HOME` the operator configured for child commands is passed through unchanged

#### Scenario: Relative configured state root is rejected

- **GIVEN** `state_base_dir` is set to a relative path
- **WHEN** Conflux resolves or validates the state root
- **THEN** validation fails with a message naming `state_base_dir`, the offending value, and the absolute-path requirement
- **AND** no directory is created for the rejected value

#### Scenario: Unusable configured state root fails closed at every orchestration entrypoint

- **GIVEN** `state_base_dir` is relative, unavailable, uncreatable, or unwritable
- **WHEN** the operator starts either orchestration entrypoint
- **THEN** configuration is loaded and validated before logging initialization
- **AND** startup exits non-zero with an actionable path diagnostic on stderr
- **AND** no listener, lifecycle adapter, or AI child process has been started
- **AND** Conflux does not fall back to `XDG_STATE_HOME` or the platform default

#### Scenario: Init templates document both storage roots

- **WHEN** the user generates a configuration template with `cflx init`
- **THEN** the generated example documents `workspace_base_dir` and `state_base_dir` as the two storage roots
- **AND** it states that `state_base_dir` takes precedence over `XDG_STATE_HOME` for Conflux-owned paths only
- **AND** it states that existing worktrees and logs are never migrated or cleaned up automatically

<!-- Expected canonical result after archive: `state_base_dir` is a first-class, absolute-only configuration key that scopes Conflux-owned persistent state, fails closed before anything starts, and never mutates the environment child commands inherit. -->
