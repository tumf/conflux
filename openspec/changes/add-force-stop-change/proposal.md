---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/tui-architecture/spec.md
  - src/orchestration/operator_command.rs
  - src/web/remote_control_api/dto.rs
  - src/client/control.rs
  - src/client/mcp.rs
verifications:
  - id: targeted-force-stop-tests
    requirement: "A named in-flight change can be force-stopped without stopping or dequeuing unrelated changes or changing the process-wide run mode"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust unit, API, client, MCP, and projection tests covering active, queued, terminal, unknown, stale-revision, and idempotent replay cases"
    rerun: "cargo test --features web-monitoring force_stop_change && cargo test --test client_cli_tests force_stop_change && cargo test --test client_mcp_integration force_stop_change"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add targeted force-stop control for one change

**Change Type**: implementation

## Premise / Context

- Process-wide `force_stop` immediately stops all work selected by the owner's authoritative lifecycle state.
- `stop_and_dequeue` addresses one change, but follows confirmed normal termination and dequeue settlement rather than providing an immediate targeted kill.
- Operators need to terminate one runaway or invalid execution while unrelated changes continue.
- Targeted force-stop is an operator intervention. It must not be inferred from errors or invoked automatically.

## Problem / Context

The v2 command API and `cflx_control` expose no immediate stop scoped to a single change. The only immediate control is process-wide `force_stop`, so stopping one runaway change also disrupts unrelated work. A caller cannot safely express “kill this change only.”

## Proposed Solution

Add a typed `force_stop_change` operator command carrying exactly one `change_id`.

The shared operator transaction MUST:

1. validate the target against the fresh authoritative state;
2. immediately cancel and terminate only managed activity owned by that change, including its current phase subprocesses;
3. wait for confirmed target quiescence before settlement;
4. remove that change's active/queued admission intent so it is not immediately redispatched;
5. classify the change as stopped while preserving its worktree and completed effects;
6. leave unrelated changes, marks, queue intents, subprocesses, and the process-wide run mode unchanged.

The command MUST be idempotent through the ordinary command registry and optimistic revision contract. Unknown, terminal, already-quiescent, unsupported-phase, and stale-revision requests return typed no-op or failure outcomes without touching another change.

Expose the same shared operation through:

- `POST /api/v2/commands` as `{ "type": "force_stop_change", "change_id": "..." }`;
- per-change `actions.force_stop_change` eligibility;
- `cflx client force-stop-change <change-id> --json`;
- `cflx_control` action `force_stop_change` requiring exactly one distinct `change_id`.

Do not implement this by temporarily replacing the process-wide mark set or by invoking process-wide `force_stop`. Do not kill by PID lookup outside the managed ownership graph.

## Acceptance Criteria

1. An applying, accepting, rejecting, archiving, resolving, or otherwise managed in-flight target can be force-stopped individually when its published action eligibility permits it.
2. Target settlement occurs only after the target's managed processes are confirmed terminated and reaped.
3. The target is dequeued/stopped, retains completed worktree effects, and is not automatically redispatched from stale queue intent.
4. Unrelated active and queued changes continue unchanged; their processes, marks, queue intents, execution IDs, and subscriptions are preserved.
5. Process-wide `app_mode`, scheduler started/stopped state, and process-wide stop state do not change.
6. Unknown, terminal, unsupported, and already-quiescent targets cannot cause cross-target cancellation.
7. Stale revisions are rejected before side effects. Exact idempotent replay returns the original settled result without repeating termination.
8. The API result identifies the target, cancelled phase, last completed phase, confirmed termination, and `effects_rolled_back: false`.
9. CLI and MCP expose the operation without allowing zero, multiple, duplicate, or blank targets.
10. Existing process-wide `stop`, `force_stop`, and individual `stop_and_dequeue` semantics remain unchanged.

## Explicit Completion Conditions

- Shared operator code has one target-scoped force-stop path used by API, CLI, MCP, and any TUI adapter; frontends do not reproduce cancellation policy.
- No targeted path invokes the process-wide ForceStop intent or rewrites unrelated execution marks.
- Reducer/projection tests prove target-only cancellation and continued progress of at least one concurrent unrelated change.
- Child-process tests prove the target process is terminated and reaped before command settlement.
- Generated OpenAPI, capabilities, CLI help, MCP schema, README, AGENTS guidance, and bundled client skill document `force_stop_change` and its one-target contract.
- `cflx openspec validate add-force-stop-change --archive-gate` passes.

## Scope Rationale

The command, shared cancellation semantics, projections, API contract, and client adapters form one safety boundary. Shipping only the raw API would leave agent clients unable to invoke the operation safely; shipping frontend-only logic would create a second cancellation path.
