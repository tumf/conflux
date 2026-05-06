## ADDED Requirements

### Requirement: Acceptance prompt MUST evaluate implementation blockers

Acceptance prompts and distributed acceptance-related skills MUST frame valid Implementation Blockers as stalled acceptance holds in user-facing guidance. During the compatibility period, the runtime-compatible acceptance verdict token MAY remain `{"acceptance":"gated"}` / `ACCEPTANCE: GATED`, but distributed guidance MUST present that token as a protocol compatibility handoff for a stalled hold rather than as a user-facing lifecycle/status or primary rubric label.

Acceptance-related skills MUST NOT instruct agents to emit `{"acceptance":"stalled"}` as the final machine-readable verdict until runtime parser support for that verdict exists in a separate change.

#### Scenario: distributed skills describe implementation blockers as stalled holds

- **GIVEN** bundled acceptance-related skills under `skills/` are reviewed
- **WHEN** they explain how to handle a valid `Implementation Blocker #<n>`
- **THEN** the primary operator-facing concept is a stalled acceptance hold
- **AND** `gated` appears only as current runtime-compatible protocol/fallback token wording or reason metadata
- **AND** the guidance does not describe `gated` as a lifecycle/display status

#### Scenario: distributed skills preserve parser-compatible blocker handoff

- **GIVEN** current runtime parser compatibility still depends on the `gated` acceptance token for implementation-blocker handoff
- **WHEN** distributed skills specify the final machine-readable verdict for a valid stalled implementation blocker hold
- **THEN** they continue to instruct the parser-compatible `{"acceptance":"gated"}` verdict and legacy `ACCEPTANCE: GATED` fallback where needed
- **AND** they explicitly state that these tokens represent a stalled acceptance hold
- **AND** they do not instruct `{"acceptance":"stalled"}` until a runtime parser migration supports it

### Requirement: Acceptance skills MUST define a JSON-primary verdict contract

The primary acceptance verdict contract MUST be a strict JSON object emitted as the final machine-readable verdict payload. The runtime MAY continue to accept legacy plain-text standalone lines such as `ACCEPTANCE: PASS` as a backward-compatible fallback, but canonical guidance MUST prefer the JSON contract.

#### Scenario: cflx-accept defines JSON-primary verdict contract

- **GIVEN** the acceptance prompt loads `cflx-accept`
- **WHEN** the skill describes the final verdict format
- **THEN** it defines a strict JSON verdict object as the primary machine-readable contract
- **AND** it documents plain-text standalone verdict markers only as backward-compatible fallback guidance
- **AND** it does not require `.opencode/commands/` files or OpenCode slash command invocation to describe the verdict interface

#### Scenario: repo-local acceptance skills follow the same contract

- **GIVEN** repo-local acceptance-related skills under `skills/` are reviewed
- **WHEN** they describe acceptance output expectations
- **THEN** they reference the same JSON-primary verdict contract
- **AND** they do not redefine a conflicting text-only canonical output rule
- **AND** they do not treat OpenCode command templates as the authoritative source for the skill interface
