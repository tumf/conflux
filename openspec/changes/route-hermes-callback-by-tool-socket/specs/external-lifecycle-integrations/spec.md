## MODIFIED Requirements

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

The reference Hermes auto-resume integration MUST extract the enqueue envelope from Hermes host tool-result representation, including its `structuredContent` and textual `result` wrapper fields. It MUST register an execution-scoped completion sink only from a supported versioned enqueue envelope with a successful admitted outcome and non-empty string `change_id`, `execution_id`, and `instance_id`. It MUST bind that execution to the originating messaging platform, chat ID, optional thread ID, and the Conflux owner Unix socket selected by the qualifying tool call.

The post-tool hook MUST treat a non-empty string `unix_socket` in the qualifying enqueue tool arguments as authoritative for callback registration. A process-global `CFLX_UNIX_SOCKET` MAY be used only as a backward-compatible fallback when the host does not provide a call-scoped socket. A call-scoped socket MUST override that fallback. The integration MUST NOT infer a project socket from `change_id`, retain a mutable project-to-socket map, or require the Hermes MCP server registration to be fixed to one project.

The callback MUST invoke an absolute Hermes executable as a fixed argv equivalent to `hermes send --quiet --to <platform>:<chat-id>[:<thread-id>] <message>`. `--to` MUST be the complete delivery destination. It MUST set explicit `HOME`, `PATH`, and `HERMES_HOME` values because Conflux scrubs the callback environment. It MUST NOT use the Hermes API Server, native wake, a webhook, shell evaluation, polling, or a watcher. Secret values MUST NOT appear in callback argv or diagnostics.

The callback MUST validate `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`, and MUST treat event contents only as data. Callback failure MUST remain observability only and MUST NOT change Conflux workflow outcome.

The generated bot message MUST identify itself as an automation event rather than user-authored instruction, MUST contain a typed `event:` line and the exact `instance_id`, `execution_id`, and `change_id` binding, and MUST require verification of current owner and repository evidence before success is reported. Callback delivery remains observability and a responder-compatible trigger only; it MUST NOT itself run an agent loop or alter Conflux workflow routing or terminal classification.

The post-tool hook MUST be observational: refusal, malformed output, missing socket routing, or registration failure MUST NOT replace the tool result or fail the Hermes turn, and diagnostics MUST remain bounded and secret-free. Request-scoped messaging and socket routing context MUST be authoritative; process-global environment mirrors MAY be used only as compatibility fallbacks when no corresponding request-scoped value was supplied.

#### Scenario: Admitted enqueue registers the originating messaging thread

- **GIVEN** Hermes completes `cflx_enqueue` or an MCP name such as `mcp__cflx__cflx_enqueue`
- **AND** the result is a supported successful admitted envelope with complete binding identifiers
- **AND** Hermes request context identifies a messaging platform and chat, with an optional thread
- **AND** the tool arguments identify the Conflux owner Unix socket
- **WHEN** the post-tool hook runs
- **THEN** it registers one callback for that exact execution binding over that call-scoped owner Unix socket
- **AND** the callback argv contains the fixed messaging target and explicit profile environment paths but no secret value

#### Scenario: Two project calls retain independent owner routing

- **GIVEN** one Hermes process calls enqueue for project A with socket A
- **AND** it calls enqueue for project B with socket B
- **WHEN** each post-tool hook registers its completion sink
- **THEN** project A registration is sent only to socket A
- **AND** project B registration is sent only to socket B
- **AND** neither call mutates process-global routing for the other

#### Scenario: Call-scoped socket overrides compatibility fallback

- **GIVEN** `CFLX_UNIX_SOCKET` names socket A
- **AND** a qualifying enqueue call argument names socket B
- **WHEN** the post-tool hook registers the sink
- **THEN** it uses socket B
- **AND** it does not contact socket A

#### Scenario: Legacy host may use environment fallback

- **GIVEN** a qualifying admitted enqueue result
- **AND** the host supplies no call-scoped `unix_socket`
- **AND** `CFLX_UNIX_SOCKET` contains a non-empty socket path
- **WHEN** the post-tool hook runs
- **THEN** it MAY register through that fallback socket

#### Scenario: Missing or malformed socket fails closed

- **GIVEN** a qualifying admitted enqueue result
- **AND** neither a valid call-scoped socket nor a valid fallback exists
- **WHEN** the post-tool hook runs
- **THEN** it registers no completion sink
- **AND** it leaves the original tool result and Hermes turn unchanged

<!-- Expected canonical result after archive: Hermes callback registration follows each enqueue tool call's project socket, while the environment remains only a legacy fallback. -->
