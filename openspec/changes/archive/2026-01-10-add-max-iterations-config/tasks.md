# Tasks: Add Max Iterations Configuration

## Implementation Tasks

- [x] Add `max_iterations` field to `OrchestratorConfig` in `src/config.rs`
  - [x] Add field with `Option<u32>` type
  - [x] Add getter method `get_max_iterations()`
  - [x] Default to `50`

- [x] Update orchestrator loop in `src/orchestrator.rs`
  - [x] Check iteration count at start of each loop
  - [x] Stop with `iteration_limit` status when limit reached
  - [x] Log warning when approaching limit (80%)

- [x] Update TUI orchestrator loop in `src/tui.rs`
  - [x] Apply same iteration limit check
  - [x] Display limit status in UI

- [x] Update templates in `src/templates.rs`
  - [x] Add `max_iterations` example (commented out) to templates

- [x] Add CLI override option
  - [x] Add `--max-iterations` flag to `run` subcommand in `src/cli.rs`
  - [x] CLI flag overrides config file value

- [x] Add unit tests
  - [x] Test iteration limit stops loop
  - [x] Test no limit when not configured
  - [x] Test CLI override
