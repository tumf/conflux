---
change_type: implementation
priority: medium
dependencies:
  - expose-authoritative-operator-snapshot
  - unify-remote-operator-commands
  - add-remote-parallel-control
  - fix-remote-event-projection
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - src/web/openapi.rs
  - docs/openapi.yaml
  - openapi.yaml
  - Makefile
verifications:
  - id: openapi-artifact-tests
    requirement: One canonical tracked OpenAPI artifact is generated deterministically and CI detects every schema or route drift
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: OpenAPI generation comparison and generated-contract test output
    rerun: make check-openapi
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Enforce the v2 OpenAPI artifact

**Change Type**: implementation

## Problem / Context

Generated `/api/v2` output, `docs/openapi.yaml`, and root `openapi.yaml` have drifted and do not provide one reliable contract for generated consumers. The authoritative snapshot and command additions cannot be consumed safely if schema changes can land without updating a canonical tracked artifact.

## Proposed Solution

Select one tracked OpenAPI artifact as canonical, generate it deterministically from the utoipa router/schema registration, remove or explicitly derive duplicate artifacts, and make `make check-openapi` fail on any route, command, DTO, error, event, or security-scheme drift. Document authenticated fetch-SSE, replay gaps, command polling, and remote worktree safety in schema descriptions.

## Acceptance Criteria

1. One documented command regenerates the canonical v2 OpenAPI artifact byte-for-byte.
2. `make check-openapi` fails when generated output and the tracked artifact differ and passes from a clean checkout.
3. Every supported v2 route, authoritative snapshot field, command variant, command outcome, event envelope, typed error, auth rule, and worktree safety field appears in the artifact.
4. Duplicate OpenAPI files are removed or generated from the same source with an explicit ownership rule; no stale legacy path is presented as supported.
5. Generated TypeScript or equivalent contract consumers compile against the canonical artifact in repository-local tests.

## Explicit Completion Conditions

- `src/web/openapi.rs`, Makefile targets, tracked artifact paths, and CI checks agree on one source of truth.
- Drift tests cover missing routes, stale command enums, omitted required fields, security declarations, and accidental legacy paths.
- Current API documentation identifies bearer-header authentication, fetch-streamed SSE, process incarnation, replay-gap resnapshot, revision/idempotency, and opaque worktree IDs.
- `make check-openapi` passes without modifying tracked files.

## Out of Scope

- Publishing external hosted documentation.
- Browser UI implementation.
- Reintroducing removed `/api/v1` or legacy single-instance routes.
