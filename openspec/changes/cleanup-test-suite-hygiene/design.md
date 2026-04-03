## Context

The repository already has testing requirements that prefer lower-level coverage over redundant higher-level duplication and explicitly allow removing redundant integration tests when an equivalent unit test exists. The repository also has prompt/acceptance requirements that forbid treating real external boundary checks as unit-test completion.

The current test suite partially follows these rules, but several files still mix scopes or rely on process-global mutation without a single shared serialization pattern.

## Goals / Non-Goals

- Goals:
  - Make test scope obvious from file placement and naming.
  - Reduce redundant or low-value tests without losing behavior coverage.
  - Serialize or isolate process-global test mutations.
  - Preserve high-value integration/contract coverage where real boundaries matter.
- Non-Goals:
  - Eliminating all real-boundary tests.
  - Refactoring production modules beyond what is needed to support clearer test boundaries.

## Decisions

- Decision: Treat test-hygiene cleanup as an implementation change rather than a spec-only change.
  - Why: The requested outcome changes repository structure, test placement, and verification mechanics rather than only documentation.
  - Alternatives considered: spec-only documentation of hygiene rules; rejected because repository already has partial rules and now needs code/test reorganization.

- Decision: Keep one proposal rather than splitting into multiple smaller hygiene proposals.
  - Why: The cleanup targets a single cohesive outcome — making test boundaries truthful and maintainable — and the inventory, consolidation, and guard introduction are tightly coupled.
  - Alternatives considered: separate proposals for env-guard fixes, e2e file splitting, and redundant test removal; rejected because they share the same acceptance logic and repository-wide classification objective.

- Decision: Prefer explicit test reclassification over blanket mocking.
  - Why: Some tests derive value specifically from exercising real `git`, process, websocket, or filesystem contracts.
  - Alternatives considered: convert all external-boundary tests to mocks; rejected because it would reduce contract confidence and exceeds requested scope.

## Risks / Trade-offs

- Removing tests can accidentally reduce behavior coverage.
  - Mitigation: only remove tests when scenario coverage is preserved at a better boundary and document the retained replacement coverage.

- Moving tests can introduce temporary failures due to imports, helpers, or fixture assumptions.
  - Mitigation: do the cleanup incrementally and require full `cargo fmt`, `cargo clippy`, and `cargo test` verification.

- Reclassification may reveal that some current "unit-style" tests are actually integration tests, increasing perceived integration footprint.
  - Mitigation: accept truthful classification as the desired outcome and extract pure helpers only where it clearly improves maintainability.

## Migration Plan

1. Inventory and classify existing tests.
2. Introduce shared guards/helpers for process-global state where needed.
3. Split mixed-scope files and move tests to appropriate locations.
4. Consolidate or delete redundant/trivial cases while preserving scenario coverage.
5. Run repository-wide verification and adjust fallout.

## Open Questions

- Whether the repository also wants a persistent spec-to-test mapping document generated or updated as part of this cleanup.
- Whether ambiguous test-scope naming should later be enforced by CI or reviewer guidance only.
