## MODIFIED Requirements

### Requirement: Existing-owner client MCP namespace

The CLI MUST provide `cflx client mcp` as a stdio Model Context Protocol server over the existing client-only intent boundary. It MUST expose closed tools for coherent status, enqueue, truthful wait, and completion-sink set/get/clear. It MUST NOT expose raw `/api/v2` command construction or become a second owner.

The MCP adapter MUST use the same route resolution, authentication environment-variable references, intent routing, typed outcomes, and completion oracle as `cflx client status`, `enqueue`, and `wait`. Route resolution consists of a call-scoped absolute project directory, a call-scoped explicit Unix socket, a namespace-level default route, or current-working-directory repository discovery, in that precedence order. A selector supplied by one call MUST override the namespace-level default and MUST NOT mutate it. Mutual exclusion applies only when the same call supplies both `project_dir` and `unix_socket`.

Every closed MCP tool MUST accept optional non-empty string `project_dir` and `unix_socket` selectors. `project_dir` is the normal public selector; `unix_socket` is the low-level override. `project_dir` MUST be an absolute path. A relative path or two selectors in the same call MUST produce the normal MCP `ToolError` / `isError` validation result before owner contact; this change MUST NOT add a new stable-envelope outcome. When neither call-scoped selector is supplied, the namespace-level default remains effective, otherwise current-working-directory discovery remains effective.

A project-directory route MUST resolve any directory inside a usable non-bare Git working tree, including a linked worktree, submodule, or canonicalized symlink, through the same canonical repository and Git common-directory derivation used by repository locking and default owner-socket resolution. It MUST derive both the absolute repository root used by repository-evidence operations and `<git-common-dir>/cflx-api.sock` from that selected project. `cflx_wait` completion certification MUST use only the selected project's repository root and MUST NOT consult the MCP server process's current repository when another project is selected.

The same route resolution MUST apply to `cflx_status`, `cflx_enqueue`, `cflx_wait`, `cflx_notify_set`, `cflx_notify_get`, and `cflx_notify_clear`. Route resolution MUST NOT mutate a repository, start an owner, infer a repository from a change ID, or persist a project registry. A missing path, non-directory, bare repository, or non-repository path MUST fail through the bounded validation channel before owner contact.

The MCP adapter MUST implement MCP initialization, initialized notification handling, ping, `tools/list`, and `tools/call` for a documented protocol revision. Before initialization it MUST accept only `initialize` and `ping`. Tool listing and calls become enabled after the adapter successfully responds to `initialize`; `notifications/initialized` MUST be accepted idempotently. It MUST reject request envelopes that do not identify JSON-RPC 2.0. Invalid request objects MUST receive an invalid-request response using a valid request ID or `null`; invalid notifications MUST receive no response. JSON-RPC batch arrays MUST be rejected as invalid requests because batch support is not advertised. Protocol errors and tool failures MUST be machine-readable and MUST NOT mix diagnostics into JSON-RPC stdout.

The stable client envelope MUST add optional top-level `instance_id`, `execution_id`, and `change_id` fields without changing existing field meanings or exit codes. Notify operations MUST use stable operation and outcome names. Owners without the execution-sink capability MUST produce a typed unsupported-owner failure rather than a protocol error.

#### Scenario: MCP enqueues into the existing TUI

- **GIVEN** a long-lived TUI owns the selected repository and serves its local Unix socket
- **WHEN** an initialized MCP host calls `cflx_enqueue` for eligible change `alpha`
- **THEN** the adapter submits the same high-level intent as `cflx client enqueue alpha`
- **AND** it returns the admitted owner, execution, and change binding
- **AND** it does not acquire the owner lock or start another scheduler owner

#### Scenario: Two projects share one MCP server process

- **GIVEN** projects A and B have independent live Conflux owners
- **AND** one stdio MCP server process is already initialized
- **WHEN** a client calls a tool with absolute project directory A and then calls a tool with absolute project directory B
- **THEN** each call contacts only the owner derived from its own project directory
- **AND** no namespace-level or process-global route is changed

#### Scenario: Linked worktree resolves the common owner socket

- **GIVEN** a linked Git worktree belongs to a repository whose Conflux owner socket is under the Git common directory
- **WHEN** a tool receives an absolute directory inside that worktree as `project_dir`
- **THEN** it derives the canonical repository root and absolute Git common directory from that worktree
- **AND** contacts the owner socket under that common directory

#### Scenario: Wait certifies evidence from the selected project

- **GIVEN** the MCP server current directory is inside project A
- **AND** project A and project B contain the same change ID but different completion evidence
- **AND** a `cflx_wait` call supplies absolute project directory B
- **WHEN** project B's owner claims terminal success
- **THEN** completion is certified only from project B's repository evidence
- **AND** project A's repository is never consulted

#### Scenario: Call-scoped project overrides the namespace default socket

- **GIVEN** the MCP server was started with a namespace-level `--unix-socket` for project A
- **WHEN** a tool call supplies only absolute `project_dir` for project B
- **THEN** the call routes to project B
- **AND** no conflict refusal is returned

#### Scenario: Conflicting call selectors are rejected before contact

- **GIVEN** one tool call supplies both `project_dir` and `unix_socket`
- **WHEN** route validation runs
- **THEN** the tool returns the normal MCP validation error result without adding a stable-envelope outcome
- **AND** no owner socket is contacted

#### Scenario: Explicit socket remains a low-level override

- **GIVEN** a tool call supplies `unix_socket` and omits `project_dir`
- **WHEN** the operation runs
- **THEN** it uses the explicit socket with existing authentication and envelope behavior

#### Scenario: Omitted selector preserves default behavior

- **GIVEN** a tool call supplies neither selector
- **WHEN** the operation runs
- **THEN** it uses the namespace-level default when configured
- **AND** otherwise derives the owner route and repository evidence root from the MCP server current repository as before

#### Scenario: Invalid project directory fails without mutation

- **GIVEN** `project_dir` is relative, missing, not a directory, bare, or not a usable Git working tree
- **WHEN** route validation runs
- **THEN** the tool returns a bounded validation error
- **AND** it starts no owner, contacts no fallback owner, and performs no repository mutation

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

<!-- Expected canonical result after archive: the client MCP namespace routes all six tools by one immutable call-scoped project or socket selector while keeping namespace and current-directory defaults compatible. -->

## MODIFIED Requirements

### Requirement: Direct client completion notification management

The CLI MUST expose `cflx client notify set`, `get`, and `clear` as direct shell-facing adapters over the same execution-scoped completion-sink implementation used by `cflx client mcp`. Every operation MUST require a change ID and execution ID, MAY accept the expected owner instance ID, and MUST preserve the complete owner/execution/change coherence checks and typed outcomes. All three commands MUST support concise human output and `--json` through the stable client envelope contract.

The `cflx client` namespace MUST accept `--project-dir <ABSOLUTE_PATH>` as its normal explicit route and `--unix-socket <PATH>` as its low-level route. Clap-level parsing MUST reject both explicit selectors together before owner contact. The selected project MUST provide both the owner socket and repository evidence root to every client subcommand. `set` MUST accept a required non-empty callback argv after `--`, preserve each argument boundary exactly, and MAY opt into blocked-event delivery. It MUST NOT parse shell source, perform expansion, or implicitly invoke a shell. Set and clear MUST preserve the existing Unix-socket-only mutation transport rule after project resolution. Get MUST preserve transport-dependent callback redaction. These commands MUST manage callback observability only and MUST NOT mutate workflow state or become an owner.

The repository's embedded Conflux operation skill and `AGENTS.md` MUST document the direct CLI commands as the default shell-facing path for registering, inspecting, and clearing completion callbacks, together with project-directory routing, the low-level socket override, selector conflict behavior, and truthful wait evidence selection. They MUST retain the MCP tool path as an alternative for MCP-only hosts and MUST preserve the same durable-callback and untrusted-event safety guidance.

#### Scenario: Operator registers one callback for an explicit project

- **GIVEN** a command-capable TUI owns execution `exec-1` for change `alpha` in project B
- **WHEN** the operator runs `cflx client --project-dir /absolute/project-b notify set alpha exec-1 --instance-id <owner-instance> --blocked --json -- /absolute/callback --flag "one argument"`
- **THEN** the client resolves project B's owner and stores the exact argv vector without shell interpretation
- **AND** blocked-event delivery is enabled
- **AND** the owner validates the complete instance, execution, and change binding
- **AND** stdout contains one successful `notify_set` envelope
- **AND** no workflow command or second owner is started

#### Scenario: CLI selectors conflict before contact

- **GIVEN** any repository state
- **WHEN** the operator supplies both `--project-dir` and `--unix-socket` on the `cflx client` namespace
- **THEN** CLI parsing fails through the existing usage-error contract
- **AND** no owner request or workspace mutation occurs

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

<!-- Expected canonical result after archive: direct client operations select one project route explicitly and carry its owner and evidence identity together. -->
