## Implementation Tasks

- [ ] Add `envs` to `OrchestratorConfig` as an optional string map with an accessor for effective command env. Completion condition: `src/config/types.rs` exposes parsed configured env as `HashMap<String, String>` or equivalent without changing existing required command validation. (verification: unit - add/extend `src/config/mod.rs` tests and run `cargo test config::` to prove JSONC parsing exposes configured keys/values.)

- [ ] Implement key-wise merge semantics for `envs` across global, project, and custom config layers. Completion condition: lower-priority env keys remain present unless a higher-priority config provides the same key, in which case the higher-priority value wins. (verification: unit - add/extend `src/config/mod.rs` merge tests and run `cargo test config::` to cover inherited, overridden, and high-priority-only keys.)

- [ ] Implement `$VAR` and `${VAR}` expansion for `envs` string values using only the Conflux parent process environment. Completion condition: expansion supports `$HOME/path`, `${HOME}/path`, repeated variables, adjacent text, and unset variables as empty strings without invoking a shell. (verification: unit - add tests in `src/config/expand.rs` or `src/config/types.rs` and run `cargo test config::` for `$VAR`, `${VAR}`, unset variable, and unsupported shell syntax cases.)

- [ ] Wire expanded `envs` into `AiCommandRunner` command spawning for all common AI-driven agent command execution paths. Completion condition: commands executed through `AiCommandRunner::execute_streaming_with_retry` receive expanded env vars in child process environment while retaining normal parent process env inheritance. (verification: integration - add/extend `src/ai_command_runner.rs` tests and run `cargo test ai_command_runner::` with a command like `sh -c 'printf %s "$CFLX_TEST_AGENT_ENV"'` observing both literal and parent-expanded values.)

- [ ] Wire expanded `envs` into legacy `AgentRunner` command spawning paths. Completion condition: legacy shell command builders used by `AgentRunner` apply expanded env vars for both default cwd and explicit cwd command execution while retaining parent process env inheritance. (verification: unit - add/extend `src/agent/runner.rs` tests and run `cargo test agent::runner::` to prove expanded env is applied without mutating process-global env.)

- [ ] Preserve scope boundaries for hooks and proposal sessions. Completion condition: hook execution continues to derive env from `HookContext` and existing hook behavior; ACP proposal sessions continue using `proposal_session.transport_env`. (verification: unit - add/extend `src/hooks.rs` and `src/server/acp_client.rs` or config tests, then run `cargo test hooks:: server::acp_client:: config::` to prove hooks do not read `envs` and `proposal_session.transport_env` remains independent.)

- [ ] Avoid logging configured or expanded environment variable values. Completion condition: command logs still show command text but do not include `envs` values; any env-related diagnostic logs include at most key names. (verification: unit - add assertion in `src/ai_command_runner.rs` or `src/agent/runner.rs` tests that spawned command strings/loggable command text do not contain a sentinel expanded env value such as `secret-value`.)

- [ ] Update configuration documentation in `docs/guides/CONFIG.md`. Completion condition: `docs/guides/CONFIG.md` documents `envs`, scope, key-wise merge behavior, `$VAR` / `${VAR}` expansion, non-goals, and relationship to `proposal_session.transport_env`. (verification: manual - runnable command: run `git diff -- docs/guides/CONFIG.md openspec/changes/add-agent-command-env/specs/configuration/spec.md` and confirm examples match the spec delta.)

## Final Validation

Expected authoring validation: `cflx openspec validate add-agent-command-env --strict --evidence warn`

Expected implementation validation after code changes: run the relevant Rust unit/integration tests covering config parsing/merge, env expansion, and command env propagation, plus the repository lint/typecheck commands if configured.

## Future Work

- Add per-operation env maps only if a concrete need appears for distinct model/provider settings per command type.
- Add secret-provider indirection only if `$VAR` / `${VAR}` parent-env expansion proves insufficient.
