## MODIFIED Requirements

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI `AppMode::Error` MUST be reserved for fatal global execution errors that stop or invalidate the active orchestration run. Event type, rather than diagnostic message content, MUST determine whether a global error is fatal. A recoverable dependency-analysis failure followed by successful metadata-dependency-only fallback MUST arrive through a non-fatal warning event and MUST NOT replace the active `Running` lifecycle presentation. A global fatal error MUST NOT be downgraded because its message contains or quotes recoverable fallback wording.

A post-archive merge or resolve failure that is scoped to one change, preserves its worktree, returns that change to `MergeWait`, and leaves the scheduler able to continue MUST arrive through a change-scoped event carrying the change ID and MUST NOT enter global TUI Error. When no other active change remains, the existing active-work transition MAY return the TUI to Select.

TUI merge-deferred diagnostics caused by retry scheduling SHALL remain bounded when the same change repeatedly receives the same merge-deferred reason and retry classification. Exact duplicate diagnostics MUST NOT flood the visible log, while distinct reasons for the same change MUST remain visible.

This diagnostic presentation is UI observability behavior only and MUST NOT be used as workflow-control input.

#### Scenario: exhausted post-archive resolve remains change-scoped

- **GIVEN** the TUI execution lifecycle is `Running`
- **AND** change `alpha` exhausts its post-archive conflict-resolution attempts
- **AND** repository and worktree evidence for `alpha` remain available for explicit retry
- **WHEN** the TUI receives the resulting execution events
- **THEN** `alpha` SHALL be displayed as `merge wait`
- **AND** the failure diagnostic SHALL retain `alpha` as structured change identity
- **AND** the TUI execution lifecycle SHALL NOT become `Error`
- **AND** the TUI SHALL remain `Running` while other active work exists

#### Scenario: no active work after change-scoped merge failure returns to Select

- **GIVEN** the TUI execution lifecycle is `Running`
- **AND** `alpha` is the only active change
- **WHEN** a change-scoped post-archive resolve failure returns `alpha` to manual `MergeWait`
- **THEN** the existing active-work transition MAY set the TUI execution lifecycle to `Select`
- **AND** it SHALL NOT set the lifecycle to `Error`
- **AND** explicit merge retry for `alpha` SHALL remain available

#### Scenario: genuine global failure still enters Error

- **GIVEN** orchestration encounters a failure that invalidates the active run and has no safe scheduler continuation
- **WHEN** the TUI receives the typed global fatal error event
- **THEN** the TUI execution lifecycle SHALL become `Error`
- **AND** change-local merge failure handling SHALL NOT downgrade or suppress that event
