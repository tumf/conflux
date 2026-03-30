# TUI Module Structure

This directory contains the Terminal User Interface (TUI) for the Conflux orchestrator.

## Module Organization

- **runner.rs**: Main event loop and TUI initialization
- **key_handlers.rs**: Keyboard input handling (Tab, arrow keys, shortcuts, etc.)
- **command_handlers.rs**: TuiCommand processing (queue operations, worktree management, etc.)
- **state.rs**: Application state management
- **render.rs**: UI rendering logic
- **events.rs**: Event types and definitions
- **orchestrator.rs**: Orchestration logic (sequential and parallel modes)
- **queue.rs**: Dynamic queue for runtime change additions
- **types.rs**: Type definitions (AppMode, ViewMode, StopMode, etc.)
- **utils.rs**: Utility functions (editor launch, terminal management, etc.)

## Recent Refactoring (refactor-tui-runner-handlers)

The key event handling and TuiCommand processing logic was extracted from `runner.rs` into separate modules for better maintainability:

### key_handlers.rs (532 lines)
Handles all keyboard input:
- `handle_tab_key()`: Switch between Changes/Worktrees views
- `handle_cursor_movement()`: Navigate with arrows/k/j keys
- `handle_editor_launch()`: Launch editor with 'e' key
- `handle_merge_key()`: Merge operations with 'M' key
- `handle_esc_key()`: Graceful/force stop
- `handle_f5_key()`: Start/resume/retry processing
- `handle_enter_key()`: Execute worktree commands
- `handle_plus_key()`: Create new worktrees with '+' key
- `handle_key_event()`: Main key event dispatcher

### command_handlers.rs (672 lines)
Handles all TuiCommand variants:
- `handle_start_processing_command()`: Spawn orchestrator tasks
- `handle_tui_command()`: Main TuiCommand dispatcher
- Processes commands: AddToQueue, RemoveFromQueue, DeleteWorktree, DeleteWorktreeByPath, Stop, CancelStop, ForceStop, Retry, MergeWorktreeBranch, ResolveMerge

### Integration Status

The helper modules are created and tested but not yet integrated into `run_tui_loop()`.
Integration is deferred to allow for manual TUI behavior testing before making the change.

To integrate the helpers:
1. Replace the large key event match block in `run_tui_loop` with calls to `key_handlers::handle_key_event()`
2. Replace the TuiCommand match block with calls to `command_handlers::handle_tui_command()`
3. Test all keyboard shortcuts and command flows manually in the TUI
4. Verify no regression in user experience

## Testing

Run TUI-specific tests:
```bash
cargo test --bin cflx tui::runner::
```

Verify code quality:
```bash
cargo fmt --check
cargo clippy -- -D warnings
```
