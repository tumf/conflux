## Implementation Tasks

- [ ] In `src/server/proposal_session.rs::create_session`, after building the ACP config, check if `transport_env` contains `OPENCODE_CONFIG`; if not, auto-generate `opencode-proposal.jsonc` in the server data dir and insert `OPENCODE_CONFIG=<path>` into the env map before spawning ACP (verification: `cargo test` passes, unit test confirms the env var is set when not explicitly configured)
- [ ] Implement the auto-generation helper: write `{ "$schema": "https://opencode.ai/config.json", "mode": "spec" }` to `<data_dir>/opencode-proposal.jsonc` if the file does not exist; return the path (verification: unit test confirms file creation and idempotency)
- [ ] Update `tests/e2e_proposal_session.rs` to verify the ACP subprocess receives `OPENCODE_CONFIG` in its environment when `transport_env` does not override it (verification: `cargo test --test e2e_proposal_session`)
- [ ] Run full verification (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) and fix any regressions (verification: all three exit 0)

## Future Work

- Manually verify against a real `opencode acp` binary that the spec agent is activated in proposal chat
