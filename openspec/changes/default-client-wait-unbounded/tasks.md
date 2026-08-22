## Implementation Tasks

- [x] Change client wait timeout parsing and arguments so omission and every exactly-zero spelling (`0`, `0s`, `0ms`, `0m`, `0h`) produce an unbounded operation duration represented as an optional deadline, while positive durations retain the existing minimum and maximum validation and the `--timeout 0s` usage-rejection test moves to asserting the sentinel (verification: integration - `cargo test --test client_cli_tests`; verification-id: client-wait-tests)
- [x] Adapt the wait loop to support no overall deadline, introducing a finite per-invocation deadline for every Git subprocess an unbounded wait spawns (never passing `None` down to `run_git`), handling inner expiry as retry or typed evidence rather than operation `timeout`, and preserving repository evidence and observation-only behavior (verification: integration - `cargo test --test client_cli_tests`; verification-id: client-wait-tests)
- [x] Add regression coverage for omitted timeout, explicit zero spellings, explicit positive timeout, eventual settlement of an unbounded wait, and termination/reaping of a stalled Git child at the per-invocation deadline without an operation `timeout` outcome (verification: integration - `cargo test --test client_cli_tests`; verification-id: client-wait-tests)
- [x] Update CLI help and operator documentation to describe default `0`, unbounded semantics, and the continued availability of explicit positive deadlines — including AGENTS.md, whose "bounded CLI completion oracle" phrasing for `cflx client wait` becomes stale (verification: integration - `cargo test --test client_cli_tests`; verification-id: client-wait-tests)

## Future Work

Proposal subscriptions remain the durable asynchronous notification mechanism for callers that must not keep a process open.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected command: `cflx openspec validate default-client-wait-unbounded --archive-gate`.
