## Implementation Tasks

- [x] Define the TUI config schema and loader for JSONC user preferences, including global candidate path resolution for `tui.jsonc` independent of `OrchestratorConfig::validate_required_commands()` (verification: unit - tests in `src/tui/config.rs` cover default config, `$XDG_CONFIG_HOME/cflx/tui.jsonc`, `~/.config/cflx/tui.jsonc`, and parse errors with actionable paths/messages).

- [x] Implement supported TUI key parsing and validation for MVP `keybindings.start` entries (`F1`-`F12`, named keys such as `Esc`, `Enter`, `Space`, `Tab`, `PageUp`, `PageDown`, `Home`, `End`, arrows, and single-character keys) (verification: unit - tests in `src/tui/config.rs` cover accepted keys, unknown names, empty arrays, duplicate entries, and unsupported modifier strings such as `Ctrl+R` if modifiers remain out of scope).

- [x] Store resolved TUI keybindings in TUI runtime state so rendering and input handling use the same resolved configuration for local and remote TUI sessions (verification: unit - tests in `src/tui/state.rs` or `src/tui/config.rs` confirm default `F5` and configured `F5/r` labels are available after initialization without reading project `.cflx.jsonc`).

- [x] Wire key event handling to trigger the existing start/resume/retry/continue behavior for any configured start key while preserving current `F5` semantics by default (verification: unit - tests in `src/tui/key_handlers.rs` prove `F5` works by default, configured `r` triggers the same `TuiCommand` path as `F5`, and unrelated keys remain ignored).

- [x] Replace hardcoded `F5` start-control labels in TUI footer/status/key hints with the resolved start key label (verification: unit - render tests in `src/tui/render.rs` assert default text includes `F5` and configured text includes `F5/r` in select, stopped, and stopping/status contexts).

- [x] Update CLI help and user-facing documentation to describe default keybindings and the TUI override file path `~/.config/cflx/tui.jsonc` without claiming dynamic `--help` rendering (verification: manual - run `cargo run -- tui --help` or inspect `src/cli.rs`, `README.md`, and `docs/guides/SERVER.md`/TUI docs to ensure help/docs mention default `F5` plus the TUI config path).

- [x] Add integration or focused command-level verification that `tui.jsonc` does not require orchestration command fields and that `.cflx.jsonc` does not override TUI keybindings (verification: integration - a temp HOME/XDG test in `src/tui/config.rs` or `tests/` loads only TUI config and separately confirms project config keybinding-like fields are ignored).

- [x] Run proposal and implementation quality gates (verification: integration - `cflx openspec validate add-user-tui-config --strict --evidence warn`, `cargo fmt --check`, and focused cargo tests such as `cargo test tui::config tui::key_handlers tui::render` pass).

## Future Work

- Extend `tui.jsonc` to additional actions such as stop, toggle selection, toggle logs, merge resolve, and worktree actions after the MVP start binding is stable.
- Add display/layout preferences such as compact footer or log hint toggles if real user demand emerges.
