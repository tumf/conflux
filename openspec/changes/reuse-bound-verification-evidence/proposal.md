---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/orchestration/acceptance.rs
  - src/openspec.rs
  - openspec/specs/proposal-metadata/spec.md
  - openspec/CONSTITUTION.md
verifications:
  - id: bound-evidence-tests
    requirement: Acceptance reuses only repository-bound verification evidence that exactly matches the current Apply result and reruns when any binding is absent or stale
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance.rs
    evidence: cargo test orchestration::acceptance --lib
    rerun: cargo test orchestration::acceptance --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Reuse bound verification evidence

**Change Type**: implementation

## Problem / Context

Apply may already run the exact repository-local command declared for Acceptance. Acceptance currently has no trustworthy, repository-visible envelope proving that execution belongs to the current result, so it may repeat expensive work. Reusing an unbound log or narrative claim would create false PASS risk.

## Proposed Solution

Define an optional repository-local verification evidence sidecar under the change worktree. A Conflux-runtime-owned executor, not the Apply agent, creates it from a process the runtime directly supervises. Acceptance may reuse it only after fail-closed validation.

Each record binds:

- verification ID;
- full-length Apply commit object ID and tree ID used for the run;
- exact argv array and normalized working directory relative to repository root;
- tracked automation-file blob ID;
- tool executable identity, preferring immutable digest and otherwise an exact version plus executable file digest;
- start/end timestamps and exit code;
- evidence artifact path plus content digest;
- clean index and worktree state at capture, excluding only the designated runtime evidence directory.

Acceptance may reuse a record only when it was created by the runtime executor and all bindings match the current worktree, proposal declaration, automation blob, executable identity, and successful exit code. The current tree may differ from the bound commit only inside the designated evidence directory. Any agent-authored, missing, malformed, stale, dirty, mismatched, or unverifiable field means rerun; it never means PASS. Cheap checks remain rerunnable by a repository-tracked duration policy. Reuse decisions are derived entirely from repository-local evidence and current Git state.

## Acceptance Criteria

- Exact matching successful evidence can satisfy only its declared verification ID without rerunning the same command.
- Commit/tree, argv, cwd, automation blob, tool identity, artifact digest, exit code, and clean-state mismatches force rerun.
- Missing or malformed evidence forces rerun and produces an actionable reason.
- Evidence from another branch, worktree, verification ID, or executable cannot satisfy Acceptance.
- Reused evidence remains repository-verifiable and survives restart without external state or Git history pollution.
- A policy threshold keeps cheap commands on the existing rerun path.

## Explicit Completion Conditions

- A versioned evidence schema, Git-excluded repository-relative location, and evidence-path-excluded clean-state rule are documented and parsed fail-closed.
- A runtime-owned executor directly supervises eligible verification commands and rejects agent-authored envelopes.
- Tests cover exact reuse and each mismatch class, including dirty worktree and short-SHA rejection.
- Acceptance output states `reused` or `rerun` per verification ID and the reason without treating malformed evidence as failure of the implementation itself.
- `cargo test orchestration::acceptance --lib` passes.

## Out of Scope

- Reusing external, deployed, credentialed, benchmark, or physical-device observations.
- Global caches or state under `~/.local/state`.
- Skipping inexpensive format/lint/unit checks by default.
- Proposal cohesion validation or Acceptance runtime limits.

## Verification Ownership

Focused Acceptance tests own the bounded repository-local proof. Repository-wide checks remain hook-owned.
