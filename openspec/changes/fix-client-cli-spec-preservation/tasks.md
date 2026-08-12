## Specification Tasks

- [x] Promote complete combined client requirements to `openspec/specs/cli/spec.md`. Expected canonical result: all original client scenarios and all correction scenarios coexist. Delta `specs/cli/spec.md` carries the three `MODIFIED` requirements in full: `Stable client output contract` (4 scenarios), `Intent-based enqueue` (8), `Observation-only completion wait` (9); scratch promotion produced exactly that canonical result with 102 insertions and 0 deletions.
- [x] Promote complete combined compatibility requirement to `openspec/specs/remote-control-api/spec.md`. Expected canonical result: original capability/revision scenarios and bearer-token scenarios coexist. Delta `specs/remote-control-api/spec.md` carries `Local client compatibility discovery` with all 7 scenarios (5 from `add-client-cli` plus 2 bearer-token scenarios) and the corrected three-paragraph description; scratch promotion produced that result with 38 insertions and 0 deletions.
- [x] Verify scenario preservation against both archived changes. Expected canonical result: no scenario heading from either source delta is lost. Every scenario heading and body from `2026-08-12-add-client-cli` and `2026-08-12-fix-client-cli-contract` is present byte-identical in the promoted canonical text; only the four requirement descriptions differ from `add-client-cli`, which is the intended supersession by `fix-client-cli-contract`.

## Notes

- Verification method: `openspec/` was copied into a throwaway git repository, `cflx openspec archive fix-client-cli-spec-preservation --yes` was run there, and the promoted canonical specs were compared heading-by-heading and body-by-body against both archived source deltas. The scratch directory was deleted; this workspace was never archived.
- Promotion touched only `openspec/specs/cli/spec.md` and `openspec/specs/remote-control-api/spec.md`, insertions only, satisfying "canonical promotion changes only the intended two spec files".
- `Existing-owner client namespace` (cli) and `Client observation does not alter API semantics` (remote-control-api) were never damaged — both already carry their full scenario sets in canonical — so this change deliberately does not restate them as `MODIFIED`.
- No product source or test file was changed, per acceptance criterion 3. Verification type is spec/manual review of promotion output; no unit-test ownership is claimed.

## Final Validation

- `cflx openspec validate fix-client-cli-spec-preservation --strict` → Validation passed (with the expected spec-only MODIFIED/REMOVED archive-risk warning).
- `cflx openspec validate fix-client-cli-spec-preservation --archive-gate` → Validation passed, exit 0.
