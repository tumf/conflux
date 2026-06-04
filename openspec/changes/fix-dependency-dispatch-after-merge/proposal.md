---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/queue_state.rs
  - src/dependency_targets.rs
  - src/parallel/tests/executor.rs
---

# Fix dependency dispatch after dependency merge

**Change Type**: implementation

## Problem/Context

Dependent changes can start apply while their dependency is still resolving. In the observed A/B/C case, B and C depend on A, but B and C become dispatchable as soon as A is archived/resolving instead of waiting until A is merged into the base branch.

Current canonical `parallel-execution` wording treats archived dependency targets as satisfied. The scheduler implementation mirrors this by accepting `DependencyTargetClass::Archived` without checking base-branch merge evidence.

## Proposed Solution

Change dependency dispatch gating so archive evidence is only a classification signal, not a satisfaction signal. A dependency is satisfied for dependent dispatch only when repository-visible base-branch evidence shows the dependency is merged.

The scheduler must continue to classify archived dependency references distinctly for diagnostics, but it must block dependents when the dependency is archived/resolving and not merged.

## Acceptance Criteria

- B/C-style dependents do not dispatch while dependency A is resolving or archived-but-not-merged.
- Dependents become eligible only after the dependency is merged to the base branch, assuming no other blockers and available slots.
- Rejected, missing, queued, in-flight, active-but-not-queued, and terminal-error dependency handling remains fail-closed as before.
- Dependency-blocked diagnostics remain stable and non-spamming when archived-but-not-merged dependencies are observed repeatedly.
- Dependency-resolved fresh workspace recreation still triggers when a previously blocked dependent becomes eligible after the dependency is merged.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` no longer treats `DependencyTargetClass::Archived` as immediately satisfied during dispatch selection.
- Dispatch selection calls base-branch merge verification for archived dependency references before allowing dependent dispatch.
- Regression tests cover archived-but-not-merged blocked behavior and merged dependency allowed behavior.
- Existing dependency classification tests are updated so `Archived` means archive evidence, not dispatch satisfaction.
- `cflx openspec validate fix-dependency-dispatch-after-merge --strict --evidence warn` passes.
- Relevant Rust tests pass, including dependency dispatch tests in `src/parallel/tests/executor.rs`.

## Out of Scope

- Changing archive finalization or merge implementation itself.
- Changing dependency metadata parsing.
- Introducing durable workflow-control state outside workspace/git/base-branch evidence.
