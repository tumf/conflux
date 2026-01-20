# Change: Prefer XDG config paths for global configuration

## Why
macOS uses `~/Library/Application Support` for `dirs::config_dir()`, which ignores XDG conventions and leads to surprising behavior for developer-focused workflows. We want to prefer XDG-style paths (`$XDG_CONFIG_HOME` / `~/.config`) before platform-specific defaults so configuration behaves consistently across environments.

## What Changes
- Update global config discovery order to check XDG paths before platform defaults.
- Keep project-level `.cflx.jsonc` as the highest priority.
- Clarify the fallback order for global config when XDG is unset.

## Impact
- Affected specs: configuration
- Affected code: `src/config/mod.rs`, `src/config/defaults.rs` (global config helpers)
