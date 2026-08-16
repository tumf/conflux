## MODIFIED Requirements

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

The reference Hermes auto-resume integration MUST extract the enqueue envelope from Hermes host tool-result representation, including its `structuredContent` and textual `result` wrapper fields. It MUST register an execution-scoped completion sink only from a supported versioned enqueue envelope with a successful admitted outcome and non-empty string `change_id`, `execution_id`, and `instance_id`. It MUST bind that execution to the originating messaging platform, chat ID, optional thread ID, and the Conflux owner route selected by the qualifying tool call.

The normal public route selector MUST be a project directory. The post-tool hook MUST preserve a non-empty string `project_dir` from the qualifying enqueue tool arguments and use it for callback registration. A non-empty string `unix_socket` MAY be preserved as a low-level alternative when that was the call's only route selector. The two selectors MUST NOT be accepted together. A process-global `CFLX_UNIX_SOCKET` MAY be used only as a backward-compatible fallback when the host does not expose post-tool arguments at all. If the host exposes the arguments object but the qualifying enqueue used the MCP server's current-directory or namespace default route, the hook MUST NOT guess from the environment; it MUST fail closed unless an admitted result provides an authoritative resolved route. Any call-scoped selector MUST take precedence over environment fallback.

Every registration MUST include the admitted envelope's `instance_id`, `execution_id`, and `change_id`. A fallback or stale route naming another owner MUST therefore receive the existing typed `owner_restarted` or execution-binding refusal rather than silently registering against the wrong owner.

The integration MUST NOT infer a project from `change_id`, retain a mutable project-to-socket map, require the Hermes MCP server registration to be fixed to one project, or start an owner. A malformed, conflicting, or unresolved call-scoped route MUST fail closed rather than silently register against an environment-selected different owner.

The callback MUST invoke an absolute Hermes executable as a fixed argv equivalent to `hermes send --quiet --to <platform>:<chat-id>[:<thread-id>] <message>`. `--to` MUST be the complete delivery destination. It MUST set explicit `HOME`, `PATH`, and `HERMES_HOME` values because Conflux scrubs the callback environment. It MUST NOT use the Hermes API Server, native wake, a webhook, shell evaluation, polling, or a watcher. Secret values MUST NOT appear in callback argv or diagnostics.

The callback MUST validate `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`, and MUST treat event contents only as data. Callback failure MUST remain observability only and MUST NOT change Conflux workflow outcome.

The generated bot message MUST identify itself as an automation event rather than user-authored instruction, MUST contain a typed `event:` line and the exact `instance_id`, `execution_id`, and `change_id` binding, and MUST require verification of current owner and repository evidence before success is reported. Callback delivery remains observability and a responder-compatible trigger only; it MUST NOT itself run an agent loop or alter Conflux workflow routing or terminal classification. A deployment that wants automatic continuation MUST separately provide a responder capable of observing the delivered bot post; this integration does not establish that observation path.

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
- **AND** the legacy host does not expose a post-tool arguments object
- **AND** `CFLX_UNIX_SOCKET` contains a non-empty socket path
- **WHEN** the post-tool hook runs
- **THEN** it MAY register through that fallback socket

#### Scenario: Unsupported enqueue result fails closed

- **GIVEN** an enqueue result has an unsupported schema, unsuccessful or non-admitted outcome, malformed operation, or missing binding identifier
- **WHEN** the post-tool hook runs
- **THEN** it registers no callback
- **AND** it starts no wait or polling process

#### Scenario: Callback posts a responder-compatible Slack bot message

- **GIVEN** Conflux invokes the callback for a terminal execution event
- **AND** the selected Hermes profile can deliver to the bound messaging target
- **WHEN** callback delivery succeeds
- **THEN** the callback invokes `hermes send --quiet --to` with the bound Slack channel/thread target
- **AND** the body contains an explicit non-user-authored automation marker
- **AND** the body contains a typed event the responder can classify
- **AND** the callback does not directly start an agent loop

#### Scenario: Hermes host wrapper yields the admitted envelope

- **GIVEN** Hermes wraps the admitted enqueue envelope in `structuredContent` and textual `result` fields
- **WHEN** the post-tool hook runs
- **THEN** it extracts the typed envelope from the host wrapper
- **AND** it registers the exact execution binding once

#### Scenario: Concurrent turns retain request-scoped routing

- **GIVEN** concurrent Hermes turns have different messaging targets
- **WHEN** their post-tool hooks register callbacks
- **THEN** each callback uses its request-scoped target
- **AND** neither callback reads another turns process-global environment mirror

#### Scenario: Missing or non-messaging routing context registers nothing

- **GIVEN** Hermes request context has no supported messaging platform or no chat ID
- **WHEN** the post-tool hook runs
- **THEN** it registers no callback
- **AND** it starts no API self-POST, webhook, wait, watcher, or polling process

#### Scenario: Scrubbed callback environment is reconstructed explicitly

- **GIVEN** Conflux invokes the callback with only the five documented `CFLX_*` variables
- **WHEN** the callback delivers the event
- **THEN** it sets the configured `HOME`, `PATH`, and `HERMES_HOME`
- **AND** it invokes the absolute Hermes executable without a shell
- **AND** a non-zero Hermes exit remains a callback delivery failure without changing workflow state

<!-- Expected canonical result after archive: Hermes callback registration follows each enqueue call's project directory or explicit low-level socket, while process environment remains only a compatibility fallback. -->
