---
change_type: hybrid
priority: high
dependencies:
  - add-demo-capability-baseline
references:
  - specs/demo-capability/spec.md
  - src/demo.py
  - tests/test_demo.py
---

# Change: Define and implement demo-capability range validation

**Change Type**: hybrid

## Why
The spec for range validation and its runtime implementation must ship atomically — the spec scenarios are tested directly by the implementation tests, so they cannot be reviewed independently.

## What Changes
- Define new range-validation requirements in `specs/demo-capability`
- Implement runtime validation in `src/demo.py`
- Add tests

## Impact
- Affected specs: `demo-capability`
- Affected code: `src/demo.py`, `tests/test_demo.py`

## Non-Goals
- No UI changes

## Success Criteria
- Spec and implementation ship together
- Tests verify the spec scenarios
