---
change_type: implementation
priority: medium
dependencies: []
references:
  - skills/cflx-workflow/SKILL.md
  - skills/cflx-workflow/references/cflx-apply.md
  - skills/cflx-workflow/references/cflx-accept.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: Strengthen cflx-workflow test boundary and acceptance guard

**Change Type**: implementation

## Problem / Context

The current `cflx-workflow` apply guidance already includes a mock-first policy, but it does not define unit-test boundaries explicitly enough. In practice, this allowed apply-mode agents to satisfy a "unit test" task by adding a test that exercised real git/worktree behavior, which is closer to integration coverage than unit coverage.

The acceptance guidance also lacks an explicit guard for test classification mismatches. As a result, acceptance may allow tasks claiming unit-test coverage to pass even when the added tests rely on real external boundaries.

## Proposed Solution

Strengthen the workflow skill and its references so apply and acceptance share a clear policy for test scope:

- Add a unit test boundary policy to apply guidance that forbids unit tests from directly depending on stateful external boundaries.
- Expand the mock-first policy so logic-oriented tests must isolate decision logic behind helpers, traits, interfaces, or in-memory fakes instead of real git/process/network/filesystem/time dependencies.
- Tighten truthful completion rules so a task claiming unit-test coverage is only complete when the resulting tests are genuinely unit-scoped and do not rely on real external boundaries.
- Add an acceptance guard that flags unit/integration classification mismatches and fails review when the mismatch makes checklist completion untruthful.

## Acceptance Criteria

- `skills/cflx-workflow/SKILL.md` defines a Unit Test Boundary Policy under Apply and makes truthful completion conditional on actual unit-scoped tests for unit-test tasks.
- `skills/cflx-workflow/references/cflx-apply.md` mirrors the same unit-boundary and truthful-completion guidance for apply-mode execution.
- `skills/cflx-workflow/references/cflx-accept.md` requires acceptance to detect unit-test classification mismatches and to fail when a completed unit-test task is only supported by integration-style tests.
- The OpenSpec delta updates `agent-prompts` so the canonical prompt policy covers both apply-side unit-test boundaries and acceptance-side classification guard behavior.
- `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-cflx-workflow-test-boundary-guard --strict` passes.

## Out of Scope

- Reclassifying or rewriting existing tests across the repository.
- Adding static analysis that automatically classifies every test in the codebase.
- Changing Rust test framework conventions or test directory layout outside the workflow guidance.
