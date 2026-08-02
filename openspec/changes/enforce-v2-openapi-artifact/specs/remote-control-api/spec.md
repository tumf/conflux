## ADDED Requirements

### Requirement: Canonical OpenAPI ownership

The repository MUST define one source-generated OpenAPI artifact as the tracked contract of `/api/v2`. Every supported v2 route and schema MUST appear in that deterministically generated artifact. Duplicate artifacts MUST either be removed or generated deterministically from the same source and ownership rule. Stale legacy routes MUST NOT appear as supported API paths.

#### Scenario: Canonical artifact matches generated contract

**Given**: The repository is clean
**When**: The documented OpenAPI generation and check commands run
**Then**: Generation is byte-for-byte deterministic
**And**: The tracked canonical artifact has no diff
**And**: All supported v2 routes and schemas are present

#### Scenario: Contract drift fails validation

**Given**: A route, DTO field, command variant, error code, event envelope, or security declaration changes without regenerating the canonical artifact
**When**: `make check-openapi` runs
**Then**: The check fails with a useful diff
**And**: It does not overwrite unrelated working-tree changes

#### Scenario: Consumer uses the canonical artifact

**Given**: A generated client or schema assertion consumes the v2 contract
**When**: Repository-local verification runs
**Then**: It reads the canonical artifact
**And**: It compiles or validates every current command and authoritative snapshot field

#### Scenario: Security and recovery semantics are documented

**Given**: A client reads the canonical API contract
**When**: It inspects authentication, events, commands, and worktree schemas
**Then**: It can identify bearer-header authentication, fetch-streamed SSE, process incarnation, replay-gap resnapshot, revision and idempotency rules, and opaque worktree safety
