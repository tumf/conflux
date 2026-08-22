# Design

## Decision

Keep the existing verification model. Add a validator check over parseable task verification references rather than a second task taxonomy or runtime task selector.

## Cohesion key

For every active checkbox reference, parse:

- `verification-id`
- standard verification ownership marker (`unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`)
- concrete command text
- source line

A reused change-blocking ID is cohesive only when ownership marker and normalized concrete command match. This permits one focused command to cover coupled code-and-test work without allowing unrelated checks to disappear behind one ID.

## Heavy command boundary

Use a small deterministic syntax policy. It examines only declared rerun/verification command tokens, never prose. Initial rejected change-blocking forms cover container orchestrators, architecture emulators, benchmark selectors, explicit full/exhaustive/heavy selectors, and repeated execution loops. Diagnostics prescribe a short repository-local smoke command and a separate operational owner.

## Migration

The rule applies during proposal validation. Existing archived proposals remain historical evidence. Active proposals receive actionable errors only from explicit parseable syntax.
