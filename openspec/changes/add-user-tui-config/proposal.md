---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - src/config/mod.rs
  - src/config/load.rs
  - src/tui/key_handlers.rs
  - src/tui/render.rs
  - src/tui/runner.rs
---

# Add user-level TUI config

**Change Type**: implementation

## Problem/Context

TUI keybindings are currently hardcoded around `F5` for start/resume/retry/continue orchestration control. Some keyboards and terminal environments make function keys awkward or unavailable.

TUI preferences such as keybindings are user- and terminal-specific rather than project-specific. Putting them in `.cflx.jsonc` would make repository-portable orchestration configuration carry personal UI preferences and would make project-to-project switching more complex than the expected usage warrants.

The Conflux constitution allows external UI state/configuration only when it is non-authoritative for workflow control. TUI keybindings determine how a user triggers an existing command; they must not become authoritative workflow state or affect resume/archive/acceptance routing.

## Proposed Solution

Introduce a TUI-only user config file at:

- `$XDG_CONFIG_HOME/cflx/tui.jsonc` when `XDG_CONFIG_HOME` is set
- `~/.config/cflx/tui.jsonc` as the default XDG path
- `dirs::config_dir()/cflx/tui.jsonc` as the platform-default lower-priority candidate

The MVP scope is configurable app-level start control only:

```jsonc
{
  "keybindings": {
    "start": ["F5", "r"]
  }
}
```

`start` maps to the existing app-level orchestration control semantics:

- start selected work
- resume from stopped mode
- retry from error mode
- continue/cancel graceful stop while stopping

The local TUI client, including `cflx tui --server`, reads this user-level TUI config. `.cflx.jsonc` does not control TUI keybindings.

## Acceptance Criteria

- `cflx tui` loads user-level TUI preferences from `tui.jsonc` without requiring orchestration command fields in that file.
- When no `tui.jsonc` exists, current default behavior remains unchanged and `F5` continues to trigger start/resume/retry/continue.
- When `keybindings.start = ["F5", "r"]`, both `F5` and `r` trigger the existing start/resume/retry/continue behavior.
- The TUI footer, status titles, and relevant key hints render the configured start binding label, such as `F5/r`, instead of hardcoded `F5` text.
- Invalid, empty, or duplicate `keybindings.start` entries fail with actionable TUI config errors.
- Project `.cflx.jsonc` does not override TUI keybindings.
- `cflx tui --help` documents the default keybindings and points users to `~/.config/cflx/tui.jsonc` for overrides rather than trying to render dynamic config values.

## Explicit Completion Conditions

This proposal is complete when repository evidence shows:

- A TUI-only config loader exists and parses JSONC from the expected global candidate paths without invoking `OrchestratorConfig::validate_required_commands()`.
- Key parsing and validation are covered by unit tests for supported key names, unknown key names, empty `start`, and duplicate `start` entries.
- TUI key event handling uses the resolved start keybindings instead of matching only `KeyCode::F(5)`.
- TUI render tests prove configured labels appear in footer/status text and the default label remains `F5` when no config is present.
- CLI help text and documentation mention `~/.config/cflx/tui.jsonc` and the default `F5` binding.
- `cflx openspec validate add-user-tui-config --strict --evidence warn` passes.

## Out of Scope

- Project-level TUI keybinding overrides in `.cflx.jsonc`.
- Configurable bindings for stop, selection, merge resolve, log toggle, or worktree actions beyond the `start` app control.
- Modifier chords such as `Ctrl+R` unless already trivial to support safely.
- TUI themes, layout preferences, or display settings other than start key hint labels.
- Changing orchestration behavior, workflow routing, archive/acceptance decisions, or durable workflow state.
