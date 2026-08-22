## Implementation Tasks

- [ ] Change client wait timeout parsing and arguments so omission and `--timeout 0` produce an unbounded operation duration, while positive durations retain the existing validation ceiling (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: client-wait-tests)
- [ ] Adapt the wait loop to support no overall deadline without weakening bounded transport/Git subprocess cleanup, repository evidence, or observation-only behavior (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: client-wait-tests)
- [ ] Add regression coverage for omitted timeout, explicit zero, explicit positive timeout, and eventual settlement of an unbounded wait (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: client-wait-tests)
- [ ] Update CLI help and operator documentation to describe default `0`, unbounded semantics, and the continued availability of explicit positive deadlines (verification: integration - `cargo test --test client_cli_tests wait_`; verification-id: client-wait-tests)

## Future Work

Proposal subscriptions remain the durable asynchronous notification mechanism for callers that must not keep a process open.

## Final Validation

Archive validation is the authoritative OpenSpec gate. Expected command: `cflx openspec validate default-client-wait-unbounded --archive-gate`.
