## MODIFIED Requirements

### Requirement: Local client compatibility discovery

The versioned single-instance API MUST expose enough generated capability, instance, authoritative-state, execution-status, command-record, and event information for the source-matched `cflx client` client to inspect and operate a command-capable owner without parsing logs or display strings. The client MUST check API compatibility and process incarnation before mutation and during command settlement. A typed command-capability field MUST identify whether the executor is bound. An unbound command executor MUST return a distinct `command_executor_unbound` wire error rather than an ordinary lifecycle conflict or a command queued for later execution.

The local client MUST validate an environment-provided bearer token as an HTTP header value before opening a connection or constructing a request. It MUST reject CR, LF, all other HTTP-forbidden control characters, and DEL with a typed error that identifies only the environment-variable source, not the token value. Valid header values MUST be transmitted unchanged.

This compatibility surface MUST include a minimal typed owner execution contract at an `instance_id` and `state_revision`: base branch identity and terminal mode (`merged`, `base_published`, or `branch_pushed`), plus selected remote and pushed branch when applicable. The generated OpenAPI document MUST publish these fields. They are observational facts only and MUST NOT become durable workflow authority; repository and workspace evidence remain authoritative for routing and truthful completion.

#### Scenario: Malformed bearer token is rejected before transport

**Given**: the named authentication environment variable contains CR, LF, another forbidden HTTP control character, or DEL
**When**: the client prepares any owner request
**Then**: it returns a typed redacted client error before connecting or writing request bytes
**And**: neither stdout nor stderr contains the token value

#### Scenario: Valid bearer token remains supported

**Given**: the named authentication environment variable contains a valid HTTP header value
**When**: the client connects to an authenticated owner
**Then**: the Authorization header carries that value unchanged
**And**: normal authentication behavior is preserved
