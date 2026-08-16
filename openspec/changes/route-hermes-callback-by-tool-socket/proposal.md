---
change_type: implementation
priority: high
dependencies: []
references:
  - examples/integrations/hermes-auto-resume/__init__.py
  - tests/hermes_auto_resume_example.rs
  - openspec/specs/external-lifecycle-integrations/spec.md
verifications:
  - id: hermes-project-socket-routing
    requirement: Each Hermes enqueue callback is registered with the owner socket selected by that tool call
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Heavy integration tests exercise two project sockets and fallback behavior without changing global environment between calls
    rerun: cargo test --features heavy-tests --test hermes_auto_resume_example
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Route Hermes callback registration by tool socket

Change Type: implementation

## Problem / Context

The Hermes MCP server can accept `unix_socket` on each Conflux tool call, but the reference auto-resume plugin currently discards post-tool `args` and registers the completion sink using only process-global `CFLX_UNIX_SOCKET`.

A Hermes agent configured with that environment variable or a fixed MCP `--unix-socket` is therefore bound to one Conflux project. Concurrent or sequential calls for other project owners can enqueue successfully but register their callback against the wrong owner.

## Proposed Solution

Read the exact `unix_socket` value from the qualifying `cflx_enqueue` tool arguments and use it for that execution's `cflx client notify set` call. Keep `CFLX_UNIX_SOCKET` only as a backward-compatible fallback when the host does not expose a call-scoped socket argument.

The plugin will fail closed when no socket can be resolved. The example documentation will register the MCP server without a project-fixed socket and show callers passing `unix_socket` per tool call.

## Acceptance Criteria

- One Hermes process can enqueue work for two independent Conflux owner sockets and register each callback with the matching owner.
- A tool-call `unix_socket` overrides `CFLX_UNIX_SOCKET` for that callback registration.
- Existing hosts that omit tool arguments may continue using `CFLX_UNIX_SOCKET` as a fallback.
- Missing, malformed, or non-string socket arguments do not cause cross-project registration or alter the original tool result.
- Documentation no longer recommends fixing one project socket in the global Hermes MCP server registration.

## Explicit Completion Conditions

- `on_post_tool_call` no longer discards `args`; it resolves call-scoped connection options before registration.
- Heavy integration tests prove routing across two distinct owner sockets and fallback behavior.
- README setup and troubleshooting describe per-call project routing.
- `cargo test --features heavy-tests --test hermes_auto_resume_example`, formatting, Clippy, and archive-gate validation pass.

## Out of Scope

- Reintroducing a multi-project Conflux server.
- Discovering repositories or owner sockets from `change_id`.
- Persisting project-to-socket mappings outside the tool call.
- Changing Conflux workflow routing or terminal classification.
