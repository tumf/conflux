## Implementation Tasks

- [x] Replace admission-oriented client enqueue with target-scoped multi-proposal execution-mark desired-state writes; mark and unmark MUST submit only `SetExecutionMark`, preserve unrelated marks, and return after command settlement without admission polling. (verification: unit - `cargo test --lib client::control::tests`; verification-id: client-tui-control-parity)
- [x] Add explicit client start, graceful-stop, and force-stop actions that delegate to the existing shared `OperatorIntent::Start`, `OperatorIntent::Stop`, and `OperatorIntent::ForceStop` transactions without reimplementing mode, retry, analyze, cancellation, or scheduler policy. (verification: integration - `cargo test --test client_cli_tests control`; verification-id: client-tui-control-parity)
- [x] Contract MCP to `cflx_status`, `cflx_control`, and `cflx_subscribe`; preserve route resolution, JSON-RPC, bounded framing, and protocol-only stdout; validate 1–64 distinct change IDs and expose the specified operation/outcome vocabulary. Remove MCP wait while retaining CLI wait. (verification: integration - `cargo test --test client_mcp_integration control`; verification-id: client-tui-control-parity)
- [x] Add a process-local proposal-scoped subscription registry with atomic 1–64 target set/clear, bounded named get, current/future-execution binding, replace/clear race semantics, episode-keyed dedupe, late-terminal delivery, and owner-restart invalidation while retaining callback sandboxing. (verification: unit - `cargo test --lib completion_sink`; verification-id: proposal-subscription-tests)
- [x] Expose authenticated subscriptions through capability-advertised `GET|PUT|DELETE /api/v2/proposals/{change_id}/subscription`, MCP, and CLI; require instance/change binding, keep mutation UDS-only, redact argv over TCP, generate OpenAPI, and avoid workflow commands/state revision. (verification: integration - `cargo test --test client_mcp_integration subscribe`; verification-id: proposal-subscription-tests)
- [x] Remove OpenCode/Hermes auto-resume hooks, both example directories and tests, stale README/AGENTS/client-test links, and enqueue/auto-resume guidance from both embedded skill source files; retain explicit notification docs only. (verification: integration - `cargo test --test install_skills_test && ! rg -n "auto-resume|cflx_enqueue|cflx client enqueue|cflx client notify" README.md AGENTS.md skills`; verification-id: proposal-subscription-tests)
- [x] Update OpenAPI, MCP initialization text, CLI help, README, bundled skill docs, and canonical specs; preserve every unrelated scenario in each MODIFIED requirement and explicitly remove retired auto-resume requirements. (verification: integration - `cargo test --test client_cli_tests help && cargo test --test client_mcp_integration tools_list`; verification-id: client-tui-control-parity)
- [x] Add a promotion-safety regression that applies this change's spec deltas to canonical specs and proves every non-retired pre-existing scenario remains present. (verification: unit - `cargo test --lib openspec_cmd::promotion`; verification-id: client-tui-control-parity)
- [x] Run the focused implementation gate and record its output. (verification: integration - `cargo test --lib client::control::tests && cargo test --lib completion_sink && cargo test --test client_cli_tests control && cargo test --test client_mcp_integration`; verification-id: client-tui-control-parity)
- [x] Run the final proposal/spec gate and record its output against `openspec/changes/align-client-mcp-with-tui-control/`. (verification: integration - inspect `openspec/changes/align-client-mcp-with-tui-control/specs/`, run `cargo test --lib openspec_cmd::promotion`, then run `cflx openspec validate align-client-mcp-with-tui-control --strict --evidence warn && cflx openspec validate align-client-mcp-with-tui-control --archive-gate && git diff --check`; verification-id: proposal-subscription-tests)

## Future Work

- None.

## Final Validation

Recorded against the worktree-built binary (`./target/debug/cflx`), not the
released one on `PATH`: this change edits the client and API surfaces the
validator's own tests exercise, so a released binary would report a contract
that no longer exists.

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets` — no warnings.
- `cargo test` (default suite) — all targets pass; 3986 lib tests, 74
  `client_cli_tests`, 19 `client_completion_sink`, 4 `client_mcp_integration`
  (15 heavy ignored), 10 `install_skills_test`, 19 `openapi_contract_tests`.
- `cargo test --features heavy-tests --test client_mcp_integration` — 19 passed.
- `cargo test --features heavy-tests --test client_cli_tests` — 74 passed.
- `cargo test --features heavy-tests --test client_completion_sink` — 20 passed.
- Focused implementation gate: `cargo test --lib client::control::tests` (14),
  `cargo test --lib completion_sink` (17), `cargo test --test client_cli_tests
  control` (16), `cargo test --test client_mcp_integration` (4 + 15 heavy).
- Documentation gate: `cargo test --test client_cli_tests help` (5),
  `cargo test --test client_mcp_integration tools_list` (1).
- Promotion safety: `cargo test --lib openspec_cmd::promotion` — 9 passed,
  including the repository scan over this change's own deltas.
- `cflx openspec validate align-client-mcp-with-tui-control --strict --evidence warn` — passed.
- `cflx openspec validate align-client-mcp-with-tui-control --archive-gate` — passed.
- `git diff --check` — clean.

## Notes

- The promotion-safety regression found a real gap while it was being written:
  six canonical scenarios would have disappeared silently, because a MODIFIED
  delta block replaces its requirement wholesale. Five were intentional
  retirements of the withdrawn enqueue/notify surface and are now declared under
  `## Retired Scenarios` in the proposal; the sixth — `Wait certifies evidence
  from the selected project` — was not a retirement at all and now moves from the
  MCP requirement to the CLI namespace requirement that still owns the oracle.
- The regression's helpers are deliberately *not* wired into
  `validate --archive-gate`. This change adds the check, not a new gate, and
  turning it into one would start refusing archives under a rule no existing
  proposal has been reviewed against.
- A proposal subscription does not displace a sink registered directly against a
  live execution, and clearing the proposal does not detach one. The two surfaces
  are separate by contract, and the alternative — a standing rule silently
  replacing a specific registration, which a later `clear` would then remove — is
  the interference that separation exists to prevent.
- A restart discovered mid-sequence reports `owner_restarted` rather than
  `partial_intent`: the marks that settled belonged to a process that is gone, so
  claiming they stand would be false, and the request stops instead of walking
  the rest of the target list into an incarnation that never saw the first
  command.
- `cflx client subscribe` requires `--instance-id`; the MCP tool accepts it as
  optional. A shell caller that registered a callback did observe an incarnation
  and can be told `owner_restarted`; a model that omits it had nothing to
  remember, and inventing an expectation for it would be a refusal about a
  disagreement that never existed.
