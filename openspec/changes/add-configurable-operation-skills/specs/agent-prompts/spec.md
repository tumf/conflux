## MODIFIED Requirements

### Requirement: Operation prompts MUST use selected skill preludes

Operation prompt construction MUST prepend exactly one selected operation skill prelude using `load skills: <skill-name>`, where `<skill-name>` is the effective configured skill for that operation and defaults to the current built-in Conflux operation skill when omitted.

Selecting a different operation skill MUST NOT duplicate fixed operation procedures inside variable prompt context, and MUST NOT change command execution, parser behavior, or workflow-control semantics by itself.

#### Scenario: apply prompt uses configured apply skill

- **GIVEN** the effective `apply_skill` is `team-apply`
- **WHEN** an apply prompt is constructed
- **THEN** the prompt contains `load skills: team-apply`
- **AND** the prompt still contains the existing apply variable context after the selected skill prelude

#### Scenario: acceptance prompt keeps context-only payload

- **GIVEN** acceptance_command uses `/cflx-accept {change_id} {prompt}`
- **AND** the effective `accept_skill` is `cflx-accept-with-speca`
- **WHEN** acceptance prompt construction builds the `{prompt}` payload
- **THEN** the payload contains `load skills: cflx-accept-with-speca`
- **AND** the payload still contains change metadata, paths, diff context, archive readiness context, previous acceptance output, user acceptance prompt, and history in the existing relative order
- **AND** the payload does not embed a second fixed acceptance checklist or a different verdict protocol

#### Scenario: acceptance full mode remains a compatibility alias

- **GIVEN** acceptance_prompt_mode is set to `full`
- **WHEN** acceptance prompt construction builds the `{prompt}` payload
- **THEN** embedded fixed acceptance instructions are not injected
- **AND** the payload contains only the same variable context as `context_only`, headed by the selected acceptance skill prelude

#### Scenario: analyze prompt uses configured analyze skill

- **GIVEN** the effective `analyze_skill` is `team-analyze`
- **WHEN** a dependency-analysis prompt is constructed
- **THEN** the prompt contains `load skills: team-analyze`
- **AND** dependency selection output semantics remain unchanged

#### Scenario: conflict resolve prompt uses configured resolve skill

- **GIVEN** the effective `resolve_skill` is `team-resolve`
- **WHEN** a conflict-resolution prompt is constructed
- **THEN** the prompt contains `load skills: team-resolve`
- **AND** conflict resolution markers and parsing behavior remain unchanged

#### Scenario: rejecting and cleanup-review prompts use configured review skills

- **GIVEN** the effective `rejecting_skill` is `team-rejecting`
- **AND** the effective `cleanup_review_skill` is `team-cleanup-review`
- **WHEN** rejecting review and cleanup-review prompts are constructed
- **THEN** those prompts contain `load skills: team-rejecting` and `load skills: team-cleanup-review` respectively
- **AND** their existing marker/parser behavior remains unchanged
