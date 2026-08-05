---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/cli.rs
  - src/main.rs
  - src/web/openapi.rs
  - src/web/remote_control_api/mod.rs
  - tests/openapi_contract_tests.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/documentation/spec.md
verifications:
  - id: openapi-local-tests
    requirement: The CLI and live API expose the same complete generated OpenAPI contract without a tracked schema artifact
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: .github/workflows/ci.yml
    evidence: cargo test output covering CLI parsing, feature-disabled rejection, schema output, live endpoint parity, and contract completeness
    rerun: cargo test --features web-monitoring --test openapi_contract_tests --test openapi_cli_tests && cargo test --no-default-features --test openapi_cli_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add runtime OpenAPI schema command

**Change Type**: implementation

## Premise / Context

- `/api/v2/openapi.yaml` already generates the contract from `src/web/openapi.rs` at runtime.
- The tracked `docs/openapi.yaml` duplicates that source and forces an expensive drift check on unrelated commits.
- The standalone `openapi-gen` binary exists only to materialize the tracked artifact.
- Operators still need a non-server path to export the schema for client generation and inspection.

## Problem / Context

Maintaining `docs/openapi.yaml` creates a second representation of the API contract, release-version plumbing, repository-wide documentation references, and a commit-time drift check. Proposal-only changes pay this cost even though they cannot affect the API. Removing the file without a replacement would also remove the convenient offline export path.

## Proposed Solution

Add `cflx openapi`, which writes the generated OpenAPI 3.1 YAML to standard output using the same serialization function as `GET /api/v2/openapi.yaml`. The command must be read-only, bypass repository orchestration locking and startup side effects, and emit no non-schema text to standard output.

Remove `docs/openapi.yaml`, the `openapi-gen` binary, `make openapi`, `make check-openapi`, the static-artifact pre-commit hook, and release logic that owns or rewrites the file. Rewrite contract tests to validate the generated document and parity between CLI and live API instead of comparing a tracked artifact, and run those artifact-free contract tests in pull-request CI. Update canonical specs, generated-document banners, workflow comments, and documentation to make generated runtime/CLI output authoritative.

## Acceptance Criteria

- `cflx openapi` emits valid OpenAPI 3.1 YAML to stdout and exits successfully without requiring a Git repository or starting orchestration services.
- `cflx openapi > openapi.yaml` produces the same contract body as `GET /api/v2/openapi.yaml` from the same build.
- Failures emit actionable diagnostics to stderr and a non-zero exit status without mixing diagnostics into stdout.
- Builds without `web-monitoring` reject the command clearly rather than emitting an incomplete schema.
- `docs/openapi.yaml`, `openapi-gen`, static generation/check Make targets, and release ownership of the artifact are removed.
- Existing route, DTO, command, error, event, security, removed-route, and determinism assertions remain enforced against the generated document.
- User and contributor documentation describes CLI export and live API discovery instead of a tracked schema file.

## Explicit Completion Conditions

- CLI parsing and dispatch include `cflx openapi`, and command help documents stdout behavior.
- A runnable integration test parses CLI output as OpenAPI YAML and proves it matches the live endpoint output.
- Contract tests fail if a supported route or required published schema element is missing or if a removed route reappears, and pull-request CI runs those tests without regenerating a tracked artifact.
- A feature-disabled test proves `cflx openapi` rejects unavailable OpenAPI support clearly without schema output.
- Repository search finds no active ownership, generation, release, workflow-comment, generated-banner, or documentation dependency on `docs/openapi.yaml`, `make openapi`, or `make check-openapi` outside archived historical changes.
- `cargo test --features web-monitoring --test openapi_contract_tests --test openapi_cli_tests` and `cargo test --no-default-features --test openapi_cli_tests` pass. (The feature-disabled proof lives in an integration test rather than a `--bin cflx` unit test because `src/main.rs` opens with `#![cfg(not(test))]`, so a `--bin cflx` filter matches zero tests and would pass vacuously.)

## Split Rationale

The Rust pre-commit path filtering requested in the same discussion is tracked separately as `scope-rust-precommit-hooks`. It is independently implementable and verifiable; neither proposal consumes repository output from the other.

## Out of Scope

- Adding Swagger UI or another documentation server.
- Supporting JSON output or file-path flags; shell redirection is the export mechanism.
- Changing `/api/v2` routes, schemas, authentication, or API semantics.
- Retaining a generated OpenAPI file in the repository.
