## MODIFIED Requirements

### Requirement: Existing-owner client namespace

The CLI MUST provide `cflx client` as a client-only namespace for operating one existing repository owner. It MUST provide `status`, `mark`, `unmark`, `start`, `stop`, `force-stop`, `wait`, `subscribe`, and `mcp`. The nested `subscribe` namespace MUST provide proposal-scoped `set`, `get`, and `clear` intents over the proposal-subscription client implementation. CLI controls MUST use the same shared operator intents and subscription services as the TUI and MCP. Invoking a client command MUST NOT acquire the orchestration repository lock, bind an owner listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, or otherwise become an owner. `cflx run` MUST retain its existing explicit-target owner semantics.

On Windows, the client MUST open the Named Pipe with identification-only security quality of service, verify that the connected server owner SID equals the current process TokenUser SID, and send no request byte unless that verification succeeds. A mismatch MUST return a typed transport refusal without TCP fallback or transport retry.

The namespace MUST derive its default platform-local endpoint from the canonical Git common directory: a Unix-domain socket on Unix and a current-user/repository-scoped Windows Named Pipe on Windows. It MAY accept an explicit Unix socket override only on Unix. Authentication secrets MUST be read from a named environment variable rather than a literal argv value. Builds without the required local API support MUST reject the namespace before side effects.

#### Scenario: Client does not compete with the owner

- **GIVEN** a TUI process owns the repository and serves its default platform-local endpoint
- **WHEN** another process runs a client status, mark, lifecycle-control, wait, subscription, or MCP command
- **THEN** it connects as a client to the existing owner
- **AND** it does not acquire the repository lock or start another orchestration process

#### Scenario: Windows client uses the repository owner Named Pipe

- **GIVEN** a Windows owner serves the default Named Pipe for a repository
- **WHEN** a client selects that repository through its current directory or an absolute `--project-dir`
- **THEN** the client derives and connects to that repository's current-user-scoped pipe
- **AND** it uses the same typed command, observation, subscription, wait, and MCP contracts as the Unix client
- **AND** it does not probe or start a TCP listener

#### Scenario: Windows client rejects a pipe owned by another user

- **GIVEN** another Windows user has created the expected pipe name
- **WHEN** the client opens that pipe
- **THEN** it verifies the connected server owner SID before writing the HTTP request
- **AND** it returns a typed transport refusal without sending request bytes
- **AND** it does not retry over TCP or another endpoint

#### Scenario: Feature-disabled client fails before mutation

- **GIVEN** the binary lacks local remote-control support
- **WHEN** an operator invokes any `cflx client` command
- **THEN** the command exits non-zero with an actionable error
- **AND** it creates no repository lock, API endpoint, log, or workspace mutation

#### Scenario: Wait certifies evidence from the selected project

- **GIVEN** the client working directory is inside project A
- **AND** project A and project B contain the same change ID but different completion evidence
- **AND** a `cflx client wait` invocation supplies absolute project directory B
- **WHEN** project B's owner claims terminal success
- **THEN** completion is certified only from project B's repository evidence
- **AND** project A's repository is never consulted

#### Scenario: Platform-specific low-level overrides remain bounded

- **GIVEN** a client is built for Windows
- **WHEN** an operator attempts to use the Unix-only explicit socket override
- **THEN** argument parsing refuses the unsupported option before owner contact
- **AND** normal Windows routing remains available through current-directory or `--project-dir` selection

<!-- Expected canonical result after archive: cflx client is available through one repository-derived platform-local transport on both Unix and Windows without changing its command or result contract. -->

### Requirement: Existing-owner client MCP namespace

The CLI MUST provide `cflx client mcp` as a stdio Model Context Protocol server over the existing client-only control boundary. It MUST expose exactly `cflx_status`, `cflx_control`, and `cflx_subscribe`. It MUST NOT expose raw `/api/v2` command construction or become a second owner. `cflx_wait` is withdrawn only from MCP; `cflx client wait` remains the bounded CLI completion oracle.

The MCP adapter MUST use the same route resolution, authentication environment-variable references, shared operator intents, typed outcomes, and subscription service as the matching client CLI operations. Route resolution consists of a call-scoped absolute project directory, a call-scoped explicit Unix socket on Unix, a namespace-level default route, or current-working-directory repository discovery, in that precedence order. A selector supplied by one call MUST override the namespace-level default and MUST NOT mutate it. Mutual exclusion applies only when the same call supplies both `project_dir` and `unix_socket`.

Every closed MCP tool MUST accept optional non-empty string `project_dir` on every platform and optional `unix_socket` only on Unix. `project_dir` is the normal public selector; `unix_socket` is a Unix-only low-level override. `project_dir` MUST be an absolute path. A relative path or two selectors in the same call MUST produce the normal MCP `ToolError` / `isError` validation result before owner contact; this change MUST NOT add a new stable-envelope outcome. When neither call-scoped selector is supplied, the namespace-level default remains effective, otherwise current-working-directory discovery remains effective.

A project-directory route MUST resolve any directory inside a usable non-bare Git working tree, including a linked worktree, submodule, or canonicalized symlink, through the same canonical repository and Git common-directory derivation used by repository locking and default owner-socket resolution. It MUST derive both the absolute repository root and the platform-local owner endpoint from that selected project: `<git-common-dir>/cflx-api.sock` on Unix or the deterministic current-user/repository-scoped Named Pipe on Windows. The same immutable call-scoped route resolution MUST apply to all three tools. Route resolution MUST NOT mutate a repository, start an owner, infer a repository from a change ID, or persist a project registry. A missing path, non-directory, bare repository, or non-repository path MUST fail through the bounded validation channel before owner contact.

The MCP adapter MUST implement MCP initialization, initialized notification handling, ping, `tools/list`, and `tools/call` for a documented protocol revision. Before initialization it MUST accept only `initialize` and `ping`. Tool listing and calls become enabled after the adapter successfully responds to `initialize`; `notifications/initialized` MUST be accepted idempotently. It MUST reject request envelopes that do not identify JSON-RPC 2.0. Invalid request objects MUST receive an invalid-request response using a valid request ID or `null`; invalid notifications MUST receive no response. JSON-RPC batch arrays MUST be rejected as invalid requests because batch support is not advertised. Protocol errors and tool failures MUST be machine-readable and MUST NOT mix diagnostics into JSON-RPC stdout.

The stable client envelope MUST retain optional top-level `instance_id`, `execution_id`, and `change_id` fields without changing existing field meanings or exit codes. Mark responses MUST leave `execution_id` absent. Operations MUST use `control_mark`, `control_unmark`, `control_start`, `control_stop`, `control_force_stop`, `subscribe_set`, `subscribe_get`, and `subscribe_clear`. Stable success outcomes MUST distinguish `marked`, `unmarked`, `unchanged`, `accepted`, `subscribed`, `observed`, and `cleared`; retained typed refusals include `owner_not_running`, `owner_not_command_capable`, `owner_restarted`, `change_not_found`, `target_ineligible`, `revision_conflict`, `transport_not_permitted`, `unsupported_owner`, `partial_intent`, and `usage_error`. Owners without proposal-subscription capability MUST produce typed `unsupported_owner`, not a protocol error.

`cflx_control` MUST accept action `mark`, `unmark`, `start`, `stop`, or `force_stop`. Mark/unmark require one through 64 distinct `change_ids`; lifecycle actions MUST reject `change_ids` and consume authoritative owner state. `cflx_subscribe` MUST accept action `set`, `get`, or `clear` and one through 64 distinct `change_ids`. Set requires bounded non-empty callback argv; get and clear reject callback argv. Duplicate IDs, an empty set, or more than 64 IDs MUST fail as `usage_error` before owner contact. No tool may expose raw command construction, expected revision, idempotency keys, queue intent, shell interpretation, or hidden admission.

#### Scenario: MCP lists only the compact tools

- **WHEN** an initialized MCP client lists tools
- **THEN** it sees exactly `cflx_status`, `cflx_control`, and `cflx_subscribe`
- **AND** it cannot call historical enqueue, wait, or notify tools

#### Scenario: Two projects share one MCP server process

- **GIVEN** projects A and B have independent live Conflux owners
- **AND** one stdio MCP server process is already initialized
- **WHEN** a client calls a tool with absolute project directory A and then calls a tool with absolute project directory B
- **THEN** each call contacts only the owner derived from its own project directory
- **AND** no namespace-level or process-global route is changed

#### Scenario: Linked worktree resolves the common owner socket

- **GIVEN** a linked Git worktree belongs to a repository whose Conflux owner endpoint is derived from the Git common directory
- **WHEN** a tool receives an absolute directory inside that worktree as `project_dir`
- **THEN** it derives the canonical repository root and absolute Git common directory from that worktree
- **AND** contacts the platform-local owner endpoint derived from that common directory

#### Scenario: Call-scoped project overrides the namespace default socket

- **GIVEN** the MCP server was started with a namespace-level `--unix-socket` for project A
- **WHEN** a tool call supplies only absolute `project_dir` for project B
- **THEN** the call routes to project B
- **AND** no conflict refusal is returned

#### Scenario: Conflicting call selectors are rejected before contact

- **GIVEN** one Unix tool call supplies both `project_dir` and `unix_socket`
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
- **AND** otherwise derives the owner route from the MCP server current repository

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
- **THEN** it sees only the three closed client tools
- **AND** it cannot submit arbitrary command types, expected revisions, idempotency keys, queue intent, shell source, or workflow state mutations

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

#### Scenario: Windows MCP uses project-derived Named Pipe

- **GIVEN** an MCP server runs on Windows
- **WHEN** a tool supplies absolute `project_dir` or uses current-repository discovery
- **THEN** it derives the current-user/repository-scoped Named Pipe
- **AND** the Unix-only `unix_socket` selector is absent from that platform contract
- **AND** no TCP endpoint is probed

### Requirement: Direct client completion notification management

The CLI MUST expose `cflx client subscribe set`, `get`, and `clear` as direct shell-facing adapters over the same proposal-scoped subscription implementation used by MCP. Every operation MUST require one through 64 distinct change IDs and the expected owner instance ID, and MUST preserve complete owner/change coherence checks and typed outcomes. All three commands MUST support concise human output and `--json` through the stable client envelope contract.

The `cflx client` namespace MUST accept `--project-dir <ABSOLUTE_PATH>` as its normal explicit route and, on Unix only, `--unix-socket <PATH>` as its low-level route. Clap-level parsing MUST reject both explicit selectors together before owner contact. Set MUST accept required non-empty bounded callback argv after `--`, preserve each argument boundary exactly, and MAY opt into blocked-event delivery. It MUST NOT parse shell source, perform expansion, or implicitly invoke a shell. Set and clear MUST preserve the owner-only-local-transport mutation rule, permitting the mode-`0600` Unix socket and secured Windows Named Pipe while refusing TCP. Get MUST preserve transport-dependent argv redaction. These commands manage callback observability only and MUST NOT mutate workflow state or become an owner.

The embedded Conflux operation skill and `AGENTS.md` MUST document explicit subscribe commands, project-directory routing, the Unix-only low-level socket override, selector conflicts, owner-restart invalidation, notification-only behavior, and untrusted-event handling. They MUST NOT document automatic registration or agent/session resume.

#### Scenario: Operator registers callbacks for explicit proposals

- **GIVEN** project B has visible proposals `alpha` and `beta`
- **WHEN** the operator runs `cflx client --project-dir /absolute/project-b subscribe set alpha beta --instance-id <owner-instance> --blocked --json -- /absolute/callback --flag "one argument"`
- **THEN** the client stores the exact argv vector for both proposals without shell interpretation
- **AND** blocked-event delivery is enabled atomically for both
- **AND** stdout contains one successful `subscribe_set` envelope
- **AND** no workflow command or second owner is started

#### Scenario: CLI selectors conflict before contact

- **GIVEN** any repository state
- **WHEN** the operator supplies both `--project-dir` and `--unix-socket`
- **THEN** CLI parsing fails through the usage-error contract
- **AND** no owner request or workspace mutation occurs

#### Scenario: Empty callback command is rejected before owner access

- **GIVEN** any repository state
- **WHEN** the operator invokes subscribe set without a callback executable
- **THEN** CLI parsing fails with the human or JSON usage-error contract
- **AND** no owner request or workspace mutation occurs

#### Scenario: Operator inspects and clears named subscriptions

- **GIVEN** `alpha` and `beta` have subscriptions
- **WHEN** the operator gets and then clears both through one request
- **THEN** get reports both using transport redaction rules
- **AND** clear removes only those named subscriptions
- **AND** responses preserve stable subscribe operation and outcome names

#### Scenario: Expected owner incarnation changed

- **GIVEN** the caller retained instance ID `owner-a`
- **WHEN** a subscribe command supplies it after the socket serves `owner-b`
- **THEN** the command returns typed `owner_restarted`
- **AND** it does not register, inspect, or clear against the replacement owner

#### Scenario: TCP cannot mutate callback registration

- **GIVEN** an authenticated TCP connection to an owner
- **WHEN** a caller attempts subscribe set or clear
- **THEN** the owner returns typed `transport_not_permitted`
- **AND** no callback registration changes

#### Scenario: Installed operation skill teaches explicit subscription

- **GIVEN** an agent loads the embedded `cflx-run` skill
- **WHEN** it wants completion notification for existing or future proposal execution
- **THEN** the skill instructs it to explicitly use proposal-scoped subscribe set/get/clear
- **AND** it states that notification does not resume an agent automatically
