## MODIFIED Requirements
### Requirement: TUI Module Structure

The TUI module SHALL be organized as a directory-based module tree under `src/tui/` with focused submodules. The TUI state layer MUST consume a shared orchestration state model for change progress and execution metadata, while UI-only fields (cursor, view modes, selection state) remain in TUI-owned state.

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

#### Scenario: Change progress uses shared state
- **GIVEN** the TUI state layer builds the change list for rendering
- **WHEN** change progress and execution metadata are required
- **THEN** the data source is the shared orchestration state
- **AND** UI-specific fields remain in TUI-owned state
