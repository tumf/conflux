## MODIFIED Requirements

### Requirement: Operation-specific workflow prompts MUST load dedicated skills

Conflux orchestrator prompt builders MUST load operation-specific workflow skills directly when the operation is already known. `apply` MUST load `cflx-apply`, `accept` MUST load `cflx-accept`, `archive` MUST load `cflx-archive`, `cleanup-review` MUST load `cflx-cleanup-review`, and rejecting review MUST load `cflx-rejecting`.

`cflx-workflow` MUST remain available as a backward-compatible router for legacy prompts, but new orchestrator-generated prompts MUST NOT depend on it as the primary source of detailed operation instructions.

#### Scenario: Apply prompt loads cflx-apply directly

- **GIVEN** the orchestrator constructs a prompt for an approved change implementation
- **WHEN** the apply prompt builder emits the skill prelude
- **THEN** the prelude contains `load skills: cflx-apply`
- **AND** it does not rely on `cflx-workflow` as the primary operation skill for new prompts

#### Scenario: Acceptance prompt loads cflx-accept directly

- **GIVEN** the orchestrator constructs an acceptance review prompt for a change
- **WHEN** the acceptance prompt builder emits the skill prelude
- **THEN** the prelude contains `load skills: cflx-accept`

#### Scenario: Archive prompt loads cflx-archive directly

- **GIVEN** the orchestrator constructs an archive prompt for a change
- **WHEN** the archive prompt builder emits the skill prelude
- **THEN** the prelude contains `load skills: cflx-archive`

#### Scenario: Cleanup-review prompt loads cflx-cleanup-review directly

- **GIVEN** the orchestrator constructs a cleanup-review prompt for a change
- **WHEN** the cleanup-review prompt builder emits the skill prelude
- **THEN** the prelude contains `load skills: cflx-cleanup-review`

#### Scenario: Rejecting review loads cflx-rejecting directly

- **GIVEN** a change enters dedicated rejecting review
- **WHEN** the rejecting review prompt is constructed
- **THEN** the prelude contains `load skills: cflx-rejecting`
- **AND** the review still returns only `REJECTION_REVIEW: CONFIRM` or `REJECTION_REVIEW: RESUME`

### Requirement: cflx-workflow MUST remain as a compatibility router

The bundled workflow skill `cflx-workflow` MUST remain installable for backward compatibility, but its primary role SHALL be to route legacy prompts to the correct operation guidance rather than to duplicate the full detailed instructions for every operation. Legacy prompts that load only `cflx-workflow` MUST still be able to execute apply / rejecting / cleanup-review / accept / archive with legacy-equivalent guidance, without requiring additional skill loads or cross-skill auxiliary file access.

#### Scenario: Legacy workflow prompt still has a supported router

- **GIVEN** an older environment emits `load skills: cflx-workflow`
- **WHEN** the skill is loaded with an operation-specific prompt such as apply, accept, archive, cleanup-review, or rejecting review
- **THEN** the skill provides compatibility guidance for that operation
- **AND** the repository still ships dedicated operation-specific skills for new orchestrator prompts

#### Scenario: Legacy workflow prompt remains self-contained

- **GIVEN** an older environment emits `load skills: cflx-workflow`
- **WHEN** the router handles apply / rejecting / cleanup-review / accept / archive
- **THEN** it remains functional without loading additional dedicated skill names in the prompt
- **AND** it does not require cross-skill auxiliary file access to provide legacy-equivalent operation guidance

### Requirement: cflx-accept MUST preserve acceptance command-template single source

The dedicated `cflx-accept` skill MAY provide operation identity and scoped acceptance guidance, but it MUST NOT become the primary source of fixed acceptance procedure. The fixed acceptance procedure MUST remain defined by `.opencode/commands/cflx-accept.md`.

#### Scenario: cflx-accept preserves command-template single source

- **GIVEN** the orchestrator emits `load skills: cflx-accept`
- **WHEN** acceptance runs through the standard command template flow
- **THEN** fixed acceptance procedure still comes from `.opencode/commands/cflx-accept.md`
- **AND** `cflx-accept` does not replace that command template as the primary fixed-instruction source
