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

Deleting `~/.local/state/cflx/**` MUST NOT change the next action chosen for the same workspace contents, except as permitted by law 1a.

External logs, metrics, caches, and UI state are allowed only as non-authoritative observability outputs and MUST NOT be used as workflow control inputs.

### 1a. Narrow runtime pause/resume exception

Conflux MAY keep a versioned, revision-bound runtime record outside the managed worktree for the single purpose of temporarily pausing and resuming a change that acceptance reported as externally blocked.

Such a record MAY authoritatively control only:

- suppression of ordinary dispatch while the hold is displayed
- reconstruction of the non-terminal `stalled` operator status and its blocker presentation
- eligibility for an explicit operator retry and selection of acceptance as the resume phase

Such a record MUST NOT establish implementation completion, acceptance PASS, archive readiness, merge eligibility, or base integration. Those outcomes remain derivable from workspace file state, workspace git state, and base-branch tree comparison alone.

The record MUST bind repository identity, change ID, managed worktree identity and path, apply revision, and schema version, and MUST be reconciled against current repository, worktree, and git facts before it controls anything. A record that fails reconciliation MUST be invalidated or quarantined and MUST NOT override repository evidence.

Deleting or corrupting the record MUST fail safe: it may only drop the displayed hold. When repository evidence still shows a complete unarchived apply revision, Conflux MUST run acceptance again and MUST NOT infer PASS, archive, or merge.

Writing, reading, reconciling, consuming, or deleting the record MUST NOT mutate the managed worktree or make it dirty.

### 2. Constitution precedence

Proposal authors, reviewers, and implementers MUST read and follow this constitution when it exists.

A proposal that intentionally changes constitutional behavior MUST amend this document explicitly as part of the same change.

### 3. Truthful completion

Tasks, acceptance, and archive decisions MUST be based on repository-verifiable evidence.

No change may be treated as implemented, accepted, or archive-ready based only on narrative claims, checklist normalization, or hidden runtime state.
