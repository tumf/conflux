# .wt Directory - Worktree Setup (wt + cflx)

## Overview

This repository includes a `.wt/` directory for worktree-related scripts and conventions.

- The **wt** tool may create worktrees under `~/.wt/worktrees/` and place symlinks in `.wt/worktrees/`.
- Conflux (**cflx**) may create git worktrees and (optionally) execute `.wt/setup` from the repository root.

Important: This repository does not use a user-global setup script.

## Directory Structure

Typical layout (some entries are created locally and are gitignored):

```text
.wt/
  AGENTS.md          # This file
  setup              # Optional setup script executed in new worktrees
  setup.local        # Local overrides (gitignored)
  worktrees/         # Symlinks to actual worktrees (gitignored, created by wt)
```

Actual worktrees (wt default):

```text
~/.wt/worktrees/
  <project>-<name>/
```

## Setup Script

### `.wt/setup`

If `.wt/setup` exists, it MAY be executed after creating a new worktree.

- Execution context: the new worktree directory (current working directory is the worktree)
- Environment variables:
  - `ROOT_WORKTREE_PATH`: path to the base repository (source tree)

The script SHOULD be idempotent (safe to run multiple times) and SHOULD avoid slow operations.

### `.wt/setup.local`

Optional local-only overrides.

- This file SHOULD be gitignored.
- If used, `.wt/setup` can source it to apply developer-specific behavior.

## cflx Behavior

When Conflux creates a git worktree, it MAY execute the repository-local `.wt/setup` script.

- cflx MUST NOT read or execute any user-global setup script.
- cflx MUST only consider `.wt/setup` located in the repository root.

When Conflux deletes a managed git worktree, it MAY execute a worktree-local `.wt/teardown` script before deletion.

- Detection path: `<worktree>/.wt/teardown`
- Execution context: current working directory is the target worktree root
- Environment variables:
  - `ROOT_WORKTREE_PATH`: path to the base repository (source tree)
- stdin: null (`/dev/null`)
- Executable requirement: non-executable `teardown` files are skipped
- Default behavior: teardown failure aborts worktree deletion to preserve recovery state
- Recovery behavior: explicit skip-teardown deletion is available for manual/operator recovery
- Convention: teams may source `.wt/state.env` from `.wt/teardown` for worktree-local cleanup state

## References

- wt tool: https://github.com/tumf/wt
- Git worktree docs: https://git-scm.com/docs/git-worktree
