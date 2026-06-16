## MODIFIED Requirements

### Requirement: Dependency-blocked diagnostics are stable and non-spamming

The scheduler SHALL preserve dependency-blocked state for queued changes that cannot dispatch, but it MUST NOT repeatedly emit identical operator-visible blocked/error diagnostics while the blocked change has the same repository-visible dependency blocker signature.

A blocker signature SHALL include at least the blocked change id, dependency ids, and dependency target classes. When the signature changes, the scheduler SHALL emit a fresh diagnostic and re-evaluate dispatch using the updated dependency evidence.

Dispatch-capacity-zero diagnostics SHALL be treated as operator-visible diagnostics subject to the same stability and non-spamming rule. The signature for a capacity-zero diagnostic SHALL include at least the analysis order (or queued change ids), `queued.len()`, `in_flight.len()`, and `max_parallelism`. When any component of the signature changes, the scheduler SHALL emit a fresh diagnostic.

All operator-visible scheduler diagnostics (including but not limited to dependency-blocked, capacity-zero, no-analysis, analysis-failure, queue-reconciliation, and merge-deferred) SHALL be emitted through a single unified `DiagnosticDeduplicationStore` implementation. Each diagnostic type SHALL register its own key shape with the store; duplicate keys for the same type SHALL suppress repeated operator-visible events.

#### Scenario: Repeated identical capacity-zero state does not spam logs

- **GIVEN** the scheduler has already emitted the `dispatch_capacity_zero_after_analysis` diagnostic for a given `(order, queued.len(), in_flight.len(), max_parallelism)` signature
- **WHEN** later scheduler re-analysis loops observe the identical zero-capacity signature
- **THEN** no duplicate operator-visible `dispatch_capacity_zero_after_analysis` log is appended
- **AND** dispatch remains suppressed

#### Scenario: Changed capacity-zero signature emits a fresh diagnostic

- **GIVEN** the scheduler previously emitted `dispatch_capacity_zero_after_analysis` for a signature with `in_flight.len() == 3`
- **WHEN** an in-flight change completes and `in_flight.len()` decreases to 2 while `queued` work remains
- **THEN** the scheduler emits a fresh `dispatch_capacity_zero_after_analysis` diagnostic reflecting the updated signature
- **AND** ordinary apply dispatch remains suppressed until a positive slot count is observed

#### Scenario: All diagnostic types share a unified deduplication implementation

- **GIVEN** the scheduler emits diagnostics of any of the nine supported types
- **WHEN** the same diagnostic key is observed twice without an intervening state change
- **THEN** the second emission is suppressed by the single `DiagnosticDeduplicationStore` instance
- **AND** no per-type HashSet boilerplate remains in `ParallelExecutor`
