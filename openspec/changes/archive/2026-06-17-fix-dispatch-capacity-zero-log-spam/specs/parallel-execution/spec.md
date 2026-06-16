## MODIFIED Requirements

### Requirement: Dependency-blocked diagnostics are stable and non-spamming

The scheduler SHALL preserve dependency-blocked state for queued changes that cannot dispatch, but it MUST NOT repeatedly emit identical operator-visible blocked/error diagnostics while the blocked change has the same repository-visible dependency blocker signature.

A blocker signature SHALL include at least the blocked change id, dependency ids, and dependency target classes. When the signature changes, the scheduler SHALL emit a fresh diagnostic and re-evaluate dispatch using the updated dependency evidence.

Dispatch-capacity-zero diagnostics SHALL be treated as operator-visible diagnostics subject to the same stability and non-spamming rule. The signature for a capacity-zero diagnostic SHALL include at least the analysis order (or queued change ids), `queued.len()`, `in_flight.len()`, and `max_parallelism`. When any component of the signature changes, the scheduler SHALL emit a fresh diagnostic.

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
