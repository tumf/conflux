## MODIFIED Requirements

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

The reference Hermes auto-resume integration MUST extract the enqueue envelope from Hermes host tool-result representation, including its `structuredContent` and textual `result` wrapper fields. It MUST register an execution-scoped completion sink only from a supported versioned enqueue envelope with a successful admitted outcome and non-empty string `change_id`, `execution_id`, and `instance_id`. It MUST bind that execution to the originating messaging platform, chat ID, optional thread ID, and the Conflux owner route selected by the qualifying tool call.

The normal public route selector MUST be a project directory. The post-tool hook MUST preserve a non-empty string `project_dir` from the qualifying enqueue tool arguments and use it for callback registration. A non-empty string `unix_socket` MAY be preserved as a low-level alternative when that was the call's only route selector. The two selectors MUST NOT be accepted together. A process-global `CFLX_UNIX_SOCKET` MAY be used only as a backward-compatible fallback when the host exposes neither call-scoped selector. Any call-scoped selector MUST take precedence over environment fallback.

The integration MUST NOT infer a project from `change_id`, retain a mutable project-to-socket map, require the Hermes MCP server registration to be fixed to one project, or start an owner. A malformed, conflicting, or unresolved call-scoped route MUST fail closed rather than silently register against an environment-selected different owner.

The callback MUST invoke an absolute Hermes executable as a fixed argv equivalent to `hermes send --quiet --to <platform>:<chat-id>[:<thread-id>] <message>`. `--to` MUST be the complete delivery destination. It MUST set explicit `HOME`, `PATH`, and `HERMES_HOME` values because Conflux scrubs the callback environment. It MUST NOT use the Hermes API Server, native wake, a webhook, shell evaluation, polling, or a watcher. Secret values MUST NOT appear in callback argv or diagnostics.

The callback MUST validate `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`, and MUST treat event contents only as data. Callback failure MUST remain observability only and MUST NOT change Conflux workflow outcome.

The generated bot message MUST identify itself as an automation event rather than user-authored instruction, MUST contain a typed `event:` line and the exact `instance_id`, `execution_id`, and `change_id` binding, and MUST require verification of current owner and repository evidence before success is reported. Callback delivery remains observability and a responder-compatible trigger only; it MUST NOT itself run an agent loop or alter Conflux workflow routing or terminal classification.

The post-tool hook MUST be observational: refusal, malformed output, missing routing, or registration failure MUST NOT replace the tool result or fail the Hermes turn, and diagnostics MUST remain bounded and secret-free. Request-scoped messaging and project routing context MUST be authoritative; process-global environment mirrors MAY be used only as compatibility fallbacks when no corresponding request-scoped value was supplied.

#### Scenario: Admitted enqueue registers the originating messaging thread

- **GIVEN** Hermes completes `cflx_enqueue` or an MCP name such as `mcp__cflx__cflx_enqueue`
- **AND** the result is a supported successful admitted envelope with complete binding identifiers
- **AND** Hermes request context identifies a messaging platform and chat, with an optional thread
- **AND** the tool arguments identify the Conflux project directory
- **WHEN** the post-tool hook runs
- **THEN** it registers one callback for that exact execution binding through the same call-scoped project route
- **AND** the callback argv contains the fixed messaging target and explicit profile environment paths but no secret value

#### Scenario: Two project calls retain independent owner routing

- **GIVEN** one Hermes process calls enqueue for project A using project directory A
- **AND** it calls enqueue for project B using project directory B
- **WHEN** each post-tool hook registers its completion sink
- **THEN** project A registration reaches only project A's owner
- **AND** project B registration reaches only project B's owner
- **AND** neither call mutates process-global routing for the other

#### Scenario: Call-scoped project overrides compatibility fallback

- **GIVEN** `CFLX_UNIX_SOCKET` names project A's owner
- **AND** a qualifying enqueue call names project directory B
- **WHEN** the post-tool hook registers the sink
- **THEN** it resolves and uses project B's route
- **AND** it does not contact project A's owner

#### Scenario: Low-level socket route remains supported

- **GIVEN** a qualifying enqueue call supplies `unix_socket` and omits `project_dir`
- **WHEN** the post-tool hook registers the sink
- **THEN** it preserves that socket route for `notify set`

#### Scenario: Conflicting call-scoped routes fail closed

- **GIVEN** a qualifying enqueue call contains both `project_dir` and `unix_socket`
- **WHEN** the post-tool hook runs
- **THEN** it registers no completion sink
- **AND** it does not fall back to process-global routing
- **AND** it leaves the original tool result and Hermes turn unchanged

#### Scenario: Legacy host may use environment fallback

- **GIVEN** a qualifying admitted enqueue result
- **AND** the host supplies neither call-scoped selector
- **AND** `CFLX_UNIX_SOCKET` contains a non-empty socket path
- **WHEN** the post-tool hook runs
- **THEN** it MAY register through that fallback socket

<!-- Expected canonical result after archive: Hermes callback registration follows each enqueue call's project directory or explicit low-level socket, while process environment remains only a compatibility fallback. -->

## ADDED Requirements

### Requirement: Client MCP routes each call by project directory

Every Conflux client MCP tool MUST accept an optional non-empty string `project_dir` as the normal public route selector. For each call, Conflux MUST resolve that path as a usable Git repository or linked worktree, obtain the absolute Git common directory using repository-aware Git semantics, and target `<git-common-dir>/cflx-api.sock`.

Every tool MAY also accept `unix_socket` as a low-level route override. `project_dir` and `unix_socket` MUST be mutually exclusive. When both are supplied, the tool MUST return a typed validation refusal before attempting owner contact. When neither is supplied, the existing MCP server current-working-directory route MUST remain in effect.

The same route resolution MUST apply to `cflx_status`, `cflx_enqueue`, `cflx_wait`, `cflx_notify_set`, `cflx_notify_get`, and `cflx_notify_clear`. Route resolution MUST NOT mutate the repository, start an owner, infer a repository from a change ID, or persist a project registry outside the workspace.

#### Scenario: Two projects share one MCP server process

- **GIVEN** projects A and B have independent live Conflux owners
- **AND** one stdio MCP server was started without a fixed project socket
- **WHEN** a client calls a tool with project directory A and then calls a tool with project directory B
- **THEN** each call contacts only the owner derived from its own project directory
- **AND** no process-global route is changed

#### Scenario: Linked worktree resolves the common owner socket

- **GIVEN** a linked Git worktree belongs to a repository whose Conflux owner socket is under the Git common directory
- **WHEN** a tool receives that worktree as `project_dir`
- **THEN** it resolves the absolute Git common directory
- **AND** contacts the owner socket under that common directory

#### Scenario: Conflicting selectors are rejected before contact

- **GIVEN** a tool call supplies both `project_dir` and `unix_socket`
- **WHEN** route validation runs
- **THEN** the tool returns a typed validation refusal
- **AND** no owner socket is contacted

#### Scenario: Explicit socket remains a low-level override

- **GIVEN** a tool call supplies `unix_socket` and omits `project_dir`
- **WHEN** the operation runs
- **THEN** it uses the explicit socket with existing authentication and envelope behavior

#### Scenario: Omitted selector preserves current-directory behavior

- **GIVEN** an MCP server starts inside a usable Git repository
- **AND** a tool call supplies neither selector
- **WHEN** the operation runs
- **THEN** it derives the owner socket from the server process's current repository as before

#### Scenario: Invalid project directory fails without mutation

- **GIVEN** `project_dir` is missing, not a directory, or not a usable Git repository/worktree
- **WHEN** route resolution runs
- **THEN** the tool returns a bounded typed refusal
- **AND** it starts no owner and performs no repository mutation

<!-- Expected canonical result after archive: all six client MCP tools use repository-aware project-directory routing per call, with explicit socket and CWD compatibility retained. -->
