## Implementation Tasks

- [ ] Refactor the `/api/v2` router so listener startup does not construct or serialize the OpenAPI document, while preserving the existing documentation paths and shared authentication/origin gate. Completion requires the production router assembly path to contain no eager OpenAPI generation call and request handlers to return the real generated contract. (verification: integration - `make check-openapi`; verification-id: openapi-compatibility)
- [ ] Preserve Swagger UI behavior while pointing it at the request-driven OpenAPI JSON resource, without adding a dependency or duplicating the API contract. Completion requires `/api/v2/docs` to load configuration for `/api/v2/openapi.json` and the JSON/YAML resources to describe the same supported paths and schemas. (verification: integration - `make check-openapi`; verification-id: openapi-compatibility)
- [ ] Add a real-process startup regression test covering default TUI UDS startup versus `--no-web-unix-socket`. Completion requires the test to observe first terminal output through a pseudo-terminal or equivalent real binary boundary, assert bounded relative UDS overhead with variance tolerance, and fail if an eager fixed initialization delay returns. (verification: benchmark - `cargo test --features web-monitoring --test run_exit_tests`; relative timing is intentional because absolute startup time varies by host; verification-id: startup-regression)
- [ ] Extend listener lifecycle coverage so the optimized path still proves the UDS is usable before orchestration, failed startup remains side-effect free, and finite shutdown removes its owned socket. Completion requires success, failure, and cleanup assertions against the real listener path. (verification: integration - `cargo test --features web-monitoring --test run_exit_tests`; verification-id: startup-regression)
- [ ] Run formatting, linting, default tests, web-monitoring tests, and deterministic OpenAPI drift checks. Completion requires `cargo fmt --check`, `cargo clippy --all-targets --features web-monitoring -- -D warnings`, `cargo test`, `cargo test --features web-monitoring`, and `make check-openapi` to pass without modifying unrelated proposal files. (verification: integration - `cargo test --features web-monitoring && make check-openapi`; verification-id: openapi-compatibility)

## Future Work

- Tune broader startup budgets only if measurements identify additional bottlenecks outside local API router initialization.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate speed-up-default-api-startup --archive-gate`
