# Design: Parallel Change Apply with jj Workspaces

## Context

The current orchestrator processes changes sequentially: analyze → select → apply → archive → repeat. This is safe but slow when multiple independent changes exist. jj (Jujutsu) provides first-class workspace support that enables isolated parallel work on the same repository.

**Stakeholders:**
- Developers who want faster change processing
- CI/CD pipelines that need efficient batch processing

**Constraints:**
- Must work with existing config-based agent commands
- Must preserve data integrity (no partial states)
- Must support fallback to sequential mode

## Goals / Non-Goals

**Goals:**
- Enable parallel execution of independent changes via jj workspaces
- Provide configurable conflict resolution strategies
- Maintain backward compatibility with sequential mode
- Integrate with existing TUI for progress visualization

**Non-Goals:**
- Git worktree support (jj-only for initial implementation)
- Automatic parallelization detection without LLM (complex dependency analysis)
- Cross-repository operations

## Decisions

### Decision 1: Use jj workspaces for isolation

**What:** Create temporary jj workspaces for each parallel change.

**Why:**
- jj workspaces share the same repository storage (efficient)
- Each workspace has independent working copy
- Native merge support with `jj new <rev1> <rev2> ...`
- Clean workspace cleanup with `jj workspace forget`

**Alternatives considered:**
- Git worktrees: Less flexible merge semantics, more disk usage
- Docker containers: Overkill, complex setup, slower
- Branch-only: No working copy isolation, conflict-prone

### Decision 2: LLM-based parallelization analysis

**What:** Extend the existing analyze_command to identify parallel execution groups.

**Why:**
- Reuses existing LLM integration infrastructure
- Can infer dependencies from change names and context
- Flexible to project-specific patterns

**Format:** LLM returns JSON with groups:
```json
{
  "groups": [
    {"id": 1, "changes": ["add-feature-a", "add-feature-b"], "depends_on": []},
    {"id": 2, "changes": ["update-docs"], "depends_on": [1]}
  ]
}
```

### Decision 3: Configurable conflict resolution

**What:** Support multiple strategies via `conflict_strategy` config:
- `fail` (default): Stop on conflict, preserve workspace state
- `skip`: Skip conflicting change, continue with others
- `resolve`: Use `resolve_command` (AI agent) to resolve conflicts

**Why:**
- Different projects have different tolerance for automation
- AI-assisted resolution leverages existing agent infrastructure
- `fail` is safe default for unknown situations

### Decision 4: Event-based progress reporting

**What:** Use `mpsc::channel` to stream parallel execution events to TUI.

**Why:**
- Decouples executor from display logic
- Enables real-time progress updates
- Supports both TUI and headless modes

## Execution Flow

```
1. List changes from openspec
2. Analyze for parallelization (LLM)
   → Returns: [{group_id, change_ids, depends_on}, ...]

3. For each group in topological order:
   3.1 Create jj workspaces (parallel)
       jj workspace add /tmp/ws-{change_id} -r @

   3.2 Execute apply in each workspace (parallel, max_concurrent limit)
       cd /tmp/ws-{change_id} && {apply_command}

   3.3 Collect results (success/failure per workspace)

   3.4 Merge successful results
       jj new {rev1} {rev2} ... -m "Merge parallel changes"

   3.5 Handle conflicts (if any)
       - fail: Stop and report
       - skip: Continue without conflicting changes
       - resolve: Run resolve_command

   3.6 Cleanup workspaces
       jj workspace forget ws-{change_id}
       rm -rf /tmp/ws-{change_id}

4. Archive completed changes
5. Report results
```

## jj Commands Reference

```bash
# Create workspace
jj workspace add /tmp/ws-change1 -r @

# Get current revision
jj log -r @ --no-graph -T change_id

# Merge multiple revisions
jj new {rev1} {rev2} {rev3} -m "Merge parallel changes"

# Check for conflicts
jj status  # Look for "Conflict" markers

# Cleanup
jj workspace forget ws-change1
rm -rf /tmp/ws-change1

# List workspaces
jj workspace list
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| jj not installed | Detect at startup, fallback to sequential mode |
| Disk space exhaustion | Limit max_concurrent_workspaces (default: 3) |
| Merge conflicts | Configurable strategy, AI-assisted resolution option |
| LLM wrong grouping | Manual override via config, validation before execution |
| Workspace cleanup failure | Best-effort cleanup with warning, manual cleanup guide |

## Migration Plan

1. **Phase 1:** Add behind `--parallel` flag (opt-in)
2. **Phase 2:** Stabilize and document
3. **Phase 3:** Consider auto-enable when jj detected (future)

**Rollback:** Sequential mode always available, parallel mode is purely additive.

## Open Questions

1. Should we support a `--dry-run` mode to preview parallelization groups?
   → **Proposed:** Yes, useful for validation

2. Should workspace base directory be configurable?
   → **Proposed:** Yes, default to system temp

3. How to handle nested/recursive changes that create new changes during apply?
   → **Proposed:** Ignore new changes during parallel batch, process in next iteration
