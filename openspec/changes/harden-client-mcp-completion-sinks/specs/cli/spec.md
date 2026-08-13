## MODIFIED Requirements

### Requirement: Existing-owner client MCP namespace

The CLI MUST provide `cflx client mcp` as a stdio Model Context Protocol server over the existing client-only intent boundary. It MUST expose closed tools for coherent status, enqueue, truthful wait, and completion-sink set/get/clear. It MUST NOT expose raw `/api/v2` command construction or become a second owner.

The MCP adapter MUST use the same Unix-socket resolution, authentication environment-variable references, intent routing, typed outcomes, and completion oracle as `cflx client status`, `enqueue`, and `wait`. It MUST implement MCP initialization, initialized notification handling, ping, `tools/list`, and `tools/call` for a documented protocol revision. Before initialization it MUST accept only `initialize` and `ping`. Tool listing and calls become enabled after the adapter successfully responds to `initialize`; `notifications/initialized` MUST be accepted idempotently. It MUST reject request envelopes that do not identify JSON-RPC 2.0. Invalid request objects MUST receive an invalid-request response using a valid request ID or `null`; invalid notifications MUST receive no response. JSON-RPC batch arrays MUST be rejected as invalid requests because batch support is not advertised. Protocol errors and tool failures MUST be machine-readable and MUST NOT mix diagnostics into JSON-RPC stdout.

The stable client envelope MUST add optional top-level `instance_id`, `execution_id`, and `change_id` fields without changing existing field meanings or exit codes. Notify operations MUST use stable operation and outcome names. Owners without the execution-sink capability MUST produce a typed unsupported-owner failure rather than a protocol error.

#### Scenario: MCP enqueues into the existing TUI

- **GIVEN** a long-lived TUI owns the repository and serves its local Unix socket
- **WHEN** an initialized MCP host calls `cflx_enqueue` for eligible change `alpha`
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
- **WHEN** an initialized MCP client lists available tools
- **THEN** it sees only the closed intent-shaped client tools
- **AND** it cannot submit arbitrary command types, expected revisions, idempotency keys, execution marks, queue intent, shell source, or workflow state mutations

#### Scenario: Tool calls require initialization

- **GIVEN** an MCP peer has not completed initialization
- **WHEN** it requests `tools/list` or `tools/call`
- **THEN** the adapter returns a machine-readable protocol error
- **AND** no owner request or workflow mutation occurs

#### Scenario: Non-JSON-RPC request is rejected

- **GIVEN** a newline-delimited JSON object omits `jsonrpc: "2.0"` or supplies another version
- **WHEN** the MCP adapter receives it
- **THEN** the adapter returns a JSON-RPC invalid-request error
- **AND** no tool is dispatched

### Requirement: MCP tool calls remain bounded

The MCP adapter MUST NOT keep an enqueue tool call open for the lifetime of a change. Enqueue MUST return after admission settlement. `cflx_wait` MUST retain an explicit bounded timeout, and asynchronous continuation MUST use an execution-scoped completion sink rather than an unbounded MCP request. Newline-delimited input framing MUST enforce its memory bound while bytes are read, including when a peer never sends a newline.
An oversized frame or invalid UTF-8 frame MUST terminate the stdio session without dispatching a tool or owner request; the adapter does not attempt stream resynchronization.

#### Scenario: Long-lived TUI does not hold enqueue open

- **GIVEN** the TUI remains alive after admitting `alpha`
- **WHEN** `cflx_enqueue` settles successfully
- **THEN** the MCP call returns the execution binding immediately after admission
- **AND** proposal completion is observed separately through wait or notification

#### Scenario: Newline-free oversized frame remains bounded

- **GIVEN** a peer sends more than the configured frame limit without a newline
- **WHEN** the MCP adapter reads stdin
- **THEN** retained input remains bounded by the configured limit plus fixed framing overhead
- **AND** no tool or owner request is dispatched
- **AND** the stdio session terminates without interpreting the remaining bytes as another frame
