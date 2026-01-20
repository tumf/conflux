# ~/.wt Directory - Git Worktree Management for AI Coding

## Overview

The `~/.wt` directory is the centralized storage location for all Git worktrees managed by the `wt` tool. This directory enables parallel AI coding sessions by maintaining isolated development environments for each task.

## Purpose

This directory supports **Vibe Coding** workflows where multiple AI coding agents work in parallel on different tasks without interfering with each other. Each worktree has:

- Independent working directory
- Separate dependency installations (node_modules, venv, etc.)
- Isolated git branches
- Automated environment setup

## Directory Structure

```
~/.wt/
├── setup                 # Global setup script (executed for all worktrees)
└── worktrees/           # Storage for all project worktrees
    └── <project-name>/
        └── <worktree-name>/
```

### Global Setup Script

**Location**: `~/.wt/setup`

This script is executed automatically when creating a new worktree with `wt add`. It typically contains:

- Environment initialization common to all projects
- Global dependency checks
- Cross-project configurations

**Available Variables**:
- `$ROOT_WORKTREE_PATH` - Path to the base repository (main worktree)

**Current Configuration**:
```bash
#!/bin/bash
# Executes project-local setup if exists
if [ -f .wt/setup.local ]; then
  source .wt/setup.local
fi

# Pre-build Rust tests (example for Rust projects)
cargo test --no-run
```

### Worktrees Directory

Each project creates a subdirectory under `worktrees/` containing all its git worktrees. These are the actual working directories where AI coding agents operate.

## Integration with Projects

Projects using `wt` create symbolic links to their worktrees:

```
<project-root>/
├── .wt/
│   ├── setup.local      # Project-specific setup script
│   └── worktrees/       # Symlink to ~/.wt/worktrees/<project>/
```

This allows developers to access worktrees via either:
- Project-local path: `.wt/worktrees/<worktree-name>/`
- Global path: `~/.wt/worktrees/<project>/<worktree-name>/`

## Workflow for AI Coding Agents

### Creating a Worktree

```bash
wt add <worktree-name>
```

This will:
1. Create a new branch
2. Initialize worktree in `~/.wt/worktrees/<project>/<worktree-name>/`
3. Execute global setup (`~/.wt/setup`)
4. Execute project setup (`.wt/setup.local` if exists)

### Running Commands in Worktrees

```bash
# Change to worktree directory
cd .wt/worktrees/<worktree-name>/

# Or use wt run
wt run <worktree-name> <command>
```

### Listing Worktrees

```bash
wt list
```

### Removing Worktrees

```bash
wt remove <worktree-name>
```

## Best Practices

### 1. Task Isolation

Each worktree should correspond to a single, well-defined task:
- ✅ `feature-login-form`
- ✅ `hotfix-memory-leak`
- ✅ `experiment-graphql`
- ❌ `feature-user-module` (too broad)

### 2. Setup Script Guidelines

Keep setup scripts **idempotent** (safe to run multiple times):

```bash
# Good: Check before installing
if [ -f package.json ] && [ ! -d node_modules ]; then
  npm install
fi

# Bad: Always installs (wastes time)
npm install
```

### 3. Parallel AI Sessions

Use multiple worktrees for parallel development:

```bash
# Terminal 1: Main feature
wt add feature-auth
cd .wt/worktrees/feature-auth
opencode .

# Terminal 2: Urgent bugfix (parallel)
wt add hotfix-login-error
cd .wt/worktrees/hotfix-login-error
opencode .
```

### 4. Resource Management

Regularly clean up completed worktrees to save disk space:

```bash
# Review active worktrees
wt list

# Remove merged/completed worktrees
wt remove old-feature-name
```

## Common Patterns

### Experimental Comparisons

Test different approaches in parallel:

```bash
wt add experiment-rest-api
wt add experiment-graphql
```

Give each AI agent different instructions and compare results.

### PR Review

Review pull requests in isolated environments:

```bash
git fetch origin pull/123/head:pr-123
wt add pr-123
cd .wt/worktrees/pr-123
# Review and test without affecting main work
```

### tmux Integration

Manage multiple worktrees with tmux windows:

```bash
wt run feature-a -- bash -c 'tmux new-window -n feature-a -c "$(pwd)" \; send-keys "opencode ." Enter'
```

## Troubleshooting

### Issue: Disk Space

**Symptom**: Running out of disk space with many worktrees.

**Solution**: Each worktree has independent dependencies. Clean up unused worktrees regularly.

```bash
wt list
wt remove <unused-worktree>
```

### Issue: Merge Conflicts

**Symptom**: Multiple worktrees editing the same files cause conflicts.

**Solution**: Design tasks to minimize file overlap. Use dependency analysis to identify potential conflicts early.

### Issue: Symlink Problems

**Symptom**: Some tools don't follow symlinks correctly.

**Solution**: Use the real path instead of the symlink:

```bash
# Instead of: cd .wt/worktrees/feature-a
cd ~/.wt/worktrees/<project>/feature-a
```

## Reference

- **wt tool**: https://github.com/tumf/wt
- **Blog post**: https://blog.tumf.dev/posts/diary/2025/10/29/wt-vibe-coding/
- **Git worktree docs**: https://git-scm.com/docs/git-worktree

## Environment Variables

The following variables are available in setup scripts:

| Variable | Description | Example |
|----------|-------------|---------|
| `ROOT_WORKTREE_PATH` | Path to base repository | `/Users/alice/projects/myapp` |

## Notes for AI Agents

When working in a worktree environment:

1. **Always check current directory** - Verify you're in the correct worktree
2. **Don't modify other worktrees** - Each AI session should stay in its assigned worktree
3. **Respect setup scripts** - Let `wt` handle environment initialization
4. **Report completion** - Notify when work is done so worktree can be cleaned up
5. **Document dependencies** - Update setup scripts if new dependencies are needed
