## MODIFIED Requirements
### Requirement: TUI Module Structure

The TUI module SHALL be organized as a directory-based module tree under `src/tui/` with focused submodules. The TUI state layer MUST consume a shared orchestration state model for change progress and execution metadata, while UI-only fields (cursor, view modes, selection state) remain in TUI-owned state. The iteration number imported from the shared orchestration state MUST NOT overwrite the TUI with a smaller value than what is already displayed. It MUST retain a larger value as needed so the displayed iteration does not regress.

#### Scenario: Module directory exists
- **WHEN** the project is compiled
- **THEN** `src/tui/mod.rs` exists as the module entry point
- **AND** submodules are organized in `src/tui/*.rs` files

#### Scenario: Each submodule has single responsibility
- **GIVEN** the TUI module structure
- **THEN** `types.rs` contains only enum and type definitions
- **AND** `state.rs` contains only state management logic
- **AND** `events.rs` contains only event and command definitions
- **AND** `render.rs` contains only rendering functions
- **AND** `orchestrator.rs` contains only orchestration logic
- **AND** `runner.rs` contains only the main TUI loop
- **AND** `queue.rs` contains only DynamicQueue implementation
- **AND** `utils.rs` contains only utility functions
- **AND** `terminal.rs` contains only terminal execution helpers
- **AND** `worktrees.rs` contains only worktree-related helpers

#### Scenario: Change progress uses shared state
- **GIVEN** the TUI state layer builds the change list for rendering
- **WHEN** change progress and execution metadata are required
- **THEN** the data source is the shared orchestration state
- **AND** UI-specific fields remain in TUI-owned state

#### Scenario: Iteration number does not regress during refresh
- **GIVEN** the TUI already displays `iteration_number=4` for a change
- **AND** the shared orchestration state reports `apply_count=3`
- **WHEN** the automatic refresh merges shared state into the TUI change list
- **THEN** the TUI keeps `iteration_number=4`
