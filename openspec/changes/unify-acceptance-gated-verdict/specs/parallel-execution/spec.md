## MODIFIED Requirements

### Requirement: Acceptance gating terminology is distinct from dependency blocked

When acceptance detects an implementation blocker, the system SHALL expose that observation as `gated` rather than reusing dependency `blocked` terminology.

The canonical machine-readable acceptance verdict vocabulary SHALL use `pass`, `fail`, `continue`, and `gated`.

During migration, runtimes MAY continue to accept legacy `blocked` acceptance verdict input for backward compatibility, but newly authored prompts, specs, and tests MUST treat `gated` as the canonical acceptance verdict term.

Canonical spec prose SHALL describe this concept as `acceptance-gated` when it must be distinguished from `dependency-blocked` in architecture, reducer, or migration guidance.

If acceptance follow-up later routes the change into an apply-side resumable hold, that hold SHALL use the apply-side `stalled` terminology rather than dependency `blocked`.

#### Scenario: canonical acceptance verdict uses gated terminology
- **GIVEN** acceptance detects an implementation blocker for change `change-a`
- **WHEN** the acceptance command emits its machine-readable verdict
- **THEN** the canonical verdict uses `gated`
- **AND** new prompts and tests do not require `blocked` as the acceptance verdict term

#### Scenario: legacy blocked acceptance verdict remains backward compatible during migration
- **GIVEN** an older acceptance integration still emits `blocked`
- **WHEN** a compatibility-aware runtime parses that verdict
- **THEN** the runtime still interprets it as an acceptance gate observation
- **AND** canonical user-facing taxonomy continues to describe the condition as `gated` / `acceptance-gated`
