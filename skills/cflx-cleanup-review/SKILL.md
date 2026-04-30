---
name: cflx-cleanup-review
description: Post-apply cleanup handoff for dirty managed worktrees. Ensures worktree is clean before acceptance starts. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Cleanup Review

Ensure a task-complete but dirty managed worktree is made handoff-ready before acceptance starts.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

After an apply operation completes all tasks, the managed worktree may have uncommitted changes (e.g., auto-generated files, formatting fixes, leftover build artifacts). This skill reviews and cleans only the apply-generated dirty state to prepare for acceptance.

## Required Behavior

- If `openspec/CONSTITUTION.md` exists, read it before cleanup review and treat it as higher-priority project law than proposal/spec deltas.

1. Run inside the managed worktree for the given change.
2. Review dirty files and clean only post-apply handoff artifacts.
3. **NEVER** use blind staging such as `git add -A` or `git add .`.
4. Stage/commit only intentional cleanup changes required for clean handoff.
5. Verify worktree cleanliness before finishing.
6. Output exactly one success marker line on success:
   - `CLEANUP_REVIEW: CLEAN`

## Cleanup Guidelines

### Safe to stage and commit:
- Formatting changes from `cargo fmt` or similar auto-formatters
- Updated lock files (e.g., `Cargo.lock`) that result from dependency changes
- Generated files that are tracked in git and were modified by the build process

### Do NOT stage:
- New untracked files that were not part of the change
- Build artifacts that should be in `.gitignore`
- Temporary or debug files
- Files unrelated to the change being processed

### Process:
1. Run `git status --porcelain` to enumerate dirty files
2. For each dirty file, determine if it is an expected apply artifact
3. Stage only expected artifacts with explicit `git add <file>` commands
4. Commit with a descriptive message
5. Verify `git status --porcelain` outputs empty after cleanup

## Output Rules

- Success output MUST contain exactly one standalone marker line: `CLEANUP_REVIEW: CLEAN`
- Do not emit alternate verdict markers.
- Do not wrap the marker in code fences.
- If cleanup cannot be completed, fail loudly (non-zero/command failure) instead of inventing a different marker.

## Output Contract

On success, output exactly:
```
CLEANUP_REVIEW: CLEAN
```

**Never**:
- Ask user for clarification
- Stop and wait for input
- Use `git add -A` or `git add .`
- Output acceptance-related or rejection-related markers
