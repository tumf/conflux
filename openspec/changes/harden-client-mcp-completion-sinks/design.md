# Design

## Bounded MCP framing

Do not call `read_line` on an unbounded `String`. Read through a byte-oriented limit-aware buffer that retains at most the configured frame size plus fixed overflow detection. Oversized frames and invalid UTF-8 terminate the stdio session without dispatching a tool or attempting stream resynchronization.

## Bounded callback process I/O

Pipe stdout and stderr and drain both concurrently for the whole life of the child. Each drain retains at most the configured limit and keeps reading and discarding past it, so a producer never blocks on a full pipe and owner memory never grows. Reaching the limit records truncation; it is not a delivery failure and does not terminate the callback. Only timeout and shutdown cancellation terminate the child, and both explicitly kill and `wait()` it. `wait_with_output` and `kill_on_drop` alone do not satisfy this contract.

## Event artifact immutability

The owner creates the payload as `0400` inside an owner-private `0700` directory. An ordinary same-UID callback is refused when opening it for writing, but the callback can defeat permissions with `chmod`; this is not an integrity boundary against hostile callback argv. The owner writes once, never re-reads or trusts the artifact, and removes it only after the child is reaped. Mutation tests run only under an unprivileged effective UID and verify default refusal plus owner non-reliance.

## Delivery ordering

Delivery remains serialized on the single dispatcher task. Shutdown is bounded by cancellation and reaping, never callback concurrency. Event artifacts are named per `(execution_id, event_type)`; any future concurrent delivery must first introduce per-delivery unique names.

## Shutdown accounting

One test-injectable global shutdown deadline governs callback draining. A cancellation token is wired through the dispatcher and active callback. Shutdown stops accepting registrations and deliveries, forbids event-directory creation or recreation, cancels queued work, and either reaps the active child normally or kills and explicitly waits for it before cleanup. Tests assert event and lifecycle ordering with channels/state, not short wall-clock thresholds.

## Protocol state

Before `initialize`, only `initialize` and `ping` are accepted. Tool listing and calls become enabled when the server responds successfully to `initialize`; `notifications/initialized` is accepted idempotently. Every request envelope must identify JSON-RPC 2.0. Invalid request objects receive an invalid-request response using their valid request ID or `null`; invalid notifications receive no response. JSON-RPC batch arrays are rejected as invalid requests because this server does not advertise batch support. Existing direct unit tests must perform initialization rather than weakening the gate.
