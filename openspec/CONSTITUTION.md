# Conflux Constitution

This document defines project-level laws that are higher priority than proposal deltas, canonical specs, and implementation details.

If any proposal, spec delta, or implementation conflicts with this constitution, the constitution wins unless it is explicitly amended first.

## Constitutional Laws

### 1. Workspace-local workflow state

Workflow state MUST be derivable from the workspace alone.

Authoritative workflow-control inputs are limited to:

- the workspace file state
- the workspace git state
- base-branch tree comparison

The system MUST NOT introduce or depend on out-of-worktree durable workflow state for resume routing, acceptance gating, archive routing, or other next-action decisions.

Deleting `~/.local/state/cflx/**` MUST NOT change the next action chosen for the same workspace contents.

Ephemeral in-memory state held within a single process lifetime is not durable workflow state and is permitted, provided it is discarded on restart and the next action is recomputed from the workspace alone.

External logs, metrics, caches, and UI state are allowed only as non-authoritative observability outputs and MUST NOT be used as workflow control inputs.

### 2. Constitution precedence

Proposal authors, reviewers, and implementers MUST read and follow this constitution when it exists.

A proposal that intentionally changes constitutional behavior MUST amend this document explicitly as part of the same change.

### 3. Truthful completion

Tasks, acceptance, and archive decisions MUST be based on repository-verifiable evidence.

No change may be treated as implemented, accepted, or archive-ready based only on narrative claims, checklist normalization, or hidden runtime state.

### 4. Target worktree non-pollution

Conflux MUST NOT create, modify, or retain files or directories in a target repository or managed worktree solely for Conflux's own runtime, orchestration, caching, logging, evidence, checkpoint, coordination, or observability needs.

The target worktree may change only when that change is an intended repository deliverable required by the active change contract. Conflux-owned operational artifacts MUST live outside the target repository and all of its worktrees.

Hiding a Conflux-owned artifact with `.gitignore`, `.git/info/exclude`, an embedded ignore file, status filtering, or cleanliness-check exceptions does not make the write permissible and MUST NOT be treated as non-polluting.

Conflux MUST evaluate target-worktree cleanliness without excluding Conflux-owned paths. If an operation would require a Conflux-owned write inside the target worktree, it MUST fail closed before writing and report the conflicting requirement.

Deleting all out-of-worktree Conflux state MAY discard caches, logs, and reusable evidence, but MUST NOT modify the target worktree or make its repository state appear cleaner than it actually is.
