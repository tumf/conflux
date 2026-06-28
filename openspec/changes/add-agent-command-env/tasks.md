## Implementation Tasks

- [ ] Add `agent_command_env` to `OrchestratorConfig` as an optional string map with an accessor for effective command env. Completion condition: `src/config/types.rs` exposes parsed configured env as `HashMap<String, String>` or equivalent without changing existing required command validation. (verification: unit - add/extend `src/config/mod.rs` tests and run `cargo test config::` to prove JSONC parsing exposes configured keys/values.)

- [ ] Implement key-wise merge semantics for `agent_command_env` across global, project, and custom config layers. Completion condition: lower-priority env keys remain present unless a higher-priority config provides the same key, in which case the higher-priority value wins. (verification: unit - add/extend `src/config/mod.rs` merge tests and run `cargo test config::` to cover inherited, overridden, and high-priority-only keys.)

- [ ] Wire configured env into `AiCommandRunner` command spawning for all common AI-driven agent command execution paths. Completion condition: commands executed through `AiCommandRunner::execute_streaming_with_retry` receive configured env vars in child process environment. (verification: integration - add/extend `src/ai_command_runner.rs` tests and run `cargo test ai_command_runner::` with a command like `sh -c 'printf %s "$CFLX_TEST_AGENT_ENV"'` observing the configured value.)

- [ ] Wire configured env into legacy `AgentRunner` command spawning paths. Completion condition: legacy shell command builders used by `AgentRunner` apply configured env vars for both default cwd and explicit cwd command execution. (verification: unit - add/extend `src/agent/runner.rs` tests and run `cargo test agent::runner::` to prove configured env is applied without process-global env mutation.)

- [ ] Preserve scope boundaries for hooks and proposal sessions. Completion condition: hook execution continues to derive env from `HookContext` and existing hook behavior; ACP proposal sessions continue using `proposal_session.transport_env`. (verification: unit - add/extend `src/hooks.rs` and `src/server/acp_client.rs` or config tests, then run `cargo test hooks:: server::acp_client:: config::` to prove hooks do not read `agent_command_env` and `proposal_session.transport_env` remains independent.)

- [ ] Avoid logging configured environment variable values. Completion condition: command logs still show command text but do not include `agent_command_env` values; any env-related diagnostic logs include at most key names. (verification: unit - add assertion in `src/ai_command_runner.rs` or `src/agent/runner.rs` tests that spawned command strings/loggable command text do not contain a sentinel configured env value such as `secret-value`.)

- [ ] Update configuration documentation in `docs/guides/CONFIG.md`. Completion condition: `docs/guides/CONFIG.md` documents `agent_command_env`, scope, merge behavior, non-goals, and relationship to `proposal_session.transport_env`. (verification: manual - runnable command: run `git diff -- docs/guides/CONFIG.md openspec/changes/add-agent-command-env/specs/configuration/spec.md` and confirm examples match the spec delta.)

## Final Validation

Expected authoring validation: `cflx openspec validate add-agent-command-env --strict --evidence warn`

Expected implementation validation after code changes: run the relevant Rust unit/integration tests covering config parsing/merge and command env propagation, plus the repository lint/typecheck commands if configured.

## Future Work

- Add per-operation env maps only if a concrete need appears for distinct model/provider settings per command type.
- Add secret-provider indirection only if storing literal env values in global config proves insufficient.
