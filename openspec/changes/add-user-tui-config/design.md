# Design: User-level TUI config

## Premise

Conflux currently uses `.cflx.jsonc` and global `config.jsonc` for orchestration behavior. TUI keybindings are different: they are personal UI preferences tied to a user's keyboard, terminal, and muscle memory. They should not be repository-portable workflow configuration.

## Configuration boundary

Introduce a dedicated `TuiConfig` surface rather than adding TUI fields to `OrchestratorConfig`.

Recommended module boundary:

- `src/tui/config.rs` owns `TuiConfig`, `TuiKeybindingsConfig`, key parsing, validation, and TUI config loading.
- `src/config/jsonc.rs` remains the shared JSONC parser.
- `src/config/mod.rs` may expose a reusable path helper for alternate filenames, but `tui.jsonc` must not participate in `OrchestratorConfig::load`.

This keeps orchestration validation independent from UI preferences. `tui.jsonc` must not require `apply_command`, `archive_command`, `analyze_command`, `acceptance_command`, or `resolve_command`.

## Path priority

Use the same low-to-high global path concept as `config.jsonc`, but with filename `tui.jsonc`:

1. `dirs::config_dir()/cflx/tui.jsonc`
2. `~/.config/cflx/tui.jsonc`
3. `$XDG_CONFIG_HOME/cflx/tui.jsonc`

If duplicate paths resolve to the same file, loading may de-duplicate them to avoid double merging, but behavior should still be equivalent to higher-priority values winning.

No project `.cflx.jsonc` file participates in TUI config resolution.

## Runtime state

Resolve TUI keybindings once when the TUI starts and store the result in TUI runtime state, such as `AppState`, so input handling and rendering use the same source of truth.

This applies to both local TUI and remote TUI. In remote mode, keybindings are client-side preferences because the keyboard input happens in the local client.

## Start action semantics

The MVP config action is named `start` for user-facing simplicity, but it maps to the existing app-level control currently triggered by `F5`:

- start selected work
- resume stopped processing
- retry error changes
- continue/cancel graceful stop while stopping

Implementation may rename internal helpers from `handle_f5_key` to a start-control name, but the behavior must remain cursor-independent. In particular, start control must not become a cursor-local merge resolve action.

## Validation

Validation should fail before entering the TUI when `tui.jsonc` is malformed or semantically invalid. Error messages should name the config path and field.

MVP validation:

- `keybindings.start` defaults to `["F5"]` when absent.
- `keybindings.start` must not be empty when present.
- duplicate entries within `start` are rejected after normalization.
- unknown keys are rejected.
- unsupported modifier syntax is rejected unless fully implemented and tested.

## Display labels

Rendering should use a stable display label generated from the resolved keys:

- `[F5]` -> `F5`
- `[F5, r]` -> `F5/r`
- `[Space, r]` -> `Space/r`

Footer/status text should vary the verb by mode while reusing the same configured label:

- select/runnable: `<keys>: run` or `Press <keys> to start processing`
- stopped: `<keys>: resume`
- stopping: `<keys>: continue`
- error: `<keys>: retry`

## Constitution alignment

`tui.jsonc` is UI preference state. It must not influence workflow-control routing, acceptance gating, archive routing, or next-action decisions. It only maps user key input to existing TUI commands.
