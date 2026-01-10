# Remove OpenSpec CLI Dependency

## Summary

Remove the dependency on external `openspec` CLI command for listing changes during the run loop. The orchestrator should use the existing native implementation (`list_changes_native()`) instead of executing `npx @fission-ai/openspec@latest list --json`.

## Problem

Currently, `orchestrator.rs` calls `openspec::list_changes(&self.openspec_cmd)` at lines 84 and 118, which executes an external command:

```
npx @fission-ai/openspec@latest list --json
```

This:
1. Adds unnecessary latency (npm/npx startup time)
2. Requires network access potentially
3. Is redundant since native implementation already exists in `list_changes_native()`

## Solution

Replace `list_changes()` calls in `orchestrator.rs` with `list_changes_native()`, which directly reads the `openspec/changes` directory and parses `tasks.md` files.

## Scope

- `src/orchestrator.rs`: Replace `list_changes()` calls with `list_changes_native()`
- `src/openspec.rs`: Remove or deprecate `list_changes()` function
- Remove `openspec_cmd` field from `Orchestrator` struct (no longer needed)
- Update CLI arguments if `--openspec-cmd` is exposed

## Impact

- **Performance**: Faster startup and iteration cycles
- **Reliability**: No external command dependencies during run
- **Simplicity**: Reduced configuration surface area
