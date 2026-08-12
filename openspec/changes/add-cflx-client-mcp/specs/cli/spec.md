## MODIFIED Requirements

### Requirement: Existing-owner client namespace

The CLI MUST provide `cflx client` as a client-only namespace for operating one existing repository owner. It MUST provide only `status`, `enqueue`, `wait`, and `mcp`. Invoking a client command MUST NOT acquire the orchestration repository lock, bind an owner listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, or otherwise become an owner. `cflx run` MUST retain its existing explicit-target owner semantics.

The namespace MUST derive the default Unix socket from the canonical Git common directory and MAY accept an explicit socket override. Authentication secrets MUST be read from a named environment variable rather than a literal argv value. Builds without the required local API support MUST reject the namespace before side effects.

#### Scenario: Client does not compete with the owner

- **GIVEN** a TUI process owns the repository and serves its default Unix socket
- **WHEN** another process runs `cflx client status --json` or `cflx client mcp`
- **THEN** it connects as a client to the existing owner
- **AND** it does not acquire the repository lock or start another orchestration process

#### Scenario: Feature-disabled client fails before mutation

- **GIVEN** the binary lacks local remote-control support
- **WHEN** an operator invokes any `cflx client` command
- **THEN** the command exits non-zero with an actionable error
- **AND** it creates no repository lock, API socket, log, or workspace mutation

## ADDED Requirements

### Requirement: Existing-owner client MCP namespace

The CLI MUST provide `cflx client mcp` as a stdio Model Context Protocol server over the existing client-only intent boundary. It MUST expose closed tools for coherent status, enqueue, truthful wait, and completion-sink set/get/clear. It MUST NOT expose raw `/api/v2` command construction or become a second owner.

The MCP adapter MUST use the same Unix-socket resolution, authentication environment-variable references, intent routing, typed outcomes, and completion oracle as `cflx client status`, `enqueue`, and `wait`. It MUST implement MCP initialization, initialized notification handling, ping, `tools/list`, and `tools/call` for a documented protocol revision. Protocol errors and tool failures MUST be machine-readable and MUST NOT mix diagnostics into JSON-RPC stdout.

The stable client envelope MUST add optional top-level `instance_id`, `execution_id`, and `change_id` fields without changing existing field meanings or exit codes. Notify operations MUST use stable operation and outcome names. Owners without the execution-sink capability MUST produce a typed unsupported-owner failure rather than a protocol error.

#### Scenario: MCP enqueues into the existing TUI

- **GIVEN** a long-lived TUI owns the repository and serves its local Unix socket
- **WHEN** an MCP host calls `cflx_enqueue` for eligible change `alpha`
- **THEN** the adapter submits the same high-level intent as `cflx client enqueue alpha`
- **AND** it returns the admitted owner, execution, and change binding
- **AND** it does not acquire the owner lock or start another scheduler owner

#### Scenario: MCP stdout remains protocol-only

- **GIVEN** an MCP host communicates over stdio
- **WHEN** a tool succeeds, fails validation, or cannot reach the owner
- **THEN** stdout contains only valid MCP JSON-RPC frames
- **AND** diagnostics are isolated from the protocol stream

#### Scenario: Raw workflow commands are not exposed

- **GIVEN** the owner supports revisioned `/api/v2` commands
- **WHEN** an MCP client lists available tools
- **THEN** it sees only the closed intent-shaped client tools
- **AND** it cannot submit arbitrary command types, expected revisions, idempotency keys, execution marks, queue intent, shell source, or workflow state mutations

### Requirement: MCP tool calls remain bounded

The MCP adapter MUST NOT keep an enqueue tool call open for the lifetime of a change. Enqueue MUST return after admission settlement. `cflx_wait` MUST retain an explicit bounded timeout, and asynchronous continuation MUST use an execution-scoped completion sink rather than an unbounded MCP request.

#### Scenario: Long-lived TUI does not hold enqueue open

- **GIVEN** the TUI remains alive after admitting `alpha`
- **WHEN** `cflx_enqueue` settles successfully
- **THEN** the MCP call returns the execution binding immediately after admission
- **AND** proposal completion is observed separately through wait or notification
