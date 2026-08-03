---
change_type: implementation
priority: high
dependencies: []
references:
  - src/main.rs
  - src/web/mod.rs
  - src/web/openapi.rs
  - src/web/remote_control_api/mod.rs
  - tests/openapi_contract_tests.rs
  - openspec/specs/web-monitoring/spec.md
  - openspec/specs/remote-control-api/spec.md
verifications:
  - id: startup-regression
    requirement: Default UDS startup reaches the first TUI render without eagerly generating the OpenAPI document while preserving the listener-before-orchestration contract
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: Test output records the default UDS startup timing relative to the no-UDS control and confirms the socket is usable before orchestration
    rerun: cargo test --features web-monitoring --test run_exit_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: openapi-compatibility
    requirement: OpenAPI JSON, YAML, Swagger UI, and the tracked canonical artifact remain available and consistent after startup initialization is deferred
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: Contract and route tests prove live resources match the canonical generated artifact without eager startup generation
    rerun: make check-openapi
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Speed up default API startup

**Change Type**: implementation

## Problem / Context

Web-enabled builds now start the repository-scoped Unix API listener for every default TUI, `cflx tui`, and `cflx run` invocation. Measured in this repository on macOS, default TUI startup produced its first terminal output in approximately 113 ms, while `--no-web-unix-socket` produced it in approximately 6.8 ms. The added latency occurs in the local API startup path, where router assembly eagerly constructs the generated OpenAPI document before binding the listener even when no client requests Swagger UI or an OpenAPI resource.

The listener must still bind before lifecycle adapters, AI subprocesses, or orchestration side effects. OpenAPI JSON, YAML, Swagger UI, authentication, and the canonical generated artifact must remain compatible.

## Proposed Solution

Remove eager OpenAPI document construction from listener startup. Configure the API documentation surface so OpenAPI serialization happens only when a client requests the OpenAPI resource or documentation UI, while preserving one generated contract source and the existing route/authentication behavior.

Add repository-local regression coverage that compares the default UDS startup path with the no-UDS control and fails when API initialization introduces a material fixed delay before the first TUI render. Keep the assertion tolerant of CI and machine variance by testing a bounded relative overhead rather than the investigation machine's absolute 6.8 ms value.

This remains one change because deferred contract generation, route compatibility, and startup timing all modify and verify the same router construction boundary. Splitting them could allow the performance change to complete without its API compatibility proof.

## Acceptance Criteria

- Default TUI startup with the required Unix listener no longer eagerly generates or serializes the OpenAPI document before the first terminal render.
- The Unix listener is still bound and usable before lifecycle adapters, AI subprocesses, or orchestration begin.
- A repository-local startup regression test demonstrates that enabling the default UDS adds only bounded listener/router overhead relative to `--no-web-unix-socket`, without encoding one machine's absolute timing.
- `/api/v2/openapi.json`, `/api/v2/openapi.yaml`, and `/api/v2/docs` remain available under the existing authentication and origin gate.
- Live OpenAPI resources and the tracked `docs/openapi.yaml` remain generated from the same source and pass deterministic drift checks.
- UDS safety, cleanup, TCP coexistence, and feature-disabled behavior remain unchanged.

## Explicit Completion Conditions

- The production startup call path in `src/main.rs`, `src/web/mod.rs`, and `src/web/remote_control_api/mod.rs` contains no eager call that constructs or serializes the OpenAPI document solely to assemble the router.
- A request-driven handler or equivalent lazy path serves the OpenAPI contract and Swagger UI references it without changing public paths.
- `cargo test --features web-monitoring --test run_exit_tests` passes a real-process default-UDS startup comparison and listener lifecycle assertions.
- `make check-openapi` passes and verifies the tracked artifact plus live contract behavior.
- `cargo fmt --check`, `cargo clippy --all-targets --features web-monitoring -- -D warnings`, and the default test suite pass.

## Out of Scope

- Removing the default Unix listener or changing its fail-fast startup contract.
- Changing API paths, schemas, authentication, CORS, or socket permissions.
- Replacing utoipa or Swagger UI with a new documentation framework.
- Optimizing unrelated TUI initialization, OpenSpec parsing, Git discovery, or periodic refresh work.
