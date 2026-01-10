# Design: TUI Module Architecture

## Context

The current `src/tui.rs` file has grown to 3,812 lines, containing:

| Component | Lines | Responsibility |
|-----------|-------|----------------|
| Types/Enums | ~280 | Data structures for state representation |
| AppState impl | ~460 | State machine and business logic |
| Orchestrator | ~670 | Change processing and archiving logic |
| Rendering | ~490 | UI component rendering |
| Utils | ~40 | Helper functions |
| Tests | ~1,330 | Unit and integration tests |

This monolithic structure violates module-based architecture principles from `openspec/project.md`.

## Goals

- Improve maintainability by separating concerns into focused modules
- Enable parallel development on different TUI aspects
- Make the codebase easier to navigate and understand
- Preserve all existing functionality and tests

## Non-Goals

- Changing any behavioral aspects of the TUI
- Modifying the public API
- Adding new features
- Performance optimization

## Decisions

### Decision: Submodule Structure

Organize TUI code into a `src/tui/` directory with focused submodules.

**Rationale**: Follows Rust conventions for module organization. Each module can be understood independently, and dependencies between modules are explicit.

**Alternatives considered**:
1. Keep single file but add section comments → Rejected: doesn't solve navigation or testability issues
2. Split into separate top-level modules (`src/tui_state.rs`, etc.) → Rejected: pollutes the top-level namespace

### Decision: Module Dependency Graph

```
         ┌─────────────────────────────────────┐
         │              mod.rs                 │
         │         (re-exports API)            │
         └─────────────────┬───────────────────┘
                           │
         ┌─────────────────▼───────────────────┐
         │             runner.rs               │
         │     (run_tui, run_tui_loop)         │
         └─────────┬───────────────────┬───────┘
                   │                   │
         ┌─────────▼───────┐   ┌───────▼───────┐
         │  orchestrator.rs│   │   render.rs   │
         │  (run_orchestr) │   │ (all render_*)│
         └─────────┬───────┘   └───────┬───────┘
                   │                   │
         ┌─────────┴───────────────────┴───────┐
         │              state.rs               │
         │    (AppState, ChangeState impls)    │
         └─────────────────┬───────────────────┘
                           │
         ┌─────────┬───────┴───────┬───────────┐
         │         │               │           │
    ┌────▼────┐┌───▼───┐  ┌───────▼────┐ ┌────▼────┐
    │types.rs ││queue.rs│  │ events.rs  │ │utils.rs │
    │(enums)  ││(Dynamic│  │(Commands,  │ │(helpers)│
    └─────────┘│ Queue) │  │ Events)    │ └─────────┘
               └────────┘  └────────────┘
```

**Rationale**: Clear layered architecture with minimal circular dependencies.

### Decision: Test Organization

Tests will be organized as:
1. Type/unit tests → inline in respective modules (`#[cfg(test)]`)
2. Integration tests for state machine → `src/tui/state.rs`
3. DynamicQueue async tests → `src/tui/queue.rs`

**Rationale**: Keeps tests close to implementation, following Rust conventions.

### Decision: Public API

The `mod.rs` will re-export only what's needed externally:
- `run_tui` - main entry point
- `DynamicQueue` - used by external callers
- `OrchestratorEvent` - for event handling
- `TuiCommand` - for command dispatch
- `get_version_string` - utility used elsewhere

Internal types like `AppState`, `ChangeState`, `render_*` functions remain private to the module tree.

**Rationale**: Minimizes coupling between TUI and other parts of the codebase.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Circular imports | Careful dependency ordering; types.rs has no deps |
| Missing re-exports | Comprehensive `cargo check` after each step |
| Test failures after move | Run tests incrementally during migration |
| Merge conflicts | Complete refactoring in single PR |

## Migration Plan

1. Create new module structure alongside existing `tui.rs`
2. Move code module-by-module, running `cargo check` after each move
3. Update imports incrementally
4. Delete original `tui.rs` only after all code migrated and tests pass
5. No staged rollout needed - pure refactoring

## Open Questions

None - this is a straightforward refactoring with well-established patterns.
