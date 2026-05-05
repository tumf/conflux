## ADDED Requirements

### Requirement: Configurable acceptance skill

The orchestrator MUST support an optional top-level `accept_skill` configuration value that selects the skill prelude used in acceptance prompts.

When `accept_skill` is omitted, the orchestrator MUST behave as if `accept_skill` were `cflx-accept`.

`accept_skill` MUST participate in the same configuration merge precedence as other top-level optional config fields: custom config overrides project config, project config overrides global config, and lower-precedence values are retained when higher-precedence configs omit the key.

The configured value MUST affect only the generated acceptance prompt's `load skills: ...` prelude. It MUST NOT change acceptance command execution, verdict parsing, archive routing, or workflow-control state by itself.

#### Scenario: omitted accept_skill uses the standard acceptance skill

- **GIVEN** the merged configuration does not contain `accept_skill`
- **WHEN** an acceptance prompt is constructed
- **THEN** the prompt contains `load skills: cflx-accept`
- **AND** acceptance command execution and verdict parsing behave as before

#### Scenario: project config selects SPECA acceptance skill

- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "accept_skill": "cflx-accept-with-speca"
  }
  ```
- **WHEN** the configuration is loaded and an acceptance prompt is constructed
- **THEN** the prompt contains `load skills: cflx-accept-with-speca`
- **AND** the prompt does not contain the default `load skills: cflx-accept` prelude as the selected acceptance skill

#### Scenario: accept_skill follows config precedence

- **GIVEN** a global config contains `"accept_skill": "cflx-accept"`
- **AND** the project config contains `"accept_skill": "cflx-accept-with-speca"`
- **WHEN** configuration is loaded
- **THEN** the effective acceptance skill is `cflx-accept-with-speca`

#### Scenario: custom config overrides project accept_skill

- **GIVEN** a project config contains `"accept_skill": "cflx-accept-with-speca"`
- **AND** a custom config specified with `--config` contains `"accept_skill": "cflx-accept"`
- **WHEN** configuration is loaded
- **THEN** the effective acceptance skill is `cflx-accept`
