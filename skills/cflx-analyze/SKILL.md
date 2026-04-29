---
name: cflx-analyze
description: Dependency analysis for Conflux change selection. Evaluates queued changes and selects the next change to process based on dependency order, progress, and completion status. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Dependency Analyzer

Analyze queued OpenSpec changes and select the next change to process based on dependency relationships, progress state, and completion readiness.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

When multiple OpenSpec changes are queued for processing, this skill evaluates them and selects the optimal next change to implement. The selection considers dependency ordering, task completion progress, and whether any changes are already complete and ready for archive.

## Selection Priority

1. **Complete changes first** - Changes with 100% task completion are ready for archive and should be processed immediately.
2. **Dependency-safe changes** - Changes whose dependencies are either absent or already completed.
3. **Highest progress** - Among equally dependency-safe candidates, prefer the change with the most progress (continuity).
4. **Name-inferred dependencies** - Use change IDs to infer likely dependency relationships when explicit dependency metadata is unavailable.

## Selection Rules

- A change that depends on another incomplete change MUST NOT be selected before its dependency.
- If all candidates have unsatisfied dependencies, select the one with the fewest blockers.
- When two candidates are equally viable, prefer the one with higher completion percentage.
- Never skip a complete change in favor of an incomplete one.

## Output Contract

For standard single-change selection usage, output exactly ONE change ID on a single line. No explanation, no formatting, just the raw change ID.

Example:
```
add-feature-x
```

For JSON dependency-analysis usage, `dependencies` must only reference IDs from:
- the current queued change IDs, and
- explicitly supplied in-flight change IDs.

Never return active-but-not-queued IDs, archived IDs, or unrelated IDs as dependency targets.

If you cannot satisfy that contract, fail rather than emitting out-of-scope dependency IDs.

## Context Provided by Orchestrator

The orchestrator injects variable context including:
- List of queued changes with task counts and progress percentages
- Dependency metadata (when available)
- Completion status for each change

## Autonomous Decision Framework

When facing ambiguous dependency relationships:

1. **Explicit dependencies** - Honor declared dependencies in proposal metadata
2. **Name inference** - Use change ID naming patterns to infer relationships (e.g., `add-auth` likely precedes `add-auth-tests`)
3. **Progress signal** - Higher progress suggests the change is closer to completion and should be continued
4. **Simplicity** - When truly ambiguous, pick the change with fewer total tasks

**Never**:
- Ask user for clarification
- Stop and wait for input
- Output more than one change ID
- Output explanatory text with the selection
