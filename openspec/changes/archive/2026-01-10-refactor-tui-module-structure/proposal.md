# Change: Refactor tui.rs into Modular Architecture

## Why

`src/tui.rs` has grown to 3,812 lines and 136KB, making it difficult to maintain, navigate, and test. The file contains multiple distinct responsibilities mixed together:

- Data structures and types (~280 lines)
- State management (~460 lines)
- Orchestrator execution logic (~670 lines)
- UI rendering (~490 lines)
- Utility functions (~40 lines)
- Tests (~1,330 lines)

This violates the Single Responsibility Principle and makes the codebase harder to understand, modify, and extend.

## What Changes

- Split `tui.rs` into focused submodules under `src/tui/`
- Extract orchestrator execution logic into a separate module
- Separate rendering logic into its own module
- Move types and state management into dedicated modules
- Reorganize tests to match new module structure
- **No behavioral changes** - pure refactoring

## Module Structure

```
src/
├── tui.rs              → src/tui/mod.rs (re-exports)
└── tui/
    ├── mod.rs          # Public API re-exports
    ├── types.rs        # Enums, structs (StopMode, AppMode, QueueStatus, etc.)
    ├── state.rs        # AppState and ChangeState implementations
    ├── queue.rs        # DynamicQueue implementation
    ├── events.rs       # TuiCommand, OrchestratorEvent, LogEntry
    ├── runner.rs       # run_tui, run_tui_loop
    ├── orchestrator.rs # run_orchestrator, archive functions
    ├── render.rs       # All render_* functions
    └── utils.rs        # truncate_to_display_width, clear_screen, launch_editor_for_change
```

## Impact

- Affected specs: None (internal refactoring only)
- Affected code: `src/tui.rs` → split into `src/tui/` module tree
- Dependencies: No external dependency changes
- API: Public exports remain identical
