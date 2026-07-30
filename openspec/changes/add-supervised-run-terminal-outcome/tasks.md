## Implementation Tasks

- [ ] **Task 1: Add the invocation-scoped supervised CLI option and validation** in `src/cli.rs`/`src/main.rs`, default it off, restrict it to non-interactive `cflx run`, and preserve ordinary run/TUI/server behavior. (verification: unit - parser and startup tests named `supervised_run_*` plus `cargo test supervised_run` prove valid/default/invalid routing; verification-id: supervised-run-tests)

- [ ] **Task 2: Define typed terminal outcome, terminal-cause, record, and exit-code models** with `schema_version: 1`, fixed field names, deterministic `completed|blocked|stalled|cancelled|failed` mapping, bounded sanitized reason data, and optional upstream identity populated only from observed repository facts. (verification: unit - serialization, privacy allow-list, omitted-optionals, and exhaustive outcome/cause/exit-code tests run via `cargo test supervised_run`; verification-id: supervised-run-tests)

- [ ] **Task 3: Refine and propagate typed run causes through orchestration and finalization** by replacing the dependency's aggregate `BlockedOrStalled` handoff with typed dependency/manual `Blocked` and acceptance/repair/verification/safe-unpublished-history `Stalled` causes, plus completed, cancelled, and non-resumable failed causes; classify clean verified publication failure as stalled, startup config/auth failure as failed, and fresh no-work versus recognized unpublished zero-work/all-skip recovery without parsing logs or lifecycle events. (verification: unit - orchestrator and scheduler tests inject each cause, including no-new-attempt pre-existing holds, and assert terminal classification through `cargo test supervised_run`; verification-id: supervised-run-tests)

- [ ] **Task 4: Wire one-shot supervised execution in `src/main.rs`** so controlled outcomes bypass the ordinary error/retry wait, perform bounded cleanup, emit one terminal record, and return the matching OS status; emit cancelled/3 only after cleanup completes, exit 1 without a record on cleanup deadline exhaustion, and retain the existing outer retry/web-control loop when disabled. (verification: integration - `tests/run_exit_tests.rs` process tests prove prompt exit and exact statuses/record presence for success, resumable hold, graceful cancellation, cancellation timeout, and failure while ordinary mode retains retry behavior; verification-id: supervised-run-tests)

- [ ] **Task 5: Reserve supervised stdout for the terminal JSONL record** by routing parent and spawned child startup/progress/tracing/stdout/warnings/errors to stderr or captured internal channels, centralizing exactly-once emission, and exiting 1 without a fallback record on serialization/write failure. (verification: integration - `tests/run_exit_tests.rs` captured parent/child stdout/stderr and closed-pipe cases run by `cargo test supervised_run` prove one parseable newline-terminated object on stdout, no mixed output, bounded stderr diagnostics, no duplicate record, and status 1 with no valid record on emission failure; verification-id: supervised-run-tests)

- [ ] **Task 6: Preserve lifecycle isolation and document the machine contract** so lifecycle adapter loss/backpressure cannot affect terminal output or exit status and CLI help documents fields, outcomes, exit codes, and crash/no-record interpretation. (verification: integration - lifecycle fixture plus process tests run with `cargo test supervised_run` and prove adapter missing/crash/backpressure does not alter result; verification-id: supervised-run-tests)

## Future Work

- A conflux-server proposal may consume the terminal record and exit status for container-job state, retry APIs, and auditing.
- A future output-format framework may generalize structured output across commands without changing schema version 1.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-supervised-run-terminal-outcome --archive-gate`

Repository quality gates expected before acceptance: `cargo test supervised_run`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings`.
