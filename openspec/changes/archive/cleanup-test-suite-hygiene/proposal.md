---
change_type: implementation
priority: medium
dependencies: []
references:
  - tests/e2e_tests.rs
  - tests/e2e_proposal_session.rs
  - tests/process_cleanup_test.rs
  - tests/install_skills_test.rs
  - tests/merge_conflict_check_tests.rs
  - src/test_support.rs
  - openspec/specs/testing/spec.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: Reorganize test suite hygiene and test-boundary classification

**Change Type**: implementation

## Problem / Context

The current Conflux test suite mixes pure unit tests, integration tests, and shell-driven pseudo-E2E tests in ways that make intent unclear and maintenance costly.

Recent repository analysis identified several concrete issues:
- `tests/e2e_tests.rs` mixes trivial command-format assertions, mock-script integration checks, and real `git` worktree flows in one file.
- Some tests that exercise real external boundaries (`git`, shell commands, filesystem state, environment mutation, timers, sockets) are structurally close to unit-style coverage claims, making scope classification harder to audit.
- Process-global state changes such as `PATH`, `HOME`, and current working directory are only partially serialized, increasing flaky-risk under parallel test execution.
- The current crate/test structure appears to execute some unit-test modules twice through both `src/lib.rs` and `src/main.rs`, inflating total `cargo test` runtime and obscuring ownership of test placement.
- Some async tests rely on real elapsed time for debounce, retry, and polling behavior (including multi-second `sleep` calls), causing the slowest test groups to be dominated by wall-clock delay rather than behavior complexity.
- Existing specs already require removing redundant integration tests when a lower-level unit test covers the same scenario, and require classifying real external boundary checks as integration/e2e rather than unit coverage.

## Proposed Solution

Tighten the repository's test hygiene by explicitly reorganizing test boundaries and reducing redundant or low-value tests.

This change will:
- separate pure unit concerns from integration/e2e concerns using file placement and naming that reflects actual test scope;
- remove or consolidate trivial / redundant tests whose value is already covered by lower-level or higher-level tests;
- remove or refactor duplicated unit-test execution paths caused by crate/module structure so the same test logic is not compiled and run twice through both library and binary targets;
- replace real-time waiting in test logic where practical with deterministic time control, injected delays, or test-only timing configuration so debounce/retry/polling tests do not spend seconds of wall-clock time per case;
- add shared test helpers/guards for process-global state mutations (`PATH`, `HOME`, `cwd`) so parallel execution does not introduce hidden coupling;
- keep valuable contract and integration coverage (for example real `git merge-tree`, worktree behavior, process cleanup, proposal-session websocket/API flows) while labeling and structuring them as integration/e2e coverage rather than unit coverage;
- document a repeatable hygiene policy for future test additions so new tests preserve clear boundaries.

## Acceptance Criteria

- Test files and names make it clear whether a test is unit, integration, contract, or e2e.
- Redundant or trivial tests identified in the hygiene review are either removed or consolidated without losing scenario coverage.
- Unit-test modules are not unintentionally executed twice through both library and binary crate targets.
- Tests that verify debounce, retry, timeout, or polling behavior avoid unnecessary wall-clock waits where deterministic time control or test-specific timing overrides are feasible.
- Tests that mutate process-global state are serialized through shared guards or rewritten to avoid global mutation races.
- Real external boundary checks are retained only where they provide integration/contract value, and are not used to claim pure unit coverage.
- `cargo test` runtime is improved specifically by removing duplicate execution paths and reducing multi-second real-time waits in the slowest test groups.
- The repository documents and enforces a repeatable pattern for future test-boundary decisions.

## Out of Scope

- Rewriting the full test suite to eliminate every real external dependency.
- Replacing all integration/e2e tests with mocks.
- Large-scale production refactors unrelated to test structure and classification.

## Impact

- Affected specs: `testing`
- Affected code: `tests/e2e_tests.rs`, `tests/e2e_proposal_session.rs`, `tests/process_cleanup_test.rs`, `tests/install_skills_test.rs`, `src/test_support.rs`, and related module-local test files
