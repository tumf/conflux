## ADDED Requirements

### Requirement: Reanalysis dispatch guards are factored by responsibility

The scheduler's reanalysis-and-dispatch path SHALL be decomposed into single-responsibility guard and action helpers instead of one monolithic function. The top-level reanalysis function SHALL read as an orchestration skeleton and delegate classification, reanalysis reason computation, debounce evaluation, executable filtering, analysis execution, post-analysis capacity handling, and dispatch execution to explicit helpers.

#### Scenario: Queue notification debounce behavior remains unchanged after extraction

- **GIVEN** `last_queue_change_at` is fresh
- **AND** scheduler iteration is greater than 1
- **AND** the reanalysis reason is `QueueNotification`
- **WHEN** the refactored reanalysis path evaluates whether to analyze
- **THEN** analysis starts immediately without waiting for the debounce window
- **AND** an `AnalysisStarted` event is emitted

#### Scenario: Zero-capacity behavior remains unchanged after extraction

- **GIVEN** queued dispatchable work exists
- **AND** `in_flight.len() == max_parallelism`
- **WHEN** the refactored reanalysis path runs dependency analysis
- **THEN** dependency analysis still runs
- **AND** ordinary apply dispatch is suppressed
- **AND** the capacity-zero diagnostic is emitted through the diagnostic deduplication store

#### Scenario: Blocked-only work skips analyzer after extraction

- **GIVEN** queued work is entirely merge-wait or terminal-error blocked
- **WHEN** the refactored reanalysis path classifies queued work
- **THEN** the dependency analyzer is not invoked
- **AND** a no-analysis diagnostic is emitted through the diagnostic deduplication store
