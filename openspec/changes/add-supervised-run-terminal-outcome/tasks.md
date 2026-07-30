## Implementation Tasks

- [ ] **Task 1: Add the invocation-scoped supervised CLI option and validation** in `src/cli.rs`/`src/main.rs`, default it off, restrict it to non-interactive `cflx run`, and preserve ordinary run/TUI/server behavior. (verification: unit - parser and startup tests named `supervised_run_*` plus `cargo test supervised_run` prove valid/default/invalid routing; verification-id: supervised-run-tests)

- [ ] **Task 2: Define typed terminal outcome, reason, record, and exit-code models** with `schema_version: 1`, fixed field names, deterministic `completed|blocked|stalled|cancelled|failed` mapping, bounded sanitized reason data, and optional upstream identity populated only from observed repository facts. (verification: unit - serialization, privacy allow-list, omitted-optionals, and exhaustive outcome/exit-code tests run via `cargo test supervised_run`; verification-id: supervised-run-tests)

- [ ] **Task 3: Propagate typed run outcomes through orchestration and finalization** so remote-confirmed upstream completion, blocked-only drain, resumable stall, cancellation, verification/push/auth/config/command failure, and ordinary no-work completion are distinguishable without parsing logs or lifecycle events. (verification: unit - orchestrator and scheduler tests inject each typed path and assert terminal classification through `cargo test supervised_run`; verification-id: supervised-run-tests)

- [ ] **Task 4: Wire one-shot supervised execution in `src/main.rs`** so controlled outcomes bypass the ordinary error/retry wait, perform bounded cleanup, emit one terminal record, and return the matching OS status; retain the existing outer retry/web-control loop when disabled. (verification: integration - `tests/run_exit_tests.rs` process tests prove prompt exit and exact statuses for success, resumable hold, cancellation, and failure while ordinary mode retains retry behavior; verification-id: supervised-run-tests)

- [ ] **Task 5: Reserve supervised stdout for the terminal JSONL record** by routing startup/progress/tracing/warnings/errors to stderr, centralizing exactly-once emission, and treating serialization/write failure as a failed process rather than printing a second result. (verification: integration - `tests/run_exit_tests.rs` captured stdout/stderr cases run by `cargo test supervised_run` prove one parseable newline-terminated object on stdout, no mixed logs, bounded stderr diagnostics, and no duplicate record; verification-id: supervised-run-tests)

- [ ] **Task 6: Preserve lifecycle isolation and document the machine contract** so lifecycle adapter loss/backpressure cannot affect terminal output or exit status and CLI help documents fields, outcomes, exit codes, and crash/no-record interpretation. (verification: integration - lifecycle fixture plus process tests run with `cargo test supervised_run` and prove adapter missing/crash/backpressure does not alter result; verification-id: supervised-run-tests)

## Future Work

- A conflux-server proposal may consume the terminal record and exit status for container-job state, retry APIs, and auditing.
- A future output-format framework may generalize structured output across commands without changing schema version 1.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-supervised-run-terminal-outcome --archive-gate`

Repository quality gates expected before acceptance: `cargo test supervised_run`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings`.
