## ADDED Requirements

### Requirement: Acceptance owns behavior-task adequacy review

Behavior-changing proposals MUST have their implementation-task adequacy judged by acceptance review rather than by native validator wording heuristics. Acceptance MUST fail when a proposal claims runtime or user-visible behavior changes but the change tasks and repository evidence do not identify concrete implementation-facing work or integration points sufficient to deliver that behavior.

#### Scenario: acceptance fails behavior-changing proposal lacking concrete implementation evidence

- **GIVEN** an implementation or hybrid proposal claims runtime or user-visible behavior changes
- **AND** the change tasks do not identify concrete implementation-facing work or repository-verifiable integration evidence for delivering that behavior
- **WHEN** acceptance review evaluates the change
- **THEN** acceptance returns FAIL with actionable findings citing the missing code/test/integration evidence
- **AND** archive does not become the first phase that surfaces this proposal-quality issue
