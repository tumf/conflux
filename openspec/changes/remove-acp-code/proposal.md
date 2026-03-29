# Change: Remove ACP code and migrate config to OpenCode Server

**Change Type**: implementation

## Why

After transport and UI state refactors, all ACP-specific code is dead. Removing it reduces maintenance burden and completes the migration.

## What Changes

- Delete `src/server/acp_client.rs`
- Remove ACP imports and types from `src/server/api.rs` and `src/server/proposal_session.rs`
- Replace `ProposalSessionConfig.acp_command / acp_args / acp_env` with `opencode_command / opencode_model / opencode_agent` in `src/config/types.rs`
- Remove `tests/fixtures/mock_acp_agent.py`
- Update `openspec/specs/proposal-session-backend/spec.md` to remove ACP references

## Impact

- Affected specs: `proposal-session-backend`
- Affected code: `src/server/acp_client.rs` (deleted), `src/server/mod.rs`, `src/config/types.rs`, `tests/fixtures/mock_acp_agent.py` (deleted)
- **BREAKING**: `acp_command`, `acp_args`, `acp_env` config keys no longer recognized

## Acceptance Criteria

1. `rg "AcpClient|acp_client|acp_command|acp_args" src/ tests/` returns no matches
2. `cargo build && cargo test` passes
3. Config with old `acp_*` keys logs a deprecation warning or is silently ignored
