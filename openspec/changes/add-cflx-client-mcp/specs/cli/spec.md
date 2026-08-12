## ADDED Requirements

### Requirement: Existing-owner client MCP namespace

The CLI MUST provide `cflx client mcp` as a stdio Model Context Protocol server over the existing client-only intent boundary. It MUST expose closed tools for coherent status, enqueue, truthful wait, and completion-sink set/get/clear. It MUST NOT acquire the orchestration repository lock, bind an owner listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, expose raw `/api/v2` command construction, or become a second owner.

The MCP adapter MUST use the same Unix-socket resolution, authentication environment-variable references, intent routing, typed outcomes, and completion oracle as `cflx client status`, `enqueue`, and `wait`. Protocol errors and tool failures MUST be machine-readable and MUST NOT mix diagnostics into JSON-RPC stdout.

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
