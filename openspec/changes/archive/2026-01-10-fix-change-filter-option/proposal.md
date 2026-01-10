# Proposal: Fix --change Option Filtering

## Problem Statement

When using `run --change a`, the orchestrator incorrectly reports all available changes in the snapshot log:

```
Captured snapshot of 2 changes: {"add-jj-parallel-apply", "add-log-scroll"}
```

This is confusing because the user specified `--change a`, but the log shows unrelated changes.

### Current Behavior Issues

1. **Snapshot includes all changes**: The initial snapshot captures ALL changes, not just the ones specified by `--change`
2. **No comma-separated support**: `--change a,b,c` is not supported for selecting multiple changes
3. **No warning for missing changes**: If `--change b` is specified but `b` doesn't exist, no warning is shown

### Expected Behavior

1. When `--change a` is specified, only `a` should be in the snapshot
2. When `--change a,b,c` is specified, `a`, `b`, and `c` should be selected
3. If `b` doesn't exist, show a warning but continue with `a` and `c`
4. The log should only show: `Captured snapshot of 2 changes: {"a", "c"}`

## Solution Overview

1. Parse `--change` value as comma-separated list
2. Filter the initial snapshot to only include specified changes
3. Warn about any specified changes that don't exist
4. Continue processing with the valid subset

## Scope

- **Files affected**: `src/cli.rs`, `src/orchestrator.rs`, `src/main.rs`
- **Breaking changes**: None (comma-separated is additive)

## Success Criteria

- `run --change a` → snapshot only includes `a`
- `run --change a,b,c` → snapshot includes `a`, `b`, `c` (if they exist)
- `run --change a,nonexistent,c` → warning for `nonexistent`, snapshot includes `a` and `c`
- Existing behavior without `--change` remains unchanged
