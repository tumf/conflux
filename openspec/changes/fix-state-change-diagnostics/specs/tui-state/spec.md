## ADDED Requirements

### Requirement: Scheduler dependency diagnostics are state-transition driven

The scheduler MUST emit dependency blocked/resolved diagnostics and events based on dependency blocker state transitions, not merely because a polling loop re-checked the same queued change.

For each blocked queued change, the scheduler MUST compare the current dependency blocker observation to the last emitted blocker observation for that change. The blocker observation MUST distinguish at least the blocked change id, unresolved dependency ids, and dependency target classes. Equivalent blocker observations MUST be treated as no-ops for diagnostic/event emission.

Any remembered blocker observation state MUST be in-memory and non-authoritative. It MUST NOT be persisted under `~/.local/state/cflx/**`, and it MUST NOT be used to decide scheduling eligibility, resume routing, acceptance routing, archive routing, or next-action behavior.

#### Scenario: unchanged dependency blocker emits once

- **GIVEN** change `feature-b` is queued
- **AND** change `feature-b` is blocked by dependency `feature-a`
- **AND** the scheduler has already emitted a `DependencyBlocked` diagnostic/event for the same blocker observation
- **WHEN** the scheduler loop evaluates `feature-b` again and `feature-a` has not changed dependency class or resolution state
- **THEN** no additional `DependencyBlocked` event is emitted for `feature-b`
- **AND** no additional TUI user-visible dependency blocked log is produced for that unchanged blocker observation

#### Scenario: changed dependency blocker emits again

- **GIVEN** change `feature-b` was previously blocked by dependency `feature-a`
- **WHEN** the blocker observation changes, such as the unresolved dependency set changes or `feature-a` changes from queued to rejected
- **THEN** the scheduler emits a new dependency blocked diagnostic/event for `feature-b`
- **AND** the diagnostic identifies the changed blocker state rather than silently suppressing it

#### Scenario: dependency resolution emits once per blocked transition

- **GIVEN** change `feature-b` previously emitted a dependency blocked diagnostic/event
- **WHEN** its dependencies become resolved
- **THEN** the scheduler emits one `DependencyResolved` event for `feature-b`
- **AND** later scheduler loops do not re-emit `DependencyResolved` while `feature-b` remains unblocked
- **AND** if `feature-b` becomes blocked again later, that later blocked transition can emit a new blocked diagnostic/event

#### Scenario: diagnostic suppression does not control scheduling

- **GIVEN** a dependency blocker observation has been remembered for diagnostic suppression
- **WHEN** the scheduler evaluates which changes are executable
- **THEN** executable selection is still derived from analysis, workspace state, git state, and in-flight execution state
- **AND** deleting external log/state directories such as `~/.local/state/cflx/**` does not change the next action chosen for the same workspace contents

### Requirement: TUI dependency transition logs are idempotent

TUI handling of dependency blocked and dependency resolved events MUST be idempotent for user-visible logs. A duplicate event that does not change the displayed row status MUST NOT append another identical user-visible log entry.

#### Scenario: duplicate dependency blocked event is a TUI log no-op

- **GIVEN** change `feature-b` is already displayed as `blocked` in the TUI
- **WHEN** the TUI receives another dependency blocked event for `feature-b` without a display state transition
- **THEN** the TUI keeps `feature-b` displayed as `blocked`
- **AND** the TUI does not append another identical dependency blocked log entry

#### Scenario: dependency resolved logs only on blocked-to-queued transition

- **GIVEN** change `feature-b` is displayed as `blocked` in the TUI
- **WHEN** the TUI receives a dependency resolved event for `feature-b`
- **THEN** the TUI changes the displayed status to `queued`
- **AND** the TUI appends one dependency resolved log entry
- **WHEN** the TUI receives another dependency resolved event for `feature-b` while it is no longer displayed as `blocked`
- **THEN** the TUI does not append another dependency resolved log entry
