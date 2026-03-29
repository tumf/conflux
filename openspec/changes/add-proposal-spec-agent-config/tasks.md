## Implementation Tasks

- [ ] Add `opencode_config_path: Option<String>` field to `ProposalSessionConfig` in `src/config/types.rs` with serde default of `None` (verification: `cargo build`)
- [ ] In `src/server/proposal_session.rs::create_session`, inject `OPENCODE_CONFIG` into the ACP subprocess environment: if `opencode_config_path` is set use that path, otherwise auto-generate a default `opencode-proposal.jsonc` in the server data dir; skip injection if `transport_env` already contains `OPENCODE_CONFIG` (verification: `cargo test` passes, and a unit test confirms the env var is set)
- [ ] Create the default `opencode-proposal.jsonc` content as a const or embedded resource with `{ "$schema": "https://opencode.ai/config.json", "mode": "spec" }` and write it to disk on first use (verification: `cargo test` unit test confirms file creation and correct content)
- [ ] Add config test in `src/config/mod.rs` verifying `opencode_config_path` deserialization from `.cflx.jsonc` (verification: `cargo test config::`)
- [ ] Update `tests/e2e_proposal_session.rs` to verify the ACP subprocess receives `OPENCODE_CONFIG` in its environment (verification: `cargo test --test e2e_proposal_session`)
- [ ] Run full verification (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) and fix any regressions (verification: all three exit 0)

## Future Work

- Manually verify against a real `opencode acp` binary that the spec agent is activated in proposal chat
