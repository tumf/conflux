## Implementation Tasks

- [ ] Refactor `scripts/cflx-log-mine.py` to scan log files incrementally and retain only bounded previous/next context needed for examples and marker hits (verification: unit - add or update repository tests for `scripts/cflx-log-mine.py` that generate a large fixture log and assert grouped errors plus manual/action markers are reported without using the current `read_lines()` whole-file path).
- [ ] Preserve output compatibility for text and JSON reports, including `files_seen`, `total_lines_scanned`, grouped examples, `manual_events`, and `action_events` fields (verification: integration - run `python3 scripts/cflx-log-mine.py --log-root <tmp-fixture-root> --format json --top 30` from the repository root and assert the JSON contains the existing top-level keys and representative hit dictionaries).
- [ ] Preserve `--change-id` filtering semantics for grouped examples and timeline markers without requiring full-file storage (verification: integration - run `python3 scripts/cflx-log-mine.py --log-root <tmp-fixture-root> --change-id alpha --format json` against a two-change fixture and assert only `alpha` appears in returned examples/markers).
- [ ] Preserve normalization/redaction of volatile values in grouped keys and emitted examples where grouping currently normalizes paths, ids, branches, and process ids (verification: unit - add fixture assertions around `normalize()` or JSON group keys in `scripts/cflx-log-mine.py` covering `/Users/example/...`, `project_id=`, `branch=`, `change_id=`, `pid=`, and `pgid=` values).
- [ ] Add a direct CLI regression check for large selected logs after a marker timestamp (verification: manual - create a temporary log root under `/var/folders/dg/xh2k12k51yb300kdz4xmtr7m0000gn/T/opencode`, write `.last-checked` and a large `.log`, then run `python3 scripts/cflx-log-mine.py --log-root <tmp-fixture-root> --top 30` from the repository root and confirm it emits all report sections within the normal interactive timeout).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-log-mine-streaming --archive-gate`
