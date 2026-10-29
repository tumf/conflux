## ADDED Requirements

### Requirement: Returned dependency analysis closes its process activity

The parallel analysis producer SHALL emit the existing AnalysisCompleted presentation event exactly once after each normally returned analyzer invocation and before every result-handling return. Completion SHALL describe invocation lifetime only; it SHALL NOT change result provenance, workflow state, retry, or dispatch policy. Existing wire fields SHALL remain unchanged, with groups_found equal to the returned legacy groups length or zero when absent.

#### Scenario: Returned result retires only analysis activity

- **GIVEN** the real producer emitted AnalysisStarted and the shared facts store records DependencyAnalysis
- **WHEN** the analyzer returns healthy, metadata-only, fallback, or an empty order
- **THEN** the producer emits one AnalysisCompleted through the existing dispatcher
- **AND** the facts store removes only DependencyAnalysis, preserving unrelated active work
- **AND** a process with no other work no longer reports active work solely from that analysis

#### Scenario: Unusable or zero-capacity result does not bypass closure

- **WHEN** a returned result reaches empty-order retry or capacity-zero handling
- **THEN** activity closes before that path returns
- **AND** the existing retry and suppression policy is unchanged
- **AND** no workflow completion or stopped event is fabricated

#### Scenario: Pending invocation is not complete

- **GIVEN** analysis has started and its future has not returned
- **WHEN** it is still pending or its future is dropped before returning
- **THEN** no AnalysisCompleted is emitted for an unreturned invocation
- **AND** existing process-terminal cleanup remains responsible for retiring activity from a dropped invocation
- **AND** an analyzer that handles cancellation internally and returns an outcome is a returned invocation and SHALL emit completion
