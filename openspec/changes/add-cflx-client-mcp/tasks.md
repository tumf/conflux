## Implementation Tasks

- [x] **Task 1: Define process-local execution identity and client envelope fields.** Create an owner-incarnation-bound episode ID for every admission source, expose it through execution status and additive envelope fields, return the current ID for `already_admitted`, and define dequeue/retry/restart boundaries without making it authoritative. (verification: unit - episode identity lives with the reducer projection that creates it, so admission, concurrent already-admitted, dequeue/re-admit, retry, and restart cases are in `src/orchestration/execution_facts/tests.rs` and the additive envelope cases are in `src/client/envelope.rs`; run `cargo test --lib execution_facts` and `cargo test --lib client`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 2: Extract shared truthful completion observation.** Reuse the execution contract and repository completion oracle from `src/client/wait.rs` for both bounded waits and owner-side subscriptions without copying terminal classification logic. (verification: unit+integration - `src/client/completion.rs` owns the oracle and the claim predicate that both `src/client/wait.rs` and the owner-side dispatcher call, unit-tested in place with `cargo test --lib client`; the terminal-mode matrix (merged, base-published, branch-pushed, disappearance, rejection, failure, timeout, owner-replacement) exercises real Git and is therefore integration evidence re-run after the extraction with `cargo test --test client_cli_tests wait`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 3: Implement dedicated process-local execution-sink resources and the bounded dispatcher.** Add authenticated OpenAPI-documented GET/PUT/DELETE resources outside the command registry, UDS-only mutation, capability discovery, exact binding, idempotent/replace set semantics, register-after-terminal delivery, argv-only callbacks, fixed environment variables, bounded event files, terminal/blocked-edge dedupe, graceful owner-stopping delivery, timeout/output caps, and observability-only failures. (verification: integration - add `tests/client_completion_sink.rs` covering UDS/TCP, old-owner capability, registration races, replacement, callback failure, and owner restart, then run `cargo test --test client_completion_sink`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 4: Add `cflx client mcp`.** Implement stdio MCP initialize/initialized, ping, `tools/list`, and `tools/call` for status, enqueue, wait, notify set/get/clear by calling the existing client boundary; preserve stable envelope/outcome semantics, protocol-only stdout, and closed tool exposure. (verification: unit - add protocol negotiation, JSON-RPC framing, schema, tool-error mapping, protocol-only stdout, and closed-tool tests under `src/client/mcp.rs`, then run `cargo test --lib client::mcp`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 5: Verify MCP against a live long-lived TUI owner.** Start a fixture owner, call MCP status/enqueue/notify, complete one change, and prove exactly one callback for its execution while the owner remains alive; cover terminal-before-register, blocked recovery/re-entry, graceful shutdown, crash/restart fallback, and distinct retry identity. Keep this test behind `heavy-tests`. (verification: integration - add `tests/client_mcp_integration.rs` and run `cargo test --features heavy-tests --test client_mcp_integration`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 6: Add the optional OpenCode reference integration.** Add plugin, callback helper, low-frequency bounded owner-continuity fallback, exact cflx MCP tool filtering, loopback/session validation, mandatory automation marker, dedupe, and untrusted-event guidance under `examples/integrations/opencode-auto-resume/`. Keep the example repository-distributed but outside the crate package include list. (verification: integration - `tests/opencode_auto_resume_example.rs` covers a fake `cflx` and a fake loopback OpenCode server, completion with the mandatory marker, dedupe, the loopback/binding refusals, exact tool filtering, and owner restart; run `cargo test --test opencode_auto_resume_example`. The authoritative `cargo package --list` exclusion takes seconds to resolve, so it is `heavy-tests`-gated and run with `cargo test --features heavy-tests --test opencode_auto_resume_example`; the include-allowlist assertion stays in the default suite) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 7: Document compatibility, security, and lifecycle semantics.** Update `README.md`, `AGENTS.md`, and the example README for the four-command namespace, client-only MCP, UDS-only sink mutation, resident TUI, execution rather than process completion, crash limitations/fallback, ordinary OpenCode `role=user` messages, and safe token/event handling. Explain why one-shot event-file delivery is separate from the long-lived lifecycle adapter stdin stream. (verification: integration - this repository's CLI-surface and documentation assertions live in `tests/client_cli_tests.rs`, not a `cli_integration.rs`; the help and README assertions were added there. Run `cargo test --test client_cli_tests cli_surface`, `cargo test --test client_cli_tests documentation`, and `cargo run -- client mcp --help`) (verification-id: cflx-client-mcp-acceptance)
- [x] **Task 8: Run repository quality gates.** Keep tests above the repository's default speed budget behind `heavy-tests`. (verification: integration - run `cflx openspec validate add-cflx-client-mcp --strict --evidence warn`, `cflx openspec validate add-cflx-client-mcp --archive-gate`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`) (verification-id: cflx-client-mcp-acceptance)

## Final Validation

Run at the end of the implementation, not tracked as tasks.

- `cflx openspec validate add-cflx-client-mcp --strict --evidence warn` — passed
- `cflx openspec validate add-cflx-client-mcp --archive-gate` — passed
- `cargo test` — passed; every suite green, with `client_mcp_integration` and the
  `cargo package --list` exclusion correctly reported as ignored
- `cargo test --features heavy-tests --test client_mcp_integration` — 7 passed
- `cargo test --features heavy-tests --test opencode_auto_resume_example` — 7 passed
- `cargo test --no-default-features --test client_cli_tests` — 3 passed, covering
  the feature-disabled refusal of `cflx client mcp`
- `cargo clippy --all-targets --all-features -- -D warnings` — clean
- `cargo fmt --check` — clean

## Notes

- Speed budget: every new test completes in well under a second individually.
  The two that cannot — the live-owner MCP end-to-end suite and the
  `cargo package --list` exclusion — are behind `heavy-tests` and are ignored by
  the default suite.
- The `mcp` module is gated on `web-monitoring` like the rest of the client's
  owner-facing code; a build without it refuses `cflx client mcp` on stderr with
  the feature-unavailable exit status and writes nothing.
- `src/ids.rs` was added so the `/api/v2` DTOs and the orchestration-side
  execution registry mint identifiers from one generator: the registry exists in
  builds that have no web feature, where `dto::new_hex_id` is not compiled.

## Acceptance Criteria

The implementation is acceptable when all Implementation Tasks are checked with their cited repository evidence. Together those checks must prove MCP admission into an existing TUI, exact execution binding, completion notification while TUI remains alive, retry identity isolation, non-authoritative failure handling, optional OpenCode same-session continuation, and compatibility of existing CLI/TUI/API behavior.
