---
change_type: implementation
priority: medium
dependencies: []
references:
  - "src/parallel/queue_state.rs:2617-2642"
  - "openspec/specs/parallel-execution/spec.md:157 (Dependency-blocked diagnostics are stable and non-spamming)"
  - "openspec/specs/tui-state/spec.md:214 (Scheduler dependency diagnostics are state-transition driven)"
  - "openspec/specs/tui-state/spec.md:253 (TUI dependency transition logs are idempotent)"
  - "openspec/changes/fix-queue-notification-reanalysis"
---

# Fix repeated dispatch capacity zero diagnostic spam in TUI

**Change Type**: implementation

## Problem / Context

When the scheduler runs `perform_reanalysis_and_dispatch` and `available_slots == 0` after dependency analysis, it unconditionally emits the following operator-visible log via `ParallelEvent::Log`:

```
Dispatch suppressed after dependency analysis: reason=dispatch_capacity_zero_after_analysis, local_queued=..., in_flight=..., max_parallelism=...
```

(See: `src/parallel/queue_state.rs:2625-2634`)

In TUI `Persistent` mode with manual resolve active (or any situation where `in_flight == max_parallelism`), this state persists across multiple re-analysis iterations. Because no deduplication key guards this specific log emission, the same message repeats on every loop iteration, producing log spam visible to the operator.

Existing mechanisms:

- `emit_no_analysis_diagnostic` dedupes reducer-visible "no analysis started" diagnostics via `no_analysis_diagnostics_seen` HashSet.
- Canonical specs already require state-transition-driven, non-spamming diagnostics:
  - `parallel-execution`: "Dependency-blocked diagnostics are stable and non-spamming"
  - `tui-state`: "Scheduler dependency diagnostics are state-transition driven", "TUI dependency transition logs are idempotent"

However, the capacity-zero dispatch diagnostic path bypasses these guards.

The active change `fix-queue-notification-reanalysis` addresses queue-notification timing and explicitly preserves zero-capacity behavior; it does not solve the repeated-log problem.

## Proposed Solution

Introduce a dedicated deduplication guard for the capacity-zero dispatch diagnostic (`dispatch_capacity_zero_after_analysis`) that is:

- Keyed on the observable state that affects the message: `(analysis order or queued ids, queued.len(), in_flight.len(), max_parallelism, reason)`.
- Stored in an in-memory `HashSet` (analogous to `no_analysis_diagnostics_seen`).
- Skips emitting the duplicate `ParallelEvent::Log` when the key is unchanged.
- Emits a fresh diagnostic when any component of the key changes (new queued change, slot freed, parallelism changed, etc.).
- Preserves all existing zero-capacity semantics: analysis still runs, dispatch remains suppressed, tests asserting `AnalysisStarted` + `dispatch_capacity_zero_after_analysis` + no `ApplyStarted` continue to pass.

Add a regression test that repeatedly triggers re-analysis under zero-capacity and asserts the capacity-zero log appears at most once for an unchanged key.

Add a minimal `MODIFIED Requirements` delta under `parallel-execution` extending the existing "Dependency-blocked diagnostics are stable and non-spamming" requirement to cover dispatch-capacity-zero diagnostics.

## Acceptance Criteria

- Repeated re-analysis under identical zero-capacity state emits the capacity-zero dispatch diagnostic at most once.
- When the zero-capacity signature changes (different queued count, different in-flight count, different max_parallelism, different analysis order), a fresh diagnostic is emitted.
- Zero-capacity suppression semantics remain unchanged: analysis runs, no ordinary apply dispatch occurs.
- Existing integration tests (`scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`) continue to pass and observe at least one capacity diagnostic.
- New regression test covers repeated zero-capacity re-analysis without log spam.
- Strict validation passes with evidence warnings resolved.

## Explicit Completion Conditions

- `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` passes and the test still asserts `saw_capacity_diagnostic`.
- New test (e.g., `repeated_capacity_zero_does_not_spam_dispatch_diagnostic`) runs under zero-capacity, triggers ≥2 re-analysis iterations, and asserts the capacity-zero log appears exactly once for the initial key.
- `src/parallel/queue_state.rs` contains a new helper or guard (`emit_capacity_zero_dispatch_diagnostic_once` or equivalent) used at line ~2625.
- `openspec/changes/fix-dispatch-capacity-zero-log-spam/specs/parallel-execution/spec.md` adds a `MODIFIED Requirements` entry that references the exact canonical heading from `openspec/specs/parallel-execution/spec.md:157`.
- `cflx openspec validate fix-dispatch-capacity-zero-log-spam --strict --evidence warn` passes with no outstanding warnings about missing ownership or absent runnable verification.

## Out of Scope

- Changing the internal `info!` tracing log (only the TUI-visible `ParallelEvent::Log` is deduped).
- Altering zero-capacity scheduling policy or manual/auto resolve gate behavior.
- Adding cross-process or persisted dedup state (in-memory only, per Constitution).
- Modifying the active `fix-queue-notification-reanalysis` change.
