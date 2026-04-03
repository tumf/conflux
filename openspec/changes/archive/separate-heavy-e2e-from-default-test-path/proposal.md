---
change_type: implementation
priority: high
dependencies: []
references:
  - tests/e2e_tests.rs
  - tests/e2e_proposal_session.rs
  - tests/process_cleanup_test.rs
  - tests/merge_conflict_check_tests.rs
  - tests/e2e_git_worktree_tests.rs
  - .pre-commit-config.yaml
  - openspec/specs/testing/spec.md
---

# Change: Separate heavy E2E tests from the default test path

**Change Type**: implementation

## Problem / Context

The repository currently mixes heavy end-to-end and real-boundary integration tests into the default Rust test surface used by normal development loops and repository hooks.

Recent investigation showed that several tests exercise real `git`, worktree operations, OS processes, sockets, filesystem mutation, and wall-clock waiting. At the same time, repository hooks currently run `cargo clippy --locked --all-targets --all-features -- -D warnings` on every real commit. This means heavyweight E2E-oriented test targets increase both the runtime cost and the failure surface of ordinary commits, even when the developer is not intentionally validating full E2E behavior.

This hurts iteration speed and makes late-phase failures more likely, because heavy test targets are treated like ordinary default-path test assets instead of explicit opt-in validation layers.

## Proposed Solution

Separate heavy E2E / contract / real-boundary tests from the default test path.

This change will:
- classify existing Rust tests into fast default-path tests versus heavy opt-in tests;
- move or mark heavyweight real-boundary tests so they are not part of the ordinary developer loop by default;
- preserve high-value integration and contract coverage, but require those tests to be invoked explicitly through dedicated commands, tags, ignored tests, file layout, or equivalent repository-supported mechanisms;
- align pre-commit, acceptance, and broader validation workflows so each phase runs the appropriate test tier.

## Acceptance Criteria

- The repository has an explicit distinction between fast default-path tests and heavy opt-in E2E/contract tests.
- Tests that require real `git`, real process execution, real sockets, or avoidable wall-clock waiting are no longer implicitly treated as ordinary default-path tests.
- Pre-commit and day-to-day developer validation no longer depend on the heavy test tier by default.
- The heavy test tier remains runnable through explicit repository-documented commands or mechanisms.
- Test documentation and naming/file placement make the boundary between default-path and heavy tests obvious.

## Out of Scope

- Deleting all heavy integration/E2E tests.
- Replacing every real-boundary test with mocks.
- Redesigning the entire CI matrix beyond what is necessary to establish the default-path/heavy-path boundary.

## Impact

- Affected specs: `testing`
- Affected code: Rust test organization under `tests/`, developer validation commands, and possibly hook/verification documentation
