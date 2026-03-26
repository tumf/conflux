# Change: Harden spec archive promotion

## Why
Session analysis identified a false-success gap in the skill-local OpenSpec archiver: change deltas may contain `MODIFIED` or `REMOVED` requirements, but the current merge helper only appends `ADDED` blocks in `skills/cflx-workflow/scripts/cflx.py` and the duplicated implementation in `skills/cflx-proposal/scripts/cflx.py`. As a result, archive can report success while leaving canonical specs unchanged.

The same analysis also showed that `validate --strict` currently proves only that a delta file exists and is syntactically well-formed. It does not prove that archiving the change will actually mutate the canonical specs. This leaves spec-only changes vulnerable to silent no-op promotion.

## What Changes
- Extract a shared spec-promotion helper so `cflx-workflow` and `cflx-proposal` stop carrying drift-prone duplicate merge logic.
- Replace append-only archive behavior with requirement-name-aware promotion for `ADDED`, `MODIFIED`, and `REMOVED` deltas.
- Add semantic archive checks that fail when a delta targets a missing canonical requirement or when promotion would produce no canonical diff.
- Strengthen archive guidance so operators verify canonical spec diffs instead of trusting `Specs updated: [...]` output alone.
- Add regression tests for `ADDED`-only, `MODIFIED`-only, `REMOVED`-only, and mixed delta cases, including a spec-only fixture that would previously archive as a no-op.

## Impact
- Affected specs: `archive-promotion`
- Affected code: `skills/cflx-workflow/scripts/cflx.py`, `skills/cflx-proposal/scripts/cflx.py`, shared helper/tests under `skills/`
- Dependencies: none; this is the foundation for safer spec-only proposal support

## Non-Goals
- Redesigning the full OpenSpec file format beyond the existing `ADDED` / `MODIFIED` / `REMOVED` requirement model
- Changing Conflux runtime orchestration outside archive/promotion validation and guidance
- Bundling proposal authoring mode changes or post-run review policy into the same implementation

## Success Criteria
- A `MODIFIED` delta replaces the matching canonical requirement block instead of appending a second copy.
- A `REMOVED` delta deletes the matching canonical requirement block and fails clearly when the target does not exist.
- Archive or archive-check fails when promotion would leave the canonical spec unchanged despite targeted `MODIFIED` / `REMOVED` deltas.
- Operators are instructed to confirm the canonical spec diff directly after archive.
- Regression tests cover the previously silent no-op promotion path.
