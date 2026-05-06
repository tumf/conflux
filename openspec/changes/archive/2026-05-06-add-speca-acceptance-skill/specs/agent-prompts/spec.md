## ADDED Requirements

### Requirement: Built-in SPECA acceptance skill

The orchestrator MUST include a built-in `cflx-accept-with-speca` skill that can be selected as the acceptance operation skill.

The `cflx-accept-with-speca` skill MUST preserve the Conflux acceptance verdict contract. It MUST produce exactly one final machine-readable acceptance verdict using the existing `pass`, `fail`, `continue`, or `gated` outcomes, with actionable `findings` for fail outcomes.

The skill MUST treat `.opencode/commands/cflx-accept.md` and the standard `cflx-accept` acceptance contract as the authoritative source for fixed checks and final verdict formatting.

The skill SHOULD guide acceptance review to derive or select SPECA-style properties from OpenSpec deltas, task claims, changed files, and constitution constraints; perform a property-grounded proof attempt when tooling and context are available; and map blocking property failures into the existing acceptance verdict format.

The skill MUST NOT require changing `acceptance_command` merely to opt into SPECA-oriented acceptance behavior.

#### Scenario: cflx-accept-with-speca is available as a built-in skill

- **GIVEN** Conflux exposes its bundled skills to an agent runtime
- **WHEN** the built-in skill inventory is inspected
- **THEN** `cflx-accept-with-speca` is present
- **AND** `cflx-accept` remains present

#### Scenario: SPECA skill maps property failure to standard verdict

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **AND** a SPECA-style property proof attempt finds a blocking implementation mismatch with concrete repository evidence
- **WHEN** the acceptance reviewer returns a final verdict
- **THEN** the verdict uses the existing JSON `fail` outcome
- **AND** the `findings` array includes the property failure and concrete actionable evidence

#### Scenario: SPECA tooling unavailable falls back without protocol drift

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **AND** external SPECA tooling is unavailable in the agent environment
- **WHEN** the reviewer completes acceptance using available repository context
- **THEN** the reviewer still returns one of the existing Conflux acceptance verdicts
- **AND** it does not emit a SPECA-specific verdict format outside the Conflux acceptance contract
- **AND** unavailable tooling is not treated as an automatic pass

#### Scenario: SPECA acceptance remains autonomous and workspace-local

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **WHEN** the reviewer evaluates a change
- **THEN** the skill instructs the reviewer not to ask user questions
- **AND** workflow-control decisions are based on repository/workspace evidence rather than out-of-worktree durable state
