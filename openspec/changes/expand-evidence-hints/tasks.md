# Tasks: Expand _EVIDENCE_HINTS

## Implementation Tasks

- [x] Add Node.js ecosystem hints (`"npm test"`, `"npm run "`, `"npx "`, `"yarn "`, `"pnpm "`) to `_EVIDENCE_HINTS` tuple in `skills/cflx-proposal/scripts/cflx.py:49` (verification: `python3 skills/cflx-proposal/scripts/cflx.py validate expand-evidence-hints --strict --evidence error` passes)
- [x] Add Rust ecosystem hints (`"cargo test"`, `"cargo build"`) to `_EVIDENCE_HINTS` tuple in `skills/cflx-proposal/scripts/cflx.py:49` (verification: `grep -c 'cargo test' skills/cflx-proposal/scripts/cflx.py` returns 1)
- [x] Add Go ecosystem hint (`"go test"`) to `_EVIDENCE_HINTS` tuple in `skills/cflx-proposal/scripts/cflx.py:49` (verification: `grep -c 'go test' skills/cflx-proposal/scripts/cflx.py` returns 1)
- [x] Add test directory and file pattern hints (`"test/"`, `".spec"`, `".test"`) to `_EVIDENCE_HINTS` tuple in `skills/cflx-proposal/scripts/cflx.py:49` (verification: `grep -c '\.spec' skills/cflx-proposal/scripts/cflx.py` returns 1)
- [x] Verify existing tests still pass (verification: `python3 skills/cflx-proposal/scripts/cflx.py validate expand-evidence-hints --strict --evidence error` passes with no new failures)

## Future Work

- Add unit tests for `_has_repository_evidence_hint` covering each new hint
- Consider adding hints for other ecosystems (Ruby `bundle exec`, Elixir `mix test`, etc.) as needed
