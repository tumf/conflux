## 1. Implementation
- [x] 1.1 Update config loading order to prefer XDG paths before platform defaults (verify in `src/config/mod.rs` by checking the search order in `OrchestratorConfig::load`).
- [x] 1.2 Add helper for resolving XDG config path with `$XDG_CONFIG_HOME` and fallback to `~/.config` (verify with unit test coverage in `src/config/mod.rs`).
- [x] 1.3 Update unit tests to cover XDG precedence and platform fallback behavior (verify with `cargo test config::tests::test_load_xdg_config_precedence`).

## 2. Validation
- [x] 2.1 Run `cargo test` (verify all tests pass).
- [x] 2.2 Run `cargo fmt --check` (verify no formatting issues).
