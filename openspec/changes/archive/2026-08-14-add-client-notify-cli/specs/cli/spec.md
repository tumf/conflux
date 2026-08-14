## MODIFIED Requirements

### Requirement: Existing-owner client namespace

The CLI MUST provide `cflx client` as a client-only namespace for operating one existing repository owner. It MUST provide `status`, `enqueue`, `wait`, `notify`, and `mcp`. The nested `notify` namespace MUST provide execution-scoped `set`, `get`, and `clear` intents over the existing completion-sink client implementation. Invoking a client command MUST NOT acquire the orchestration repository lock, bind an owner listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, or otherwise become an owner. `cflx run` MUST retain its existing explicit-target owner semantics.

The namespace MUST derive the default Unix socket from the canonical Git common directory and MAY accept an explicit socket override. Authentication secrets MUST be read from a named environment variable rather than a literal argv value. Builds without the required local API support MUST reject the namespace before side effects.

#### Scenario: Client does not compete with the owner

- **GIVEN** a TUI process owns the repository and serves its default Unix socket
- **WHEN** another process runs `cflx client status --json`, `cflx client notify get alpha <execution-id> --json`, or `cflx client mcp`
- **THEN** it connects as a client to the existing owner
- **AND** it does not acquire the repository lock or start another orchestration process

#### Scenario: Feature-disabled client fails before mutation

- **GIVEN** the binary lacks local remote-control support
- **WHEN** an operator invokes any `cflx client` command
- **THEN** the command exits non-zero with an actionable error
- **AND** it creates no repository lock, API socket, log, or workspace mutation

## ADDED Requirements

### Requirement: Direct client completion notification management

The CLI MUST expose `cflx client notify set`, `get`, and `clear` as direct shell-facing adapters over the same execution-scoped completion-sink implementation used by `cflx client mcp`. Every operation MUST require a change ID and execution ID, MAY accept the expected owner instance ID, and MUST preserve the complete owner/execution/change coherence checks and typed outcomes. All three commands MUST support concise human output and `--json` through the stable client envelope contract.

`set` MUST accept a required non-empty callback argv after `--`, preserve each argument boundary exactly, and MAY opt into blocked-event delivery. It MUST NOT parse shell source, perform expansion, or implicitly invoke a shell. Set and clear MUST preserve the existing Unix-socket-only mutation rule. Get MUST preserve transport-dependent callback redaction. These commands MUST manage callback observability only and MUST NOT mutate workflow state or become an owner.

The repository's embedded Conflux operation skill MUST document the direct CLI commands as the default shell-facing path for registering, inspecting, and clearing completion callbacks. It MUST retain the MCP tool path as an alternative for MCP-only hosts and MUST preserve the same durable-callback and untrusted-event safety guidance.

#### Scenario: Operator registers one callback from the shell

- **GIVEN** a command-capable TUI owns execution `exec-1` for change `alpha`
- **WHEN** the operator runs `cflx client notify set alpha exec-1 --blocked -- /absolute/callback --flag "one argument" --json`
- **THEN** the owner stores the exact argv vector without shell interpretation
- **AND** blocked-event delivery is enabled
- **AND** stdout contains one successful `notify_set` envelope
- **AND** no workflow command or second owner is started

#### Scenario: Empty callback command is rejected before owner access

- **GIVEN** any repository state
- **WHEN** the operator invokes `cflx client notify set alpha exec-1 --` without a callback executable
- **THEN** CLI parsing fails with the existing human or JSON usage-error contract
- **AND** no owner request or workspace mutation occurs

#### Scenario: Operator inspects and clears one callback

- **GIVEN** execution `exec-1` for change `alpha` has a registered callback
- **WHEN** the operator runs `cflx client notify get alpha exec-1 --json` and then `cflx client notify clear alpha exec-1 --json`
- **THEN** get reports the current subscription using the existing transport redaction rules
- **AND** clear removes only that execution's callback
- **AND** both responses preserve the stable notify operation and outcome names

#### Scenario: Expected owner incarnation changed

- **GIVEN** the caller retained the instance ID that admitted execution `exec-1`
- **WHEN** a notify CLI command supplies that instance ID after the socket begins serving a different owner
- **THEN** the command returns typed `owner_restarted` non-zero
- **AND** it does not register, inspect, or clear a callback against the replacement owner

#### Scenario: TCP cannot mutate callback registration

- **GIVEN** an authenticated TCP connection to an owner
- **WHEN** a caller attempts the equivalent direct notify set or clear operation
- **THEN** the owner returns typed `transport_not_permitted`
- **AND** no callback registration changes

#### Scenario: Installed operation skill teaches the direct CLI path

- **GIVEN** an agent loads the embedded `cflx-run` skill in a shell-capable environment
- **WHEN** it delegates a long-running change to an existing TUI owner
- **THEN** the skill instructs it to use `cflx client notify set` with the admitted execution binding
- **AND** it documents `get` and `clear` for inspection and cancellation of the callback registration
- **AND** MCP remains documented as an alternative rather than the only notification interface

<!-- Expected canonical result after archive: the client namespace gains direct, argv-safe execution notification set/get/clear commands while retaining the existing owner, transport, and stable-envelope contracts. -->
