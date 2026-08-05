## MODIFIED Requirements

### Requirement: Canonical OpenAPI ownership

The generated OpenAPI document produced from the source declarations MUST be the canonical contract of `/api/v2`; the repository MUST NOT track a generated OpenAPI YAML or JSON artifact. Every supported v2 route and schema MUST appear in the deterministic document exposed by both `cflx openapi` and `GET /api/v2/openapi.yaml`. Stale legacy routes MUST NOT appear as supported API paths.

#### Scenario: CLI and live endpoint share the canonical contract

**Given**: one `cflx` build with web monitoring enabled
**When**: a client captures `cflx openapi` and `GET /api/v2/openapi.yaml`
**Then**: both outputs contain the same deterministic OpenAPI document
**And**: all supported v2 routes and schemas are present

#### Scenario: Contract completeness fails validation

**Given**: a route, DTO field, command variant, error code, event envelope, or security declaration is absent from the generated contract
**When**: repository-local OpenAPI contract verification runs
**Then**: verification fails with an assertion identifying the missing contract element
**And**: verification does not write generated artifacts into the working tree

#### Scenario: Consumer exports the canonical contract

**Given**: a generated client or schema assertion needs the v2 contract
**When**: it invokes `cflx openapi` or reads the live OpenAPI endpoint
**Then**: it receives the canonical generated document
**And**: it can validate every current command and authoritative snapshot field

#### Scenario: Security and recovery semantics are documented

**Given**: a client reads the generated canonical API contract
**When**: it inspects authentication, events, commands, and worktree schemas
**Then**: it can identify bearer-header authentication, fetch-streamed SSE, process incarnation, replay-gap resnapshot, revision and idempotency rules, and opaque worktree safety
