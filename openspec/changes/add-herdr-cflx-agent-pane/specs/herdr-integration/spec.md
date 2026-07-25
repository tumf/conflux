## ADDED Requirements

### Requirement: Herdr-managed cflx TUI pane

The repository SHALL provide a Herdr plugin pane entrypoint that starts `cflx tui` in the invoking Herdr workspace context without changing the pane's working directory.

#### Scenario: Open the Conflux TUI from Herdr

**Given**: the plugin is linked or installed, `cflx` is available, and a Herdr workspace is focused on a Conflux project
**When**: the user opens the plugin's `tui` pane entrypoint
**Then**: a managed terminal pane starts `cflx tui` in that project's working directory

### Requirement: Exact cflx agent identity

The Herdr plugin SHALL report its active TUI pane with the exact agent label `cflx` and SHALL release that report when the TUI terminates.

#### Scenario: TUI appears in the Agents list

**Given**: the plugin TUI pane has started successfully
**When**: Herdr renders its Agents list
**Then**: the pane is listed with the exact label `cflx`

#### Scenario: TUI exits

**Given**: the plugin previously reported the pane as agent `cflx`
**When**: `cflx tui` exits normally, fails, or receives a forwarded termination signal
**Then**: the plugin releases its lifecycle report and returns the TUI process exit status

### Requirement: Safe plugin launch failure

The plugin SHALL fail with a concise non-zero error when required Herdr pane context or the `cflx` executable is unavailable and SHALL NOT leave stale agent lifecycle authority.

#### Scenario: Required launch context is missing

**Given**: `HERDR_PANE_ID`, `HERDR_BIN_PATH`, or the `cflx` executable is unavailable
**When**: the plugin pane launcher runs
**Then**: it exits non-zero, reports the missing prerequisite, and leaves no unmatched active `cflx` report

### Requirement: Observational integration boundary

Herdr plugin state SHALL remain a non-authoritative observability output and SHALL NOT influence Conflux resume routing, acceptance gating, archive routing, or next-action decisions.

#### Scenario: Herdr state changes independently

**Given**: Herdr reports, clears, or loses the `cflx` agent entry
**When**: Conflux evaluates the next workflow action for unchanged workspace and git state
**Then**: Conflux chooses the same action regardless of the Herdr state
