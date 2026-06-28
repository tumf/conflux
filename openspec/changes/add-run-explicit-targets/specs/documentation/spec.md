## ADDED Requirements

### Requirement: Bundled cflx-run skill documents explicit run targets

The bundled `cflx-run` skill documentation SHALL describe `cflx run` as requiring an explicit target mode and SHALL avoid recommending bare `cflx run` for non-interactive orchestration.

<!-- Expected canonical result after archive: documentation requirements will cover bundled `cflx-run` skill docs so operator guidance stays aligned with explicit CLI target behavior. -->

#### Scenario: Skill docs show explicit target examples

- **WHEN** a reader reviews `skills/cflx-run/SKILL.md` or `skills/cflx-run/references/cflx-run.md`
- **THEN** standard execution examples use `cflx run <change-id>...` or `cflx run --all`
- **AND** bare `cflx run` is not presented as a valid default execution command

#### Scenario: Skill docs explain TUI equivalence

- **WHEN** a reader reviews bundled `cflx-run` guidance
- **THEN** positional IDs are described as equivalent to starting with those TUI changes selected
- **AND** `--all` is described as equivalent to TUI bulk mark with `x`

#### Scenario: Skill docs preserve legacy change syntax guidance

- **WHEN** a reader reviews bundled `cflx-run` guidance
- **THEN** `cflx run --change a,b` is documented as legacy-compatible syntax
- **AND** the documentation recommends positional IDs or `--all` for new usage

#### Scenario: Skills README reflects explicit target mode

- **WHEN** a reader reviews `skills/README.md`
- **THEN** the `cflx-run` purpose and installation summary describe explicit target execution
- **AND** the summary does not imply bare `cflx run` processes changes by default
