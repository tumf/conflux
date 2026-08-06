## Context

The repository already has the right domain services but not one complete application transaction.

- `OperatorCommandService` owns mark, queue, dequeue, retry planning, and hooks.
- `RunControlService` owns start target resolution, scheduler dispatch, process stop controls, force-stop classification, and resolve reservations.
- `EventDispatcher` applies `ExecutionEvent` to `OrchestratorState` and fans one post-transition state to `EventSink` implementations.
- `WebState` owns the v2 projection, while TUI `AppState` caches mode, row status, and checkbox presentation.
- `SharedServiceExecutor` calls the services and then requests a Web snapshot refresh; it does not deliver the accepted outcome to live TUI state or bind the command record to the revision that outcome produced.

The active `synchronize-execution-marks` change makes `ExecutionMarkStore` authoritative during system events and binds mark reconciliation before frontend fan-out. This design assumes that repository output and does not duplicate its event-revocation policy.

## Goals

- Make TUI and v2 enter one command application transaction for equivalent operator intent.
- Use one process lifecycle mode for admission and project it to every frontend.
- Order accepted command effects before scheduler events they enable.
- Make changed, no-op, and failure effects atomic and truthful.
- Store the exact command-produced `result_revision`.
- Preserve exact replay without duplicate side effects.
- Keep all new coordination process-local.

## Non-Goals

- Adding commands or changing bulk/parallel eligibility contracts.
- Replacing reducer-owned change lifecycle with a process-mode state machine.
- Reimplementing event-driven execution-mark revocation.
- Defining global `Stopped` row reconciliation.
- Persisting command coordination or operator intent.

## Decision: One Process-Local Application Coordinator

Add one coordinator over the existing services rather than another service implementation per frontend. Both adapters submit a typed intent to the coordinator and receive the same typed application result.

The coordinator owns:

- a process-local command serialization gate;
- access to Core process lifecycle mode;
- final revision and target revalidation for new remote commands;
- service execution and rollback/staging boundaries;
- authoritative outcome dispatch;
- scheduler activation ordering;
- the exact revision returned by projection.

TUI key handling may still decide presentation-only concerns such as a confirmation modal or local log wording. It cannot apply a lifecycle transition before the coordinator accepts it.

## Decision: Process Mode Is Core State, Not Frontend Admission State

The command-capable process needs one ephemeral lifecycle value with `Select`, `Running`, `Stopping`, `Stopped`, and `Error`. It is not reducer change lifecycle and is not persisted.

TUI `AppExecutionMode` and Web `app_mode` become projections of this value. They may keep local rendering fields, but `RunControlService` validation receives the coordinator's current mode rather than a client-provided or frontend-cached mode.

Mode transitions are outcome-driven:

| Accepted effect | Resulting mode |
|---|---|
| start/resume/retry dispatch | Running |
| active resolve reservation | Running |
| queued resolve reservation | unchanged |
| graceful stop | Stopping |
| cancel stop | Running |
| force stop awaiting safe boundary | Stopping |
| settled force stop | Stopped |
| no-op/failure | unchanged |

Existing terminal-event guards remain authoritative for late `AllCompleted` and similar delivery.

## Decision: Serialize New Remote Commands Through Settlement

Current projection admission is atomic only for record reservation. It releases the lock before service execution, allowing two new commands to reserve against the same revision.

Move final new-command admission under the application coordinator gate:

```text
lookup exact idempotency identity
  -> if replay, return original record
acquire application gate
  -> re-check exact replay
  -> validate expected_revision against current projection
  -> reserve command record
  -> execute application transaction
  -> dispatch accepted outcome or retain unchanged state on no-op/failure
  -> complete record with the returned revision
release gate
```

A long `stop_and_dequeue` can still execute asynchronously relative to the HTTP request and return `202`; it remains serialized as a command until it settles. This favors correctness over concurrent operator mutations. Runtime orchestration events continue independently through the authoritative dispatcher, but the command's accepted outcome is bound to its own dispatch revision rather than a later read.

Exact replay never waits for or reacquires side-effect execution. It returns the current stored representation of the original record.

## Decision: Prepare, Commit, Dispatch, Activate

A new scheduler task must not emit progress before its command's accepted mode and decision state. Use a prepared activation boundary or an equivalent dispatch transaction held across service execution.

The required order is:

```text
validate
  -> prepare scheduler/cancellation capability without observable progress
  -> stage reducer, mark, queue/retry, resolve, stop, and mode changes
  -> commit staged changes
  -> dispatch one authoritative command outcome and obtain its revision
  -> activate/wake scheduler or complete cancellation control
  -> settle command with the dispatch revision
```

Activation after dispatch must be infallible. If an operation can fail, that failure belongs to preparation before commit. A port that can still fail after commit must provide rollback covering every staged axis and must prove no event escaped. The simpler preferred implementation is a prepared scheduler permit whose final activation cannot fail.

For a live scheduler wake, queue/retry state is committed before wake and the outcome dispatch occurs before notification. For a new run, the spawned future waits on activation and cannot publish through `EventDispatcher` until released.

## Decision: Minimal Typed Outcome Vocabulary

Reuse exact existing events:

- `Stopping` for accepted graceful stop;
- `Stopped` for force stop after its safe boundary settles;
- `ChangeDequeued` for successful target dequeue.

Add one `OperatorCommandApplied` execution-event variant carrying a closed internal effect enum only for state not represented by an existing exact event:

- run dispatched with target IDs and explicit-retry fact;
- stop cancelled;
- force stop awaiting a safe boundary with typed classification needed by projections;
- mark delta and queue-intent delta;
- active or queued resolve reservation.

The event has state ownership because its process mode, marks, queue intent, or resolve reservation changes the operator snapshot. `OrchestratorState::apply_execution_event` only applies reducer work that has not already been committed; it must not duplicate `ReducerCommand` application. The post-transition dispatch state is the source for row statuses.

Do not emit synthetic `ProcessingStarted`: it would incorrectly set a current change and activity before the scheduler actually starts that target.

## Decision: Return Revision From Projection Application

Projection application must return a typed result such as:

- changed with resulting revision;
- unchanged with current revision;
- duplicate dispatch with the original resulting revision when required internally.

The coordinator passes that revision directly to command settlement. `Projection::complete_command` no longer reads `inner.state_revision` as an implicit substitute.

This gives precise semantics:

| Outcome | `result_revision` |
|---|---|
| changed | outcome dispatch revision |
| no-op | unchanged admitted revision |
| failed with no effect | unchanged admitted revision |
| replay | originally stored revision |

Later scheduler events advance projection normally but never mutate the record.

## Decision: Command Mark Projection Uses Deltas

After `synchronize-execution-marks`, TUI system-event handling reads marks from the shared store. Operator commands also need to avoid the reverse direction.

- Single mark outcomes name one ID and target value.
- Bulk outcomes name only changed IDs and one target value.
- Queue outcomes name one target and queue membership; the TUI checkbox may render queue membership in Running without writing it back as a mark.
- Run outcomes name exact accepted targets.
- Dequeue clears the target through existing shared event reconciliation.

Remove command-side calls that rebuild all marks from every `ChangeState::selected`. If a bulk local interaction still classifies rows before service execution, its final mutation must go through the shared service and outcome delta; local optimistic row changes cannot commit shared authority.

## Decision: Resolve Reservation Is Projected at Acceptance

`ResolveReservations` remains the single process-local ledger. Bind the same ledger to Core command coordination, TUI, and Web snapshot building.

- Active reserve sets `is_resolving=true` at the command outcome revision and moves mode to Running.
- Queued reserve records FIFO but does not dispatch scheduling and does not independently change mode.
- Duplicate reserve is no-op.
- Preparation failure cancels the reservation and restores reducer intent before failure settlement.
- Resolve lifecycle events finish, cancel, or promote ledger ownership through Core processing rather than a TUI-only handler.

## Failure Atomicity

The coordinator must snapshot or stage only the affected process-local axes. A failure must preserve:

- reducer runtime and blocker evidence;
- target and unrelated marks;
- dynamic queue and pending removals;
- explicit-retry publications;
- resolve active/waiting order;
- graceful-stop flag and cancellation ownership;
- process mode;
- hook counts;
- scheduler start/notify/cancel counts;
- event sequence and state revision.

Tests use recording ports and compare before/after state. A narrative error result without those comparisons is insufficient.

## Event Ordering and Duplicate Handling

All command outcomes and scheduler events use the same process-wide dispatch owner supplied by the event-ownership architecture. The owner serializes reducer application and sink fan-out for one event before processing the next.

The scheduler activation gate establishes the causal order. Frontend mode handlers still retain monotonic guards so duplicate delivery is harmless, but guards are defense in depth rather than the primary ordering mechanism.

## Dependency and Parallel Work

This change depends on `synchronize-execution-marks` because it consumes the dispatcher/store binding and one-way TUI mark projection introduced there. Its verification prerequisite is `execution-mark-event-regressions`.

No hard dependency is declared on:

- `fix-force-stop-reducer-reconciliation`: this change orders and emits `Stopped`; that change independently defines the reducer row transition.
- `restore-ready-on-persistent-idle`: idle mode projection is independent from command acceptance.
- bulk or parallel-control work: this change preserves their existing command shapes and classifications.

## Verification Strategy

1. Table-drive every command across process mode, target status, scheduler live/idle, and changed/no-op/failure outcomes using one shared coordinator harness.
2. Use the same shared `ExecutionMarkStore`, `ResolveReservations`, reducer, Web projection, TUI `AppState`, and recording scheduler for same-process convergence tests.
3. Gate scheduler test emission until accepted outcome dispatch and deliberately race activation with immediate terminal events.
4. Compare every mutable axis before and after injected preparation failure.
5. Submit concurrent v2 requests with one expected revision and prove only one new state-changing identity enters service execution.
6. Advance projection after settlement and prove replay retains the original record and revision without effects.
7. Keep default unit/integration cases under one second; no real agent or external process is required.

## Risks and Mitigations

- **Long stop-and-dequeue blocks other operator commands:** serialize by design; exposing interleaving would violate revision and target assumptions. HTTP can return `202` while the record remains running.
- **Process mode duplicates reducer lifecycle:** keep mode process-level only; change statuses remain reducer-derived and restart routing remains workspace-derived.
- **Prepared scheduler adds lifecycle complexity:** confine it to `RunSchedulerPort` and test that activation is infallible and event-silent before release.
- **Outcome enum grows into a second event model:** reuse exact existing events and keep the added enum limited to accepted command decision facts.
- **Dependency creates unnecessary blockage:** the dependency consumes concrete dispatcher/mark projection code, not roadmap ordering; unrelated idle and stop-reconciliation work remains parallelizable.
