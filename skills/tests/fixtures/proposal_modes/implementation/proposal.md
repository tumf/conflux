# Change: Implement demo-capability validation

**Change Type**: implementation

## Why
Add runtime validation to the demo capability so it enforces the range requirements defined in the spec.

## What Changes
- Implement input range validation in `src/demo.py`
- Add unit tests for validation edge cases

## Impact
- Affected specs: `demo-capability`
- Affected code: `src/demo.py`, `tests/test_demo.py`

## Non-Goals
- No spec authoring changes in this proposal

## Success Criteria
- Validation logic rejects out-of-range inputs at runtime
- Tests pass
