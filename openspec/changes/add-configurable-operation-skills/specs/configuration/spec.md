## ADDED Requirements

### Requirement: Configurable operation skills

The orchestrator MUST support optional top-level configuration values that select the skill prelude for each supported orchestrator operation:

- `analyze_skill`, defaulting to `cflx-analyze`
- `apply_skill`, defaulting to `cflx-apply`
- `rejecting_skill`, defaulting to `cflx-rejecting`
- `cleanup_review_skill`, defaulting to `cflx-cleanup-review`
- `accept_skill`, defaulting to `cflx-accept`
- `archive_skill`, defaulting to `cflx-archive`
- `resolve_skill`, defaulting to `cflx-resolve`

Each operation skill key MUST participate in the same configuration merge precedence as other top-level optional config fields: custom config overrides project config, project config overrides global config, and lower-precedence values are retained when higher-precedence configs omit the key.

The configured values MUST affect only generated prompt `load skills: ...` preludes. They MUST NOT change command execution, verdict parsing, archive routing, dependency selection semantics, conflict marker parsing, rejection review markers, or workflow-control state by themselves.

#### Scenario: omitted operation skill config uses standard skills

- **GIVEN** the merged configuration does not contain operation skill keys
- **WHEN** orchestrator operation prompts are constructed
- **THEN** their prompt preludes use `cflx-analyze`, `cflx-apply`, `cflx-rejecting`, `cflx-cleanup-review`, `cflx-accept`, `cflx-archive`, and `cflx-resolve` respectively
- **AND** existing command execution and parser behavior remain unchanged

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
- **AND** acceptance command execution and verdict parsing behave as before

#### Scenario: project config selects custom resolve skill

- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "resolve_skill": "team-resolve"
  }
  ```
- **WHEN** the configuration is loaded and a conflict-resolution prompt is constructed
- **THEN** the prompt contains `load skills: team-resolve`
- **AND** conflict resolution output markers and parsing behavior remain unchanged

#### Scenario: operation skill config follows config precedence

- **GIVEN** a global config contains `"accept_skill": "cflx-accept"` and `"resolve_skill": "cflx-resolve"`
- **AND** the project config contains `"accept_skill": "cflx-accept-with-speca"` and `"resolve_skill": "team-resolve"`
- **WHEN** configuration is loaded
- **THEN** the effective acceptance skill is `cflx-accept-with-speca`
- **AND** the effective resolve skill is `team-resolve`

#### Scenario: custom config overrides project operation skill config

- **GIVEN** a project config contains `"accept_skill": "cflx-accept-with-speca"`
- **AND** a custom config specified with `--config` contains `"accept_skill": "cflx-accept"`
- **WHEN** configuration is loaded
- **THEN** the effective acceptance skill is `cflx-accept`
