# Design: cflx client MCP and execution-scoped completion sinks

## Architecture

```text
OpenCode / another MCP host
    │ stdio MCP
    ▼
cflx client mcp
    │ owner-only UDS, /api/v2
    ▼
long-lived cflx TUI / owner
    ├─ shared operator command service
    ├─ process-local execution registry
    ├─ truthful completion observer
    └─ bounded completion-sink dispatcher
```

The MCP adapter is an intent-shaped frontend over the existing `client` and `/api/v2` contracts. It must not contain a second orchestration state machine.

## Why MCP

OpenCode can identify an MCP tool by name in `tool.execute.after`. This avoids parsing arbitrary shell commands and lets the reference plugin bind a successful enqueue result to the originating OpenCode session. The MCP protocol itself does not expose that session identity; the OpenCode plugin remains the client-specific bridge.

## Why execution identity

`change_id` alone cannot distinguish:

- an original run from a later retry;
- a notification registered before owner restart from a new owner;
- two admitted episodes of the same proposal.

The owner therefore creates a random process-local `execution_id` whenever any admission source moves a change from non-admitted into queued or active work. The binding includes the owner `instance_id` and `change_id`. Concurrent or later `already_admitted` clients observe the current episode ID. Dequeue or typed terminal settlement ends the episode; dequeue followed by admission creates a new ID. Iterations within one admitted run keep the same ID. It is observability identity only, not durable workflow evidence.

## MCP surface

Use stdio JSON-RPC and expose closed tools.

### `cflx_status`

Input: optional Unix socket path and token environment-variable name.

Output: the existing coherent status envelope.

### `cflx_enqueue`

Input: `change_id` plus optional connection settings.

Output: the existing enqueue result plus `instance_id`, `execution_id`, and `change_id` when admitted.

### `cflx_wait`

Input: `change_id`, timeout, and optional connection settings.

Output: the existing truthful wait envelope. This remains useful for bounded synchronous observation and verification.

### `cflx_notify_set/get/clear`

These tools address an exact execution binding. `set` accepts a bounded argv array and optional blocked-attention delivery; every terminal type is mandatory. Identical set requests are idempotent and a different valid set replaces the prior sink. Unknown fields fail closed. Shell command strings are not accepted. Mutating calls are accepted only through the owner Unix socket.

The owner exposes authenticated `GET`, `PUT`, and `DELETE /api/v2/executions/{execution_id}/sink` resources outside the closed workflow command registry. Requests carry and validate `instance_id` and `change_id`, create no command record, require no expected revision or idempotency key, and do not advance `state_revision`. Execution status exposes the current `execution_id`; capabilities advertise execution-sink support. An older owner returns a typed unsupported-owner refusal.

## Owner-side execution registry

The registry lives only for the owner process lifetime. An entry contains:

- `instance_id`
- `execution_id`
- `change_id`
- admission revision and timestamp
- current observation classification
- configured sink, if any
- delivered event-type set

The registry never decides what Conflux does next. It observes authoritative dispatch plus repository completion evidence. Deleting it or restarting the process cannot alter workflow routing.

## Completion classification

Reuse `cflx client wait`'s execution contract and repository completion oracle. Refactor shared observation/classification code rather than copying it.

Terminal success requires current repository evidence for the owner's declared terminal mode. Terminal failure and attention classifications come from typed owner state. Change disappearance is ambiguous and never success.

`blocked` is non-terminal and edge-triggered. A later transition can produce `completed`, `failed`, or `stopped`. Identical unchanged attention states do not redeliver; leaving and re-entering blocked creates a new attention edge. `stopped` comes from settled stop/dequeue removal, including removal before active work. `failed` comes only from a typed terminal unsuccessful execution state, not disappearance or an unrestricted error string.

Repository verification runs outside the reducer/orchestration critical path with bounded subprocess deadlines. A typed terminal transition triggers verification; inconclusive repository evidence receives bounded retries and then a bounded diagnostic rather than a fabricated terminal event.

## Sink execution

The owner writes a versioned JSON event to an owner-private temporary directory and starts the configured argv directly with fixed environment variables. It applies bounded runtime and captured-output limits. The event file is immutable for the callback lifetime and removed after the callback is reaped; owner shutdown removes remaining files.

Delivery state is process-local. One terminal delivery attempt is allowed per execution, regardless of terminal event type. Blocked delivery is deduplicated per blocked attention edge. A failed delivery records bounded diagnostics but does not retry forever and does not affect orchestration. Registration after typed terminal settlement immediately attempts that terminal delivery once.

## OpenCode reference integration

The plugin does not mutate arbitrary MCP results. It filters the exact enqueue tool, validates the returned binding, and calls the notify tool. The callback helper:

1. validates loopback destination and IDs;
2. reads only the event file named by `CFLX_EVENT_PATH`;
3. deduplicates locally;
4. submits a short marked prompt to the original session;
5. tells the agent to inspect repository state and continue verification.

Because a crashed owner cannot deliver from a process-local registry, the plugin also runs a low-frequency bounded continuity observer. It resumes the session with typed `owner_restarted` when the registered owner vanishes or changes; it never reports that as completion.

OpenCode stores this as `role=user`; the marker is mandatory.

## Dependency choice

Prefer a maintained Rust MCP crate only if it materially shortens a correct stdio implementation and does not pull network/server machinery into the default runtime. Otherwise implement the minimal JSON-RPC stdio subset required by tools/list and tools/call. Keep MCP in a feature or isolated module so non-MCP behavior remains unchanged.

## Rejected Alternatives

### Agent constructs `notify_command`

Rejected because it depends on prompt compliance and repeats session-binding logic.

### OpenCode plugin parses terminal commands

Rejected because aliases, quoting, Herdr launch paths, and shell composition make it unreliable.

### Hold `cflx_wait` open forever

Rejected because long tool calls are fragile across client and server restarts.

### Observe completion only inside the long-lived MCP process

Rejected as the primary design because MCP hosts may restart or disconnect after registration, while the owner remains the only process with typed stop, failure, blocked, and admission-episode transitions. Owner-side registration survives MCP-client disconnect and gives all frontends the same execution identity. The OpenCode adapter uses client-side observation only as a narrow owner-continuity fallback because no process-local owner can report its own crash.

### Process lifecycle adapter only

Rejected because process idle/stopping events do not identify one proposal execution and the TUI remains alive.

### Persist subscriptions across restart

Rejected because they could become external durable workflow-control state and silently bind to changed repository evidence.
