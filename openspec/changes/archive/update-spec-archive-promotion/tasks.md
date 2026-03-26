## 1. Shared promotion engine
- [x] 1.1 Extract a shared helper at `skills/shared/cflx_spec_promotion.py` and update `skills/cflx-workflow/scripts/cflx.py` plus `skills/cflx-proposal/scripts/cflx.py` to use it (verification: both scripts import the shared helper and duplicate merge code is removed)
- [x] 1.2 Implement requirement-block parsing keyed by requirement heading so `ADDED`, `MODIFIED`, and `REMOVED` deltas can be promoted deterministically (verification: `python3 -m pytest skills/tests/test_cflx_spec_promotion.py` covers append, replace, and delete cases)

## 2. Semantic archive guards
- [x] 2.1 Add archive-check validation that simulates canonical promotion before archive writes files (verification: `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-spec-archive-promotion --strict` documents the new archive-check path and targeted tests cover it)
- [x] 2.2 Fail promotion when a `MODIFIED` or `REMOVED` delta references a missing canonical requirement or when promotion produces no canonical diff (verification: `python3 -m pytest skills/tests/test_cflx_spec_promotion.py -k no_op_or_missing_target`)

## 3. Archive operator guidance
- [x] 3.1 Update `skills/cflx-workflow/references/cflx-archive.md` and `skills/cflx-workflow/SKILL.md` to require canonical diff verification instead of trusting `Specs updated: [...]` output alone (verification: both files explicitly mention reviewing touched `openspec/specs/**` diffs)
- [x] 3.2 Keep the archive command path consistent across workflow docs so operators follow one supported implementation path (verification: `skills/cflx-workflow/references/cflx-archive.md` and `skills/cflx-workflow/SKILL.md` use the same archive command family)

## 4. Regression coverage
- [x] 4.1 Add promotion fixtures for `ADDED`-only, `MODIFIED`-only, `REMOVED`-only, and mixed deltas under `skills/tests/fixtures/archive_promotion/` (verification: fixture names map to each scenario)
- [x] 4.2 Add a regression fixture that mirrors the spec-only no-op archive failure uncovered in the session analysis (verification: `python3 -m pytest skills/tests/test_cflx_spec_promotion.py -k spec_only_no_op`)
