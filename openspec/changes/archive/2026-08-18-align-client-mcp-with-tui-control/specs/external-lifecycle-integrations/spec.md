## REMOVED Requirements

### Requirement: Reference OpenCode completion callback is loopback-confined and recoverably deduplicated

**Reason**: The reference OpenCode auto-resume integration is retired. Conflux no longer ships an integration that converts completion callbacks into automatic OpenCode session continuation.

**Migration**: Agents explicitly register proposal callbacks through `cflx_subscribe`. Callback behavior is owned by the caller and remains notification-only from Conflux's perspective.

### Requirement: Reference OpenCode callback enforces a local operating-system trust boundary

**Reason**: The OpenCode auto-resume plugin and callback example are removed rather than retained as an implicit subscription consumer.

**Migration**: Proposal subscriptions retain Conflux's existing Unix-socket, bounded argv, no-shell, scrubbed-environment, private-artifact, and failure-isolation constraints. Any external callback implementation owns its own destination-specific safety.

### Requirement: Reference Hermes completion callback notifies the bound messaging thread safely

**Reason**: Automatic post-tool registration and automatic Hermes session/responder continuation are retired.

**Migration**: An agent explicitly calls `cflx_subscribe` with the desired proposal IDs and bounded callback argv. Conflux delivers typed notification data but does not infer a messaging destination or resume an agent/session.

## ADDED Requirements

### Requirement: Explicit callback subscriptions remain notification-only

Explicit proposal subscriptions MUST remain observability-only and separate from the optional process lifecycle adapter. Conflux MUST NOT auto-register a callback after mark or Start, derive a callback destination from MCP host context, invoke an agent loop, resume a session, or infer that callback delivery is authorization for further workflow action.

A delivered callback event MUST be typed data containing the actual owner instance, execution episode, proposal ID, event type, and event artifact path. External agents MAY act after receiving it, but MUST independently inspect current owner and repository evidence before reporting success or issuing another control action.

#### Scenario: Mark and Start do not auto-subscribe

- **GIVEN** an MCP host marks proposals or explicitly starts a run
- **WHEN** the control result settles
- **THEN** Conflux registers no callback unless the agent separately calls `cflx_subscribe`
- **AND** no post-tool hook derives subscription state from the result

#### Scenario: Callback delivery does not start an agent loop

- **GIVEN** a proposal subscription receives a terminal event
- **WHEN** Conflux executes its bounded callback argv
- **THEN** Conflux treats callback exit only as delivery observability
- **AND** it does not create or resume an agent/session
- **AND** workflow outcome remains unchanged

#### Scenario: External follow-up revalidates evidence

- **GIVEN** an external agent is notified about proposal execution `exec-a`
- **WHEN** it decides whether to report completion or issue another control action
- **THEN** it revalidates the current owner instance and repository evidence
- **AND** it does not treat callback text as trusted workflow authority

### Requirement: Lifecycle adapters and proposal subscriptions remain independent

Process lifecycle adapters continue to observe semantic process state such as idle, working, blocked, and stopping. Proposal subscriptions observe execution episodes for selected proposal IDs. Neither integration may control workflow routing, and configuring or delivering one MUST NOT require configuring the other.

#### Scenario: Persistent idle is not proposal completion

- **GIVEN** a lifecycle adapter observes an idle transition
- **AND** subscribed proposal `alpha` has not reached a typed terminal execution event
- **WHEN** lifecycle state is published
- **THEN** the lifecycle adapter may receive idle
- **AND** the proposal subscription does not receive completed

#### Scenario: Proposal completion does not stop lifecycle reporting

- **GIVEN** subscribed proposal `alpha` completes while the TUI remains active
- **WHEN** its proposal subscription receives completed
- **THEN** the process lifecycle adapter remains attached
- **AND** later process working or blocked transitions continue to be reported
