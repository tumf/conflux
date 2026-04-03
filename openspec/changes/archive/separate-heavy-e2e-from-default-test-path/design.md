## Context

The project already values truthful test boundaries and recently identified that several Rust tests are integration/e2e in substance even when they live in the ordinary `tests/` surface. Those suites exercise real external boundaries such as `git`, worktrees, OS process cleanup, websockets, filesystem mutation, and wall-clock timing.

At the same time, repository hooks run broad Rust validation in ordinary commit flows. When heavy suites remain in the default surface, they slow down the default loop and expand the failure surface for commits that do not need full real-boundary validation.

## Goals / Non-Goals

- Goals:
  - Make the default Rust test path fast and suitable for frequent local iteration.
  - Preserve high-value heavy integration/contract/e2e coverage as an explicit opt-in tier.
  - Make the test-boundary choice obvious from layout, naming, and docs.
- Non-Goals:
  - Removing meaningful real-boundary coverage.
  - Forcing all heavy tests into mocks.
  - Building a full multi-stage CI redesign in this proposal.

## Decisions

- Decision: Separate heavy tests from the default path instead of merely optimizing them in place.
  - Why: the root issue is boundary classification and execution policy, not only runtime cost.
  - Alternatives considered: speed up individual tests while keeping them in the default path; rejected because the default path would still carry real-boundary coupling and broad failure surface.

- Decision: Preserve explicit heavy-tier execution.
  - Why: real `git`, process, and websocket tests still provide contract/integration value.
  - Alternatives considered: delete heavy suites outright; rejected because it would reduce confidence in important external boundaries.

- Decision: Align validation phases with test tiers.
  - Why: pre-commit and ordinary local loops need fast feedback, while broader readiness checks can intentionally pay the heavy cost.
  - Alternatives considered: keep one universal test command for all phases; rejected because it preserves the current slowdown/problem surface.

## Risks / Trade-offs

- If the separation is unclear, developers may stop running heavy tests entirely.
  - Mitigation: document explicit heavy-tier commands and keep them easy to invoke.

- Some tests may sit near the boundary between fast integration and heavy E2E.
  - Mitigation: classify by real external dependency and runtime cost, not only by filename.

- Hook or CI assumptions may need adjustment after the split.
  - Mitigation: include validation guidance updates as part of the change, not as follow-up guesswork.

## Migration Plan

1. Inventory and classify the current Rust test suites.
2. Choose and document the separation mechanism.
3. Move/mark heavy suites and preserve explicit execution commands.
4. Update default validation guidance and regression checks.
5. Verify both the fast default path and heavy opt-in path.

## Open Questions

- Whether the repository prefers `#[ignore]`, file layout separation, feature flags, or a dedicated command wrapper as the primary Cargo-facing mechanism.
- Whether acceptance should run the heavy tier always, conditionally, or via a separate readiness phase.
