# Context

Execution marks are process-local operator selection intent. Reducer queue intent is the scheduler-visible admission source. They are related but remain separate authorities.

The current settlement path is additive-only: it inspects marked IDs and plans additions. Therefore unmarking an ordinary pending row cannot publish `queued: false`, leaving the TUI gray/queued and leaving analysis input stale.

A global bidirectional scan would be unsafe. Explicit API/TUI queue additions do not create marks, and explicit queue removal does not necessarily clear marks. Reconciliation must therefore be scoped to targets whose marks actually changed.

# Decisions

## 1. One rule for individual, bulk, and API mark mutation

Every accepted mark delta enters the same process-local settlement batch. Individual and bulk controls differ only in how many accepted deltas they produce.

After the existing stability window, settlement re-reads only the targets named by that batch from one coherent current snapshot. It does not scan or mutate unrelated queue intent.

## 2. Bidirectional, delta-scoped reconciliation

At expiry:

- marked + tracked + parallel-eligible + ordinary `not queued`: add queue intent;
- unmarked + reducer intent `Queued` + idle ordinary pending: remove queue intent;
- already aligned state: no-op;
- active, in-flight, lane-wait, retry, MergeWait, ResolveWait, RejectWait, blocked, stalled, terminal, archive-complete, unknown, or ineligible: fail closed with stable exclusion evidence.

Explicitly queued unmarked rows and marked rows explicitly removed from queue remain unchanged unless their own mark changed in the current batch.

## 3. Application-time lifecycle guard

Classification can race with dispatch or terminal transition. Each settlement-derived queue operation therefore revalidates under the authoritative reducer write boundary.

- Removal becomes a reasoned no-op if the target became active, in-flight, waiting, terminal, or otherwise excluded. It never clears active lifecycle evidence.
- Addition becomes a reasoned no-op if the target became terminal-error or otherwise excluded. It never aliases `RetryError` or publishes an explicit-retry edge.

Unmarking never cancels, stops, dequeues, changes phase, or interrupts work.

## 4. One scheduler notification per applied batch

Per-target queue hooks retain their existing exactly-once mutation semantics. Scheduler notification is emitted exactly once after a settled batch if at least one queue membership mutation applied, including removal-only batches. No applied mutation means no notification.

Frontend and settlement do not decide whether Analyze starts.

## 5. Capacity-gated analyzer owned by scheduler

Queue classification, reducer reconciliation, and diagnostics remain available with zero worker slots. The expensive dependency analyzer and ordinary dispatch require a freshly recomputed positive slot count.

When capacity is zero, scheduler records neither a completed/suppression signature nor consumption of an unevaluated edge. Slot recovery supplies the immediate liveness edge; capacity in the runtime signature provides the bounded timer fallback.

Analyzer starts only when:

1. available slots are positive;
2. at least one eligible ordinary queued candidate remains; and
3. an unconsumed explicit edge exists or the current signature differs from the last usable completed analysis.

An empty eligible queue never starts Analyze.

# Rejected alternatives

- **Global mark/queue convergence:** destroys explicit queue intent unrelated to the mark mutation.
- **Make queue intent create/clear marks:** expands authority and contradicts existing queue-presentation requirements.
- **Gate only settlement notification:** timer/signature evaluation would still start Analyze at zero capacity.
- **Let each addition notify scheduler:** creates duplicate analysis attempts for one settled batch.

# Verification strategy

Use paused time for the stability window. Operator tests cover target scoping, both queue directions, hooks, active/race guards, exclusions, restart, and individual/bulk/API equivalence. Parallel scheduler tests cover zero-capacity suppression, no signature/edge consumption, empty queue suppression, and slot-recovery analysis.
