## MODIFIED Requirements

### Requirement: Failed Change Tracking

Parallel execution SHALL continue to track failed changes for dependency-skip decisions, but the failure-side terminology MUST distinguish queue-side dependency blocking from resumable execution holds.

A change held because apply cannot proceed yet but remains resumable SHALL be recorded as `stalled`, not `blocked`.

Dependency-based queue waiting SHALL continue to use `blocked` only for unresolved dependency conditions that prevent dispatch.

#### Scenario: stalled apply blocker is recorded as failed without using blocked terminology
- **GIVEN** apply output contains a resumable blocker such as permission auto-reject
- **WHEN** the runtime records the failed change for downstream dependency-skip logic
- **THEN** the change is recorded as `stalled`
- **AND** dependent changes are still eligible for failure-based skip logic
- **AND** the user-facing wording does not describe the change as dependency `blocked`

### Requirement: Permission Auto-Reject Handling

When permission auto-reject is detected during apply, the system MUST stop apply retry for that change and record the change as `stalled`.

The system MUST NOT label this condition as dependency `blocked`.

#### Scenario: permission auto-reject becomes stalled
- **GIVEN** apply output contains `permission requested` and `auto-rejecting`
- **WHEN** the apply loop evaluates the output
- **THEN** the change is recorded as `stalled`
- **AND** apply retry does not continue
- **AND** stall detection via empty WIP commits is skipped for that change
- **AND** the recorded reason includes rejected paths and permission guidance

## ADDED Requirements

### Requirement: Acceptance gating terminology is distinct from dependency blocked

When acceptance detects an implementation blocker, the system SHALL expose that observation as `gated` rather than reusing dependency `blocked` terminology.

Canonical spec prose SHALL describe this concept as `acceptance-gated` when it must be distinguished from `dependency-blocked` in architecture, reducer, or migration guidance.

If acceptance follow-up later routes the change into an apply-side resumable hold, that hold SHALL use the apply-side `stalled` terminology rather than dependency `blocked`.

#### Scenario: acceptance gate wording remains distinct from dependency wait
- **GIVEN** acceptance parsing returns a blocker verdict for change `change-a`
- **WHEN** runtime emits logs, events, or frontend-visible status for that blocker
- **THEN** the blocker is described as `gated`
- **AND** canonical status taxonomy identifies the condition as `acceptance-gated`
- **AND** it is not described as dependency `blocked`
- **AND** any later apply-side hold uses `stalled` wording instead of dependency `blocked`
