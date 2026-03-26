## 1. Proposal classification
- [x] 1.1 Extend `skills/cflx-proposal/scripts/cflx.py` validation to recognize `Change Type: spec-only | implementation | hybrid` and reject invalid or missing values (verification: `python3 -m pytest skills/tests/test_cflx_proposal_change_types.py -k change_type_validation`)
- [x] 1.2 Update `skills/cflx-proposal/SKILL.md` so proposal authoring guidance explains when to use each change type and when to split a hybrid proposal instead (verification: `skills/cflx-proposal/SKILL.md` includes a change-type decision table or equivalent guidance)

## 2. Spec-only authoring flow
- [x] 2.1 Add spec-only scaffolding guidance that uses `## Specification Tasks` instead of `## Implementation Tasks` and requires a one-line expected canonical outcome for each delta (verification: `skills/cflx-proposal/SKILL.md` contains spec-only examples with canonical-outcome lines)
- [x] 2.2 Emit an archive-risk warning for spec-only proposals whose deltas are `MODIFIED` / `REMOVED` only or otherwise depend on archive promotion semantics (verification: `python3 -m pytest skills/tests/test_cflx_proposal_change_types.py -k archive_risk_warning`)

## 3. Acceptance behavior
- [x] 3.1 Update `skills/cflx-workflow/references/cflx-accept.md` and `skills/cflx-workflow/SKILL.md` so spec-only acceptance checks archive-readiness rather than unrelated runtime integration evidence (verification: both files explicitly mention archive simulation or canonical promotion checks for spec-only changes)
- [x] 3.2 Fail spec-only acceptance when archive simulation shows a no-op or unresolved `MODIFIED` / `REMOVED` target (verification: `python3 -m pytest skills/tests/test_spec_only_acceptance.py` covers pass and fail outcomes)

## 4. Regression coverage
- [x] 4.1 Add fixtures for `spec-only`, `implementation`, and `hybrid` proposals under `skills/tests/fixtures/proposal_modes/` (verification: fixture names map to all three modes)
- [x] 4.2 Add validation and acceptance tests proving the new mode-specific rules are enforced (verification: `python3 -m pytest skills/tests/test_cflx_proposal_change_types.py skills/tests/test_spec_only_acceptance.py`)
