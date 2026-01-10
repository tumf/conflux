# Proposal: Add Max Iterations Configuration

## Summary

Add a configurable maximum iteration limit for the apply loop to prevent infinite loops and provide predictable execution bounds.

## Problem

Currently, the orchestration loop runs indefinitely until all changes are complete or an error occurs. This can lead to:
- Infinite loops if a change never completes
- Unpredictable resource consumption
- No upper bound on execution time

## Solution

Add a `max_iterations` configuration option that limits the number of apply loop iterations. When the limit is reached, the orchestrator stops gracefully with a clear status.

## Key Features

1. **Configuration Option**: `max_iterations` in `.openspec-orchestrator.jsonc`
2. **Default Value**: 50 iterations
3. **Graceful Stop**: When limit reached, finish with `iteration_limit` status
4. **Hook Support**: `on_finish` hook receives `iteration_limit` status
5. **CLI Override**: Optional `--max-iterations` flag for one-time override

## Out of Scope

- Per-change iteration limits
- Automatic retry with backoff
- Dynamic limit adjustment

## Impact

- Configuration: New `max_iterations` field
- Orchestrator: Check iteration count before each loop
- Hooks: New finish status value `iteration_limit`
