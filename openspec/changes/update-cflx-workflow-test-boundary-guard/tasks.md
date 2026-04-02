## Implementation Tasks

- [ ] Update `skills/cflx-workflow/SKILL.md` Apply guidance to add a Unit Test Boundary Policy covering representative external boundaries and unit-vs-integration classification rules (verification: file explicitly names VCS/SCM, network/API, database, filesystem, OS process/CLI, clock/timer, and environment-dependent OS state as unit-test boundaries).
- [ ] Expand `skills/cflx-workflow/SKILL.md` Mock-First Policy and Truthful Completion Rules so unit-test tasks require isolated decision logic and genuinely unit-scoped verification (verification: file states that unit-test completion is invalid when tests rely on real external boundaries).
- [ ] Update `skills/cflx-workflow/references/cflx-apply.md` to mirror the same apply-mode unit-boundary and truthful-completion requirements (verification: file instructs apply-mode agents to extract logic or use mocks/fakes instead of real boundary dependencies for unit tests).
- [ ] Update `skills/cflx-workflow/references/cflx-accept.md` to add an acceptance guard for unit-test classification mismatches and the required FAIL/follow-up behavior when checklist claims become untruthful (verification: file explicitly tells acceptance to flag unit tests that rely on real external boundaries).
- [ ] Keep the OpenSpec delta in `openspec/changes/update-cflx-workflow-test-boundary-guard/specs/agent-prompts/spec.md` aligned with the workflow guidance for both apply and acceptance behavior (verification: delta includes requirements/scenarios for apply-side unit-test boundaries and acceptance-side mismatch detection).
- [ ] Run `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-cflx-workflow-test-boundary-guard --strict` and resolve any validation issues (verification: command exits successfully).

## Future Work

- Audit existing tests for retrospective reclassification after the new workflow guidance is adopted.
