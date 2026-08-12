---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md:2399
  - openspec/specs/remote-control-api/spec.md:17
  - openspec/specs/operator-command-execution/spec.md:7
  - openspec/specs/external-lifecycle-integrations/spec.md:36
  - src/client/mod.rs
  - src/client/enqueue.rs
  - src/client/wait.rs
  - src/web/remote_control_api/
verifications:
  - id: cflx-client-mcp-acceptance
    requirement: The client MCP, execution identity, completion sinks, and OpenCode reference integration satisfy the proposal without changing existing-owner workflow authority
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Repository-local unit and integration tests plus OpenSpec, cargo test, and clippy output
    rerun: cflx openspec validate add-cflx-client-mcp --archive-gate && cargo test && cargo clippy --all-targets --all-features -- -D warnings
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add a cflx client MCP with execution-scoped completion notifications

**Change Type**: implementation

## Summary

Add a stdio MCP adapter for the existing `cflx client` intent boundary and an execution-scoped completion-sink contract owned by the running Conflux process. This lets an agent enqueue work into a long-lived TUI without becoming a second owner, then receive one terminal callback for the exact admitted execution.

The repository will also include an optional OpenCode reference integration. It binds the MCP tool result to the originating OpenCode session and registers a callback without requiring the agent to construct notification commands.

## Problem

`cflx client enqueue` can safely admit one change into an existing TUI, and `cflx client wait` can prove completion. Neither provides autonomous continuation:

- a caller must repeatedly invoke `wait` or hold a long-running process;
- the TUI intentionally remains alive after a change finishes, so process exit is not a completion signal;
- an MCP server does not receive the OpenCode session ID through the standard tool-call contract;
- asking every agent to construct a callback command is easy to forget and couples prompts to one client;
- process-wide lifecycle adapters describe TUI state, not one exact admitted change execution.

## Proposed Solution

### 1. Add `cflx client mcp`

Run a stdio MCP server that reuses the existing client modules and the owner's local `/api/v2` socket. Initially expose intent-shaped tools:

- `cflx_status`
- `cflx_enqueue`
- `cflx_wait`
- `cflx_notify_set`
- `cflx_notify_get`
- `cflx_notify_clear`

The MCP process remains a client. It does not acquire the repository lock, become an owner, start orchestration, or duplicate raw revision/idempotency logic.

### 2. Identify an admitted execution explicitly

A successful `cflx_enqueue` returns a process-local `execution_id` in addition to `instance_id` and `change_id`. The ID identifies one admitted execution episode, including retries of the same proposal as distinct episodes. The owner creates episodes for every admission source, including TUI, scheduler, direct `/api/v2`, CLI client, and MCP; `already_admitted` returns the current episode ID.

The owner creates the ID when a change enters queued or active work from a non-admitted state. Notification operations require the tuple `(instance_id, execution_id, change_id)` and reject stale owner incarnations or mismatched bindings.

Execution IDs and notification registrations are process-local observability state. They are discarded on restart and are never consulted for workflow routing, acceptance, archive, merge, retry, or scheduler eligibility.

### 3. Add owner-side completion sinks

`cflx_notify_set` attaches one command sink to an existing execution. The owner emits a versioned event file and invokes the sink once when repository-verifiable observation reaches one of:

- `completed`
- `failed`
- `blocked`
- `stopped`
- `owner_stopping` on graceful owner shutdown only

`completed` uses the same completion oracle and owner execution contract as `cflx client wait`; disappearance or a presentation-only lifecycle transition is not success. `blocked` is an opt-in attention event and may be followed by a later terminal event. All terminal event types are always enabled. Terminal delivery is attempted at most once per execution; `blocked` is delivered once per leave-and-reenter edge. Registering after an execution is already terminal causes an immediate single terminal delivery attempt.

Sink set/get/clear use dedicated authenticated `/api/v2` execution-sink resources, not the closed workflow command registry. Sink mutation is accepted only over the owner-only Unix socket. Capability discovery reports support explicitly, and unsupported owners fail with a typed refusal.

The callback receives only fixed environment variables:

```text
CFLX_EVENT_PATH
CFLX_EVENT_TYPE
CFLX_EXECUTION_ID
CFLX_CHANGE_ID
CFLX_INSTANCE_ID
```

The event file contains bounded typed data and paths, never prompts, terminal contents, environment dumps, credentials, or unrestricted error text. Delivery failure is observability-only and cannot change orchestration outcome.

### 4. Include an OpenCode reference integration

Add an opt-in plugin and callback helper under `examples/integrations/opencode-auto-resume/`.

The plugin observes OpenCode's common tool hook, filters only the cflx MCP enqueue tool, extracts the returned execution binding, and invokes `cflx_notify_set` with the originating loopback OpenCode server and session ID. The callback checks the typed event, deduplicates by execution and event type, then resumes the original session with an explicit marker:

```text
[AUTOMATION EVENT — not user-authored]
```

The generated message is documented as an ordinary OpenCode `role=user` message, not a trusted internal event. Event files and logs are data, never instructions.

## Security and Failure Model

- MCP uses stdio and the existing owner-only Unix socket by default.
- Authentication tokens remain environment-variable references; token values never enter argv, tool results, logs, or event files.
- Notification commands are argv, not shell source. No `sh -c` interpretation is added, and sink set/clear is rejected over TCP even with a bearer token.
- Notification registration validates exact instance/execution/change binding.
- OpenCode callbacks allow only loopback destinations by default.
- Callback delivery has a bounded timeout and output size. It never blocks or rolls back orchestration.
- Owner restart invalidates process-local execution IDs and registrations. Later get/set/wait operations return the existing typed `owner_restarted` outcome; registrations are not silently rebound. A graceful old owner may attempt `owner_stopping`, but a crash cannot deliver a final callback.
- The OpenCode reference plugin therefore keeps a low-frequency bounded owner-continuity observer after registration. It resumes the original session with typed `owner_restarted` if the owner disappears or changes, without treating that outcome as workflow success.
- The MCP adapter does not expose raw `/api/v2` commands or free-form workflow mutations.

## Non-Goals

- Replacing the TUI or `/api/v2`.
- Making MCP notification messages trigger model inference without a client integration.
- Keeping a single MCP tool call open for the full change duration.
- Persisting notification registrations across owner restart.
- Treating callback delivery as workflow success.
- Adding notification state as durable workflow authority.
- Supporting arbitrary non-loopback OpenCode endpoints in the reference integration.

## Impact

- Adds a new optional client surface and a small MCP dependency or minimal protocol implementation.
- Extends the client result envelope and owner projection with process-local execution identity.
- Adds process-local completion subscription and bounded callback execution.
- Adds tests for MCP protocol, execution binding, truthful terminal classification, idempotency, restart invalidation, callback isolation, and OpenCode wiring.
- Preserves current CLI, TUI, owner lock, remote-control, and workspace-derived workflow semantics when unused.

## Rollback

Remove the MCP subcommand, completion-sink registry, execution identity fields, and reference integration. Existing `cflx client status/enqueue/wait`, TUI, and `/api/v2` behavior remain the compatibility baseline.
