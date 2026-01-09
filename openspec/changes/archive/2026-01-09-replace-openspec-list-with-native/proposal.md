# Proposal: Replace openspec list with Native Implementation

## Summary

Replace all `openspec list --json` command invocations with a native Rust implementation that directly reads the `openspec/changes` directory and parses task progress from `tasks.md` files.

## Problem

1. **External command dependency**: TUI mode calls `openspec list --json` multiple times per iteration, adding process spawn overhead
2. **Inconsistent task counts**: The external CLI may report incorrect task counts (e.g., `0/0`) for certain task list formats
3. **Reliability issues**: Discrepancy between CLI output and actual file state can cause premature completion detection

## Solution

Implement `list_changes_native()` function in `openspec.rs` that:
1. Reads `openspec/changes` directory entries
2. For each subdirectory, parses `tasks.md` using existing `task_parser` module
3. Returns `Vec<Change>` with accurate task progress

Replace all `openspec::list_changes()` calls with the native implementation.

## Affected Files

- `src/openspec.rs` - Add `list_changes_native()` function
- `src/tui.rs` - Replace `openspec::list_changes()` calls (4 locations)
- `src/main.rs` - Replace initial change list fetch (2 locations)

## Out of Scope

- Non-interactive `run` mode (uses different orchestrator logic)
- Archive/apply commands (still use external commands)
