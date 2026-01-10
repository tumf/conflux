# Design: Parallel Change Apply with jj Workspaces

## Context

The current orchestrator processes changes sequentially: analyze → select → apply → archive → repeat. This is safe but slow when multiple independent changes exist. jj (Jujutsu) provides first-class workspace support that enables isolated parallel work on the same repository.

**References:**
- Official jj documentation: https://docs.jj-vcs.dev/latest/
- jj GitHub repository: https://github.com/jj-vcs/jj
- Working copy & workspaces: https://docs.jj-vcs.dev/latest/working-copy/

**Stakeholders:**
- Developers who want faster change processing
- CI/CD pipelines that need efficient batch processing

**Constraints:**
- Must work with existing config-based agent commands
- Must preserve data integrity (no partial states)
- Must support fallback to sequential mode
- **jj is strictly required** - parallel mode is unavailable without `.jj` directory

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

### Decision 3: Automatic conflict resolution with jj

**What:** Always use AI agent to resolve conflicts via jj commands. No configurable strategies - resolution is automatic.

**Why:**
- This is an AI-driven automation tool - manual intervention defeats the purpose
- jj provides excellent conflict resolution tooling (`jj resolve`, conflict markers)
- Simplifies configuration (no `conflict_strategy` or `resolve_command` options)

**Hardcoded Resolution Prompt:**
```
The merge resulted in conflicts. Use jj commands to resolve them.

Conflicted files:
{conflict_files}

Steps:
1. Run `jj status` to see conflict details
2. For each conflicted file, either:
   - Edit the file to resolve conflict markers, OR
   - Run `jj resolve <file>` to use merge tool
3. After resolving all conflicts, run `jj status` to verify
4. Conflicts are resolved when `jj status` shows no conflicts

Important jj conflict commands:
- `jj status` - Show current conflicts
- `jj resolve <file>` - Interactive resolve for a file
- `jj diff` - Show current changes including conflict markers
```

**Fallback:** If AI cannot resolve conflicts after max retries, stop with error and preserve workspace state for manual inspection.

### Decision 4: Event-based progress reporting

**What:** Use `mpsc::channel` to stream parallel execution events to TUI.

**Why:**
- Decouples executor from display logic
- Enables real-time progress updates
- Supports both TUI and headless modes

## Technical Feasibility: Parallelization Analyzer

### Existing Infrastructure (Reusable)

| Component | Status | Location |
|-----------|--------|----------|
| LLM analysis method | ✅ Existing | `src/agent.rs:163-182` `analyze_dependencies()` |
| Prompt building | ✅ Existing | `src/orchestrator.rs:476-506` |
| JSON parsing | ✅ Available | `serde_json` in dependencies |
| Config extension | ✅ Easy | `src/config.rs` JSONC system |
| Placeholder expansion | ✅ Existing | `{prompt}`, `{change_id}` patterns |

### Implementation Approach

New module `src/analyzer.rs`:

```rust
pub struct ParallelizationAnalyzer {
    agent: AgentRunner,
}

impl ParallelizationAnalyzer {
    pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>> {
        // 1. Build prompt (reuse existing pattern)
        let prompt = self.build_parallelization_prompt(changes);

        // 2. Call LLM (reuse existing method)
        let response = self.agent.analyze_dependencies(&prompt).await?;

        // 3. Parse JSON response (serde_json)
        let result: AnalysisResult = serde_json::from_str(&response)?;

        // 4. Validate DAG (detect circular dependencies)
        self.validate_dependency_graph(&result.groups)?;

        // 5. Return groups in topological order
        Ok(self.topological_sort(result.groups))
    }
}
```

### Key Algorithms

**DAG Validation (Circular Dependency Detection):**
- Kahn's algorithm or DFS-based cycle detection
- Standard graph algorithm, ~50 lines of code

**Topological Sort:**
- Order groups by `depends_on` relationships
- Standard algorithm, ~30 lines of code

### Estimated Effort

| Component | Lines of Code |
|-----------|---------------|
| `src/analyzer.rs` | ~200-300 |
| Config extensions | ~50 |
| Orchestrator integration | ~150 |
| Tests | ~400 |
| **Total** | **~700-1000** |

### Fallback Strategy

```
LLM analysis success?
  → Yes: Use parallelization groups
  → No: Fallback to sequential execution (existing behavior)
```

### Risk Assessment

| Risk | Probability | Mitigation |
|------|-------------|------------|
| LLM returns invalid JSON | Medium | Fallback to sequential |
| LLM misidentifies dependencies | Medium | Validation + manual override config |
| Circular dependency in response | Low | DAG validation rejects |
| Performance overhead | Low | Analysis runs once per batch |

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
       - Run `jj status` to detect conflicts
       - If conflicts exist:
         a. Build hardcoded resolution prompt with conflict file list
         b. Execute AI agent (apply_command) to resolve via jj commands
         c. Verify with `jj status`
         d. Retry up to max_retries if still conflicted
         e. If still unresolved: stop with error, preserve workspace

   3.6 Cleanup workspaces
       jj workspace forget ws-{change_id}
       rm -rf /tmp/ws-{change_id}

4. Archive completed changes
5. Report results
```

## jj Commands Reference

Based on official jj documentation (https://docs.jj-vcs.dev/latest/):

```bash
# Check if jj is available and repo is jj-managed
jj root  # Returns repo root path, or error if not jj repo

# Create workspace with name
# Ref: https://docs.jj-vcs.dev/latest/working-copy/#workspaces
jj workspace add /tmp/ws-change1 --name ws-change1

# Get current revision (change ID)
jj log -r @ --no-graph -T change_id

# Merge multiple revisions (creates new commit with multiple parents)
# jj records conflicts as first-class objects - merge won't fail
jj new {rev1} {rev2} {rev3} -m "Merge parallel changes"

# Check for conflicts in working copy
jj status  # Shows "Conflict" markers if present

# Resolve conflicts with external tool
jj resolve  # Uses configured merge tool for 2-way conflicts

# Cleanup workspace
# Ref: "When you're done using a workspace, use jj workspace forget"
jj workspace forget ws-change1
rm -rf /tmp/ws-change1

# List all workspaces
jj workspace list

# Update stale working copy (if modified from another workspace)
jj workspace update-stale
```

### Key jj Concepts for This Design

From https://docs.jj-vcs.dev/latest/working-copy/:

1. **Workspaces**: "You can have multiple working copies backed by a single repo. Use `jj workspace add` to create a new working copy. The working copy will have a `.jj/` directory linked to the main repo."

2. **Automatic commits**: "Unlike most other VCSs, Jujutsu will automatically create commits from the working-copy contents when they have changed."

3. **Conflicts as first-class objects**: "If an operation results in conflicts, information about those conflicts will be recorded in the commit(s). The operation will succeed."

4. **Stale working copy**: "When you modify workspace A's working-copy commit from workspace B, workspace A's working copy will become stale." → Use `jj workspace update-stale` to recover.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| jj not installed | Detect `.jj` directory at startup; CLI exits with error, TUI hides parallel option |
| Disk space exhaustion | Limit max_concurrent_workspaces (default: 3) |
| Merge conflicts | jj records conflicts as first-class objects; configurable strategy with AI resolution option |
| LLM wrong grouping | Manual override via config, validation before execution |
| Workspace cleanup failure | Best-effort cleanup with warning, manual cleanup guide |
| Stale working copy | Use `jj workspace update-stale` if needed after parallel operations |

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
