---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-architecture/spec.md
  - src/client/enqueue.rs
  - src/client/mcp.rs
  - src/orchestration/mark_settlement.rs
  - src/orchestration/operator_command.rs
  - tests/client_cli_tests.rs
verifications:
  - id: client-enqueue-analysis-tests
    requirement: "CLI and MCP enqueue preserve existing execution marks and use the same mark-settlement/analyze admission path as TUI mark input without directly creating queue intent"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust unit and integration test output covering live-owner enqueue command records, mark preservation, delayed analyze admission, stale revisions, refusal, and MCP/CLI parity"
    rerun: "cargo test --test client_cli_tests enqueue"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Route client enqueue through execution-mark analysis

**Change Type**: implementation

## Premise / Context

- Execution marks are human-selected run targets. Queue intent is a distinct scheduler-owned admission state.
- TUI Space and bulk `x` write `ExecutionMarkStore`, notify the shared mark-settlement coordinator, and let the existing analyze/admission path decide whether and when marked work can enter the current run.
- `cflx client enqueue` is shared by the CLI and MCP `cflx_enqueue` adapter, but its live-owner `Route::Queue` currently submits `SetQueueIntent(true)` directly.
- In the observed `prior-art-graph` run, this shortcut admitted and dispatched `prioritize-invention-source-frontier` as `queued` and then `active` while `execution_marked` remained false.
- A later scheduler `analyze_command` does not repair the violation: the client already bypassed the mark-settlement boundary that owns human target selection, stability, capacity, and additive admission.

## Problem / Context

The existing-owner client was intended to expose high-level enqueue intent while hiding raw commands. For a live scheduler it instead chooses a lower-level queue command that the equivalent TUI action does not use. This creates two admission semantics:

- TUI: add one execution mark, preserve the existing mark set, settle/analyze, then admit when eligible and capacity permits;
- CLI/MCP: write queue intent immediately, bypassing mark settlement and its analyze decision.

That divergence permits an ordinary change to start without ever appearing as selected in the TUI and allows an external client to skip the analyze gate entirely.

## Proposed Solution

Make CLI and MCP enqueue use the existing TUI-equivalent mark input and shared settlement/analyze path:

- remove the live-owner `Route::Queue` shortcut from `src/client/enqueue.rs`;
- for an eligible ordinary target, submit target-scoped `SetExecutionMark(true)` only when that target is currently unmarked, preserving all unrelated marks;
- rely on the existing shared mark-settlement coordinator and analyze/admission services to decide whether and when the marked target becomes queued or active;
- wait boundedly for authoritative admission evidence before returning `admitted`; a settled mark without admission before refusal, owner replacement, or deadline remains a typed non-success and MUST NOT be reported as admitted;
- keep retry routing for retryable error evidence and already-admitted detection unchanged;
- keep execution marks and queue intent distinct. Enqueue MUST NOT synthesize queue state from a mark or mark state from queue presentation;
- keep MCP as a thin adapter over the same `client::enqueue` implementation used by the CLI, with no MCP-specific route or command construction.

No new queue, analyze implementation, timer, durable state, command type, dependency, or MCP behavior fork is introduced.

## Acceptance Criteria

1. Live-owner CLI and MCP enqueue no longer submit `set_queue_intent` for an ordinary fresh target.
2. Enqueue adds only the requested execution mark and preserves every unrelated execution mark.
3. The existing shared mark-settlement/analyze path is notified exactly as for an accepted TUI mark write; the client does not invoke queue admission directly.
4. Enqueue returns `admitted` only after authoritative state shows the target queued or active in the current execution episode.
5. A target that is marked but not admitted remains visibly marked and produces a bounded typed non-success rather than false admission.
6. Retryable-error routing, already-admitted idempotence, stale-revision recomputation, owner-incarnation checks, unsafe-target refusal, and execution ID resolution retain their existing contracts.
7. CLI and MCP use the same implementation and produce the same envelope semantics for equivalent owner state.
8. Execution marks remain process-local human target intent; queue intent remains separate scheduler admission state.

## Explicit Completion Conditions

- `src/client/enqueue.rs` has no live-owner ordinary path that constructs `SetQueueIntent { queued: true }`.
- The ordinary enqueue path uses the existing mark command/service and observes the resulting shared settlement/analyze admission rather than adding a second analyze or queue implementation.
- Focused unit tests reject `Route::Queue` behavior and prove the requested mark is additive over unrelated marks.
- `tests/client_cli_tests.rs` proves a live-owner enqueue records a mark command, does not record a queue-intent command from the client, waits for analyze-driven admission, and does not claim admission from mark settlement alone.
- MCP adapter tests or shared-path assertions prove `cflx_enqueue` delegates to the same corrected enqueue implementation without constructing commands itself.
- Existing stale-revision, retry, refusal, already-admitted, and owner-replacement tests continue to pass.
- The declared `client-enqueue-analysis-tests` verification passes.

## Scope Rationale

The CLI and MCP are one scope because MCP already calls the CLI client module. Mark input, settlement notification, analyze admission, and admission observation must change atomically; changing only presentation or only MCP would preserve the bypass through another adapter.

## Out of Scope

- Merging execution marks with queue intent.
- Changing TUI Space or bulk `x` behavior.
- Changing the 10-second stability deadline or analyze capacity policy.
- Automatically unmarking work after admission, completion, or archive beyond existing reconciliation rules.
- Replacing the lower-level `/api/v2 set_queue_intent` command for explicit queue-control callers.
- Changing retry, resolve, stop, dequeue, or scheduler parallelization policy.

The tracked Rust hooks are path-scoped and will run workspace-wide formatting and Clippy when implementation paths are staged. Requirement-specific focused tests remain explicit implementation evidence.
