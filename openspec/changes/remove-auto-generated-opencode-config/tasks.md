## Implementation Tasks

- [x] Remove `OPENCODE_PROPOSAL_CONFIG_FILENAME` and `OPENCODE_PROPOSAL_CONFIG_CONTENT` constants from `src/server/proposal_session.rs` (verification: `src/server/proposal_session.rs` no longer contains `OPENCODE_PROPOSAL_CONFIG` identifiers)
- [x] Remove `proposal_session_data_dir()` helper from `src/server/proposal_session.rs` (verification: function no longer exists)
- [x] Remove `ensure_default_opencode_proposal_config()` from `src/server/proposal_session.rs` (verification: function no longer exists)
- [x] Remove `inject_default_opencode_config_if_missing()` from `src/server/proposal_session.rs` (verification: function no longer exists)
- [x] Remove the call to `inject_default_opencode_config_if_missing()` in `create_session()` (around L211-216) and its associated logging (verification: `create_session` no longer references inject)
- [x] Update `tests/e2e_proposal_session.rs` to remove references to `opencode-proposal.jsonc` default config injection (verification: `cargo test --test e2e_proposal_session` passes)
- [x] Add proposal session customization section to README.md documenting `proposal_session.transport_env.OPENCODE_CONFIG` (verification: section exists in README.md)
- [x] Run `cargo clippy -- -D warnings` and `cargo fmt --check` (verification: no errors)

## Future Work

- Clean up any existing auto-generated `opencode-proposal.jsonc` files on user machines (manual)
