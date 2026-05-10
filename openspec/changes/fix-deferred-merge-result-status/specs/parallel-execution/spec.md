## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

#### Scenario: deferred merge is not logged as completed successfully

- **GIVEN** a post-archive merge attempt for change `alpha` returns `MergeAttempt::Deferred`
- **WHEN** queue state handles the background merge task result
- **THEN** operator-visible logs do not say `Background merge task completed successfully for 'alpha'`
- **AND** the task is logged or surfaced as deferred/pending with the deferral reason
- **AND** `alpha` remains in the appropriate merge-wait or resolve-wait state for later retry or operator action

#### Scenario: success follow-up is limited to actual merged outcomes

- **GIVEN** a post-archive merge attempt for change `alpha` returns `MergeAttempt::Deferred`
- **WHEN** queue state processes the result
- **THEN** success-only follow-up behavior for completed merges is not triggered solely because the async task returned without a Rust error
- **AND** no workflow decision is made from log text

#### Scenario: already-integrated change remains successful

- **GIVEN** archive verification or merge verification determines from repository-visible base state that change `alpha` is already integrated
- **WHEN** the merge attempt returns a merged/idempotent success outcome
- **THEN** queue state may log background merge completion for `alpha`
- **AND** success-only follow-up behavior may run
