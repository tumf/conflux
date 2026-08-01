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
