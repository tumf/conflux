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
- evidence (suite): `make check-openapi` passes (artifact matches; 16 contract assertions green in 0.10s), `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features web-monitoring -- -D warnings` are clean, and `cargo fmt --check` is clean.

## Future Work

- Hosted API documentation publication is an operational follow-up, not a completion gate.
