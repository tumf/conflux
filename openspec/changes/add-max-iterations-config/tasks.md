# Tasks: Add Max Iterations Configuration

## Implementation Tasks

- [ ] Add `max_iterations` field to `OrchestratorConfig` in `src/config.rs`
  - [ ] Add field with `Option<u32>` type
  - [ ] Add getter method `get_max_iterations()`
  - [ ] Default to `50`

- [ ] Update orchestrator loop in `src/orchestrator.rs`
  - [ ] Check iteration count at start of each loop
  - [ ] Stop with `iteration_limit` status when limit reached
  - [ ] Log warning when approaching limit (80%)

- [ ] Update TUI orchestrator loop in `src/tui.rs`
  - [ ] Apply same iteration limit check
  - [ ] Display limit status in UI

- [ ] Update templates in `src/templates.rs`
  - [ ] Add `max_iterations` example (commented out) to templates

- [ ] Add CLI override option
  - [ ] Add `--max-iterations` flag to `run` subcommand in `src/cli.rs`
  - [ ] CLI flag overrides config file value

- [ ] Add unit tests
  - [ ] Test iteration limit stops loop
  - [ ] Test no limit when not configured
  - [ ] Test CLI override

- [ ] Update documentation
  - [ ] Add to config spec
