## Implementation Tasks

- [ ] Add range-validation spec delta under `specs/demo-capability/spec.md` (verification: `skills/cflx-proposal/scripts/cflx.py validate hybrid-fixture --strict` passes)
- [ ] Implement runtime validation in `src/demo.py` (verification: `python3 -m pytest tests/test_demo.py -k validation`)
- [ ] Add tests for spec scenarios (verification: `python3 -m pytest tests/test_demo.py`)
