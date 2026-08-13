---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-08-13-add-cflx-client-mcp
verifications:
  - id: core-mcp-sink-hardening
    requirement: MCP framing and completion sinks enforce their documented resource, identity, lifecycle, and protocol boundaries
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused unit and integration tests exercise oversized input/output, complete sink bindings and UDS-only argv disclosure, default read-only event payloads, MCP initialization ordering, and serialized multi-sink shutdown
    rerun: cargo test --test client_completion_sink && cargo test --test client_mcp_integration --features heavy-tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Harden client MCP and completion sinks

**Change Type**: implementation

## Problem / Context

The implementation merged by `add-cflx-client-mcp` passes its current tests but does not enforce several boundaries promised by the canonical CLI and remote-control API specs. MCP line input is collected without an effective memory limit. Callback output uses unbounded collection; stdout is not retained for diagnostics and stderr is truncated only after collection. Sink GET does not require the complete execution binding and can disclose argv over TCP. Callback event files remain writable by the callback UID. MCP accepts calls before initialization and accepts malformed JSON-RPC envelopes. Graceful shutdown uses a fixed wait that can expire before several sequential callbacks are reaped.

These are implementation defects. The canonical behavior already requires bounded, exact, protocol-valid execution-scoped notification delivery.

## Proposed Solution

- Read newline-delimited MCP frames with an allocation bound that applies before newline arrival.
- Capture callback stdout/stderr through bounded drains while retaining timeout and child reaping.
- Require `(instance_id, execution_id, change_id)` for sink GET, PUT, and DELETE, while treating the binding as coherence rather than access control and returning argv only over UDS.
- Create callback payloads as `0400` files in an owner-private `0700` directory. This is default write refusal, not an integrity guarantee against a hostile same-UID callback; the owner never re-reads or trusts the file.
- Enforce MCP initialization order and JSON-RPC 2.0 envelope validity without contaminating stdout.
- Keep callback delivery serialized. Apply one test-injectable global shutdown deadline, stop new event creation, cancel callbacks that cannot finish, explicitly reap every child, and only then clean artifacts.

## Acceptance Criteria

- An MCP peer sending more than the frame limit without newline cannot make the MCP process retain unbounded input.
- A callback producing unbounded stdout/stderr cannot make the owner retain output beyond configured limits; the owner continues draining and discarding after the limit so the child cannot block on a full pipe. Overflow alone does not kill the callback.
- Sink inspection without the complete binding is rejected, and argv is returned only over the owner Unix socket.
- Under an unprivileged owner UID, the callback can read its event payload but opening it for writing is refused by default. Owner decisions never depend on reading the artifact back.
- `tools/list` and `tools/call` before initialization, and requests without `jsonrpc: "2.0"`, receive machine-readable protocol errors.
- Graceful shutdown with more than two slow callbacks starts no new delivery or artifact after its injected deadline and cannot delete artifacts before callbacks are terminated/reaped; verification uses state transitions rather than wall-clock performance assertions.
- Existing success, late-registration, typed terminal, retry identity, and callback-failure semantics remain green.

## Explicit Completion Conditions

- Regression tests reproduce each defect and pass with the fixes.
- `cargo test --test client_completion_sink` passes.
- `cargo test --test client_mcp_integration --features heavy-tests` passes.
- Default `cargo test` passes without adding new default-suite work above the project heavy-test policy.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- Durable notification delivery across owner crashes.
- New MCP tools or raw `/api/v2` command exposure.
- Retrying failed terminal callbacks.
- OpenCode-specific callback behavior, handled by `secure-opencode-auto-resume-callback`.

## Split Rationale

Core Rust protocol and owner-sink correctness must ship atomically because the same execution contract spans MCP admission, remote resources, callback execution, and owner shutdown. OpenCode reference-adapter hardening is independently reviewable and is split into a separate change.
