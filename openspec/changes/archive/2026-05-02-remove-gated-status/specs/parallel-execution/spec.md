## MODIFIED Requirements

### Requirement: Acceptance blocker input compatibility is distinct from lifecycle display taxonomy

When acceptance detects an implementation blocker, the system SHALL NOT expose that observation as `gated` in user-facing lifecycle or display taxonomy. The runtime SHALL treat the condition as a non-terminal stalled/review hold while preserving reason metadata such as `acceptance-gated` when the cause must be distinguished from dependency `blocked`.

The canonical machine-readable acceptance verdict parser MAY continue to accept `gated` input for compatibility. During migration, runtimes MAY continue to accept legacy `blocked` acceptance verdict input for backward compatibility. Newly authored lifecycle/status surfaces, operator-facing docs, and UI tests MUST NOT require `gated` as a display status.

If acceptance follow-up later routes the change into a resumable hold, that hold SHALL use `stalled` terminology rather than dependency `blocked` or display `gated`.

#### Scenario: canonical acceptance blocker displays as stalled
- **GIVEN** acceptance detects an implementation blocker for change `change-a`
- **WHEN** the runtime exposes the lifecycle/display status
- **THEN** the displayed status is `stalled`
- **AND** new prompts and tests do not require `gated` as a lifecycle/display term
- **AND** dependency wait remains the only `blocked` display meaning

#### Scenario: gated verdict input remains parser-compatible during migration
- **GIVEN** an acceptance integration emits `gated`
- **WHEN** a compatibility-aware runtime parses that verdict
- **THEN** the runtime interprets it as an acceptance blocker observation
- **AND** the user-facing lifecycle taxonomy describes the paused condition as `stalled`, not `gated`

#### Scenario: legacy blocked acceptance verdict remains backward compatible during migration
- **GIVEN** an older acceptance integration still emits `blocked`
- **WHEN** a compatibility-aware runtime parses that verdict
- **THEN** the runtime still interprets it as an acceptance blocker observation
- **AND** canonical user-facing taxonomy describes the paused condition as `stalled`
