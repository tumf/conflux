## Implementation Tasks

- [ ] Delete `src/server/acp_client.rs` (verification: `ls src/server/acp_client.rs` fails)
- [ ] Remove `pub mod acp_client;` from `src/server/mod.rs` (verification: `cargo build` succeeds without the module)
- [ ] Remove all `use crate::server::acp_client::*` imports and ACP-specific type references from `src/server/api.rs` and `src/server/proposal_session.rs` (verification: `rg "acp_client" src/server/` returns no matches)
- [ ] Replace `ProposalSessionConfig` fields in `src/config/types.rs`: remove `acp_command`, `acp_args`, `acp_env` and add `opencode_command`, `opencode_model`, `opencode_agent` with sane defaults (verification: `cargo test` config parsing tests pass with the new field names)
- [ ] Delete `tests/fixtures/mock_acp_agent.py` (verification: `ls tests/fixtures/mock_acp_agent.py` fails)
- [ ] Run `cargo fmt && cargo clippy -- -D warnings && cargo test` (verification: all pass with zero ACP-related warnings)
- [ ] Confirm `rg "AcpClient|acp_command|acp_args|AcpEvent|AcpMessage" src/ tests/` returns no matches except documentation or comments (verification: grep output is empty or doc-only)

## Future Work

- Add `OPENCODE_SERVER_PASSWORD` config support for authenticated `opencode serve`
