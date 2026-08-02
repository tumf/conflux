## Implementation Tasks

- [ ] Select and document one canonical tracked v2 OpenAPI artifact and update generation targets so all duplicate files are removed or deterministically derived from it. (verification: integration - `make check-openapi` verifies a clean generation run produces no git diff outside the canonical artifact; verification-id: openapi-artifact-tests)

- [ ] Register every supported route, authoritative snapshot field, command variant/outcome, event envelope, typed error, auth scheme, replay rule, and remote worktree safety field in utoipa. (verification: integration - `make check-openapi` verifies schema tests assert representative required fields and complete route/command/error enums; verification-id: openapi-artifact-tests)

- [ ] Make `make check-openapi` regenerate to a temporary artifact and fail with a useful diff on tracked drift without overwriting unrelated working-tree changes. (verification: integration - `make check-openapi` verifies drift fixture changes a schema and proves the check fails, then passes after regeneration; verification-id: openapi-artifact-tests)

- [ ] Remove stale legacy routes and contradictory duplicate contract descriptions from tracked OpenAPI artifacts while preserving intentional current documentation. (verification: integration - `make check-openapi` verifies repository assertions prove removed paths are absent and all supported v2 paths remain; verification-id: openapi-artifact-tests)

- [ ] Add generated-consumer compilation or type assertions and document the exact regeneration/check commands. (verification: integration - `make check-openapi` verifies `make check-openapi` runs the consumer/schema check and fails on incompatible generated types; verification-id: openapi-artifact-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate enforce-v2-openapi-artifact --archive-gate`

The implementation must also pass `make check-openapi`.

## Future Work

- Hosted API documentation publication is an operational follow-up, not a completion gate.
