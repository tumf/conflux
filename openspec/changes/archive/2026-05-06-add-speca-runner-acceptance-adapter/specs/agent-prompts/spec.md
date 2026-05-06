## MODIFIED Requirements

### Requirement: Built-in SPECA acceptance skill

The orchestrator MUST include a built-in `cflx-accept-with-speca` skill that can be selected as the acceptance operation skill.

The `cflx-accept-with-speca` skill MUST preserve the Conflux acceptance verdict contract. It MUST produce exactly one final machine-readable acceptance verdict using the existing `pass`, `fail`, `continue`, or `gated` outcomes, with actionable `findings` for fail outcomes.

The skill MUST treat `.opencode/commands/cflx-accept.md` and the standard `cflx-accept` acceptance contract as the authoritative source for fixed checks and final verdict formatting.

The skill SHOULD guide acceptance review to derive or select SPECA-style properties from OpenSpec deltas, task claims, changed files, and constitution constraints; perform a property-grounded proof attempt when tooling and context are available; and map blocking property failures into the existing acceptance verdict format.

When the official NyxFoundation/speca runner is available and usable outside the Conflux worktree, the skill MUST guide the reviewer to attempt official SPECA runner execution as supporting evidence using the checked-out runner's documented `uv run python3 scripts/run_phase.py ...` command shape. The skill MUST require generated runner inputs, outputs, logs, and tool checkout/cache files to remain outside the Conflux worktree by default.

The skill MUST treat official SPECA runner outputs as supporting proof/falsification evidence only. Runner outputs, logs, caches, and temporary inputs MUST NOT become authoritative workflow-control state for pass/fail/continue/gated routing.

The skill MUST require fallback to manual SPECA-style property review when the official runner, prerequisites, authentication/session access, or usable outputs are unavailable. Runner unavailability MUST NOT be treated as an automatic pass and MUST NOT introduce a SPECA-specific verdict format.

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

#### Scenario: official SPECA runner is attempted when usable

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **AND** a NyxFoundation/speca checkout and prerequisites are available outside the Conflux worktree
- **WHEN** the reviewer performs the SPECA proof-attempt pass
- **THEN** the skill guides the reviewer to prepare inputs outside the Conflux worktree
- **AND** the reviewer attempts the official runner using the installed checkout's documented `uv run python3 scripts/run_phase.py ...` command shape
- **AND** any produced official outputs are considered supporting evidence for standard acceptance findings

#### Scenario: official SPECA runner artifacts remain non-authoritative

- **GIVEN** official SPECA execution produces logs, caches, temporary inputs, or output reports outside the Conflux worktree
- **WHEN** the reviewer decides the final acceptance outcome
- **THEN** workflow-control routing is still based on workspace files, workspace git state, base-branch comparison, tests, and repository evidence
- **AND** deleting the out-of-worktree SPECA artifacts would not change the next Conflux action for the same workspace contents

#### Scenario: SPECA tooling unavailable falls back without protocol drift

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **AND** external SPECA tooling is unavailable in the agent environment
- **WHEN** the reviewer completes acceptance using available repository context
- **THEN** the reviewer still returns one of the existing Conflux acceptance verdicts
- **AND** it does not emit a SPECA-specific verdict format outside the Conflux acceptance contract
- **AND** unavailable tooling is not treated as an automatic pass

#### Scenario: official SPECA runner fails or lacks auth

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **AND** the official SPECA runner cannot complete because prerequisites, auth/session access, or usable outputs are unavailable
- **WHEN** the reviewer completes acceptance
- **THEN** the reviewer records the limitation in human-readable reasoning
- **AND** continues with manual SPECA-style property review against repository evidence
- **AND** emits only the standard Conflux acceptance verdict

#### Scenario: SPECA acceptance remains autonomous and workspace-local

- **GIVEN** acceptance is using `cflx-accept-with-speca`
- **WHEN** the reviewer evaluates a change
- **THEN** the skill instructs the reviewer not to ask user questions
- **AND** workflow-control decisions are based on repository/workspace evidence rather than out-of-worktree durable state
