## ADDED Requirements

### Requirement: Acceptance prompt MUST evaluate spec-only archive readiness

Acceptance guidance MUST detect `spec-only` changes and evaluate whether their deltas can be promoted into canonical specs safely. It MUST fail when archive simulation indicates a no-op promotion or an unresolved `MODIFIED` / `REMOVED` target.

#### Scenario: Spec-only acceptance fails on archive no-op risk
- **GIVEN** acceptance reviews a change classified as `spec-only`
- **AND** archive simulation shows that promoting the delta would not change the touched canonical spec
- **WHEN** acceptance evaluates the change
- **THEN** acceptance outputs `FAIL`
- **AND** the follow-up tasks tell the agent to fix the archive-readiness issue instead of asking for unrelated runtime evidence
