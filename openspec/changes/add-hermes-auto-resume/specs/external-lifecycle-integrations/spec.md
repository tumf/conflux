## ADDED Requirements

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

The reference Hermes auto-resume integration MUST register an execution-scoped completion sink only from a supported versioned enqueue envelope with a successful admitted outcome and non-empty string `change_id`, `execution_id`, and `instance_id`. It MUST bind that execution to the originating messaging platform, chat ID, and optional thread ID supplied by Hermes request-scoped context.

The callback MUST invoke an absolute Hermes executable as a fixed argv equivalent to `hermes send --quiet --to <platform:chat[:thread]> <message>`. It MUST set explicit `HOME`, `PATH`, and `HERMES_HOME` values because Conflux scrubs the callback environment. It MUST NOT use the Hermes API Server, a webhook, shell evaluation, polling, or a watcher. Secret values MUST NOT appear in callback argv or diagnostics.

The callback MUST validate `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`, and MUST treat event contents only as data. Callback failure MUST remain observability only and MUST NOT change Conflux workflow outcome.

The generated message MUST identify itself as an automation event rather than user-authored instruction and MUST require verification of current owner and repository evidence before success is reported. Callback delivery remains observability and continuation only and MUST NOT alter Conflux workflow routing or terminal classification.

#### Scenario: Admitted enqueue registers the originating messaging thread

- **GIVEN** Hermes completes a `cflx_enqueue` or segment-exact namespaced equivalent
- **AND** the result is a supported successful admitted envelope with complete binding identifiers
- **AND** Hermes request context identifies a messaging platform and chat, with an optional thread
- **WHEN** the post-tool hook runs
- **THEN** it registers one callback for that exact execution binding over the owner Unix socket
- **AND** the callback argv contains the fixed messaging target and explicit profile environment paths but no secret value

#### Scenario: Unsupported enqueue result fails closed

- **GIVEN** an enqueue result has an unsupported schema, unsuccessful or non-admitted outcome, malformed operation, or missing binding identifier
- **WHEN** the post-tool hook runs
- **THEN** it registers no callback
- **AND** it starts no wait or polling process

#### Scenario: Callback notifies the existing Hermes thread

- **GIVEN** Conflux invokes the callback for a terminal execution event
- **AND** the selected Hermes profile can deliver to the bound messaging target
- **WHEN** callback delivery succeeds
- **THEN** the callback invokes `hermes send --quiet --to` with the bound target
- **AND** the body contains an explicit non-user-authored automation marker
- **AND** the body instructs the receiving agent to verify repository evidence rather than trust event fields

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

<!-- Expected canonical result after archive: external-lifecycle-integrations will include a reference Hermes messaging-thread callback contract parallel to the existing OpenCode contract, without changing Conflux workflow authority or packaging. -->
