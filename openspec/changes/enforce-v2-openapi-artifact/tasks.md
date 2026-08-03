## Implementation Tasks

- [x] Select and document one canonical tracked v2 OpenAPI artifact and update generation targets so all duplicate files are removed or deterministically derived from it. (verification: integration - `make check-openapi` verifies a clean generation run produces no git diff outside the canonical artifact; verification-id: openapi-artifact-tests)

- [x] Register every supported route, authoritative snapshot field, command variant/outcome, event envelope, typed error, auth scheme, replay rule, and remote worktree safety field in utoipa. (verification: integration - `make check-openapi` verifies schema tests assert representative required fields and complete route/command/error enums; verification-id: openapi-artifact-tests)

- [x] Make `make check-openapi` regenerate to a temporary artifact and fail with a useful diff on tracked drift without overwriting unrelated working-tree changes. (verification: integration - `make check-openapi` verifies drift fixture changes a schema and proves the check fails, then passes after regeneration; verification-id: openapi-artifact-tests)

- [x] Remove stale legacy routes and contradictory duplicate contract descriptions from tracked OpenAPI artifacts while preserving intentional current documentation. (verification: integration - `make check-openapi` verifies repository assertions prove removed paths are absent and all supported v2 paths remain; verification-id: openapi-artifact-tests)

- [x] Add generated-consumer compilation or type assertions and document the exact regeneration/check commands. (verification: integration - `make check-openapi` verifies `make check-openapi` runs the consumer/schema check and fails on incompatible generated types; verification-id: openapi-artifact-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate enforce-v2-openapi-artifact --archive-gate`

The implementation must also pass `make check-openapi`.

## Implementation Notes

- evidence (task 1): `docs/openapi.yaml` is the sole tracked artifact (`OPENAPI_ARTIFACT` in `Makefile`); the stale root `openapi.yaml` is deleted; `src/bin/openapi_gen.rs` writes `conflux::web::openapi::document_yaml()` to stdout so the file, the `GET /api/v2/openapi.yaml` body, and the check all come from one function. `make openapi` followed by `git status --short` reports no change to any tracked file, and `the_repository_tracks_exactly_one_openapi_artifact` walks the working tree to prove no second artifact exists.
- evidence (task 2): `src/web/openapi.rs` registers all 16 v2 paths, 46 schemas, the `bearer_token` HTTP scheme with a document-wide requirement and explicit `security: []` on the four unauthenticated routes, and a long-form description covering incarnation scoping, fetch-streamed SSE, replay gaps, revision/idempotency, and opaque worktree IDs. `document()` carries a `debug_assert_eq!` against `SUPPORTED_V2_PATHS`, and `every_published_path_is_bound_with_the_authentication_it_declares` probes the real axum router so the constant, the artifact, and the router cannot disagree.
- evidence (task 3): `check-openapi` generates into `mktemp`, prints `diff -u`, and never writes the working tree. `the_drift_check_fails_on_a_mutated_schema_and_passes_after_regeneration` drops `event_sequence` from `EventEnvelope.required` on a scratch copy, runs the target's own `diff -u` predicate, asserts it fails and names the dropped field, then asserts regeneration clears it. A non-mutating `openapi-contract` prek hook runs the check in CI.
- evidence (task 4): the removed unversioned `/api/*`, `/ws`, and `/api/v1/*` surfaces are enumerated in `REMOVED_PATHS` and asserted absent by `no_removed_path_is_presented_as_supported`, while `the_artifact_publishes_exactly_the_supported_route_surface` asserts set equality with the supported surface, so removal cannot silently take a live route with it.
- evidence (task 5): `published_schemas_describe_what_the_server_actually_serializes` and `the_authoritative_snapshot_publishes_every_operator_field` build real DTO values field by field and validate them against the published schemas, which is the "generated consumer compiles" property in Rust — a new or renamed response field fails to compile the fixture. Regeneration and check commands are documented in `CONTRIBUTING.md`, `docs/guides/DEVELOPMENT.md`, and the generated-file banner at the top of the artifact itself.
- note (acceptance repair, attempt 1): the contract-discovery routes are inside the gate now, not beside it. `UNAUTHENTICATED_V2_PATHS` stays the published list and is unchanged, so the generated artifact is byte-identical; `is_unauthenticated_v2_path` wraps it for the gate and additionally exempts the Swagger UI's own assets under `/api/v2/docs/`, which are not published routes but must load without a token for the page to render. Bearer exemption is therefore the only thing those paths skip — origin policy, preflight, correlation IDs, and out-of-band credential refusal all apply.
- evidence (suite): `make check-openapi` passes (artifact matches; 16 contract assertions green in 0.10s), `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features web-monitoring -- -D warnings` are clean, and `cargo fmt --check` is clean.

## Future Work

- Hosted API documentation publication is an operational follow-up, not a completion gate.

## Current Acceptance Follow-up
- attempt: 1
- [ ] `src/web/openapi.rs:63-66` and the generated artifact promise rejection of query/subprotocol credentials on every path, but `src/web/remote_control_api/mod.rs:218-240` applies `gate` before adding `/api/v2/openapi.yaml`, `/api/v2/openapi.json`, and `/api/v2/docs`, so those routes bypass `reject_out_of_band_credentials`; route them through the gate while skipping only bearer enforcement and add router-level regression coverage.
  evidence: `router` now registers `/api/v2/openapi.yaml` and the SwaggerUi merge *before* `route_layer(gate)` (`src/web/remote_control_api/mod.rs:236-243`), and `gate` skips only the bearer check via the new `crate::web::openapi::is_unauthenticated_v2_path` (`src/web/openapi.rs:57-71`, `mod.rs:281`), so origin policy and `reject_out_of_band_credentials` now cover the contract routes; `query_credentials_are_refused_on_the_contract_discovery_routes`, `subprotocol_credentials_are_refused_on_the_contract_discovery_routes`, `a_rejected_origin_cannot_read_the_contract_documents`, and `the_contract_documents_stay_credential_free_behind_the_gate` in `src/web/remote_control_api/tests/auth_tests.rs` drive the real router and pass (32/32 auth tests, 267/267 `web::` lib tests, 16/16 `openapi_contract_tests`, `make check-openapi` clean).
