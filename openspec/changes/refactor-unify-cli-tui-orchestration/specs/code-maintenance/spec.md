## ADDED Requirements

### Requirement: Unified Orchestration Module

The codebase SHALL have a unified orchestration module that contains shared logic between CLI and TUI modes.

#### Scenario: Archive logic is shared
- **WHEN** a change is archived in CLI mode
- **AND** when a change is archived in TUI mode
- **THEN** both modes SHALL use the same `orchestration::archive_change()` function
- **AND** the archive path validation SHALL use `openspec/changes/archive/`

#### Scenario: Apply logic is shared
- **WHEN** a change is applied in CLI mode
- **AND** when a change is applied in TUI mode
- **THEN** both modes SHALL use the same `orchestration::apply_change()` function
- **AND** hook invocations (pre_apply, post_apply, on_error) SHALL be consistent

#### Scenario: State management is shared
- **WHEN** orchestration state is tracked in CLI mode
- **AND** when orchestration state is tracked in TUI mode
- **THEN** both modes SHALL use the same `OrchestratorState` structure
- **AND** variable naming SHALL be consistent (pending_changes, completed_changes, apply_counts)

### Requirement: OutputHandler Abstraction

The orchestration module SHALL provide an OutputHandler trait for mode-specific output handling.

#### Scenario: CLI uses logging output
- **WHEN** CLI mode executes orchestration
- **THEN** output SHALL be written to the tracing log
- **AND** no channel communication is required

#### Scenario: TUI uses channel output
- **WHEN** TUI mode executes orchestration
- **THEN** output SHALL be sent through mpsc channels
- **AND** output SHALL be displayed in the TUI log panel

### Requirement: Hook Context Helpers

The orchestration module SHALL provide helper functions for building HookContext instances.

#### Scenario: Archive hook context
- **WHEN** archive operation needs hook context
- **THEN** `build_archive_context()` helper SHALL be used
- **AND** the helper SHALL set all required fields consistently

#### Scenario: Apply hook context
- **WHEN** apply operation needs hook context
- **THEN** `build_apply_context()` helper SHALL be used
- **AND** the helper SHALL set all required fields consistently
