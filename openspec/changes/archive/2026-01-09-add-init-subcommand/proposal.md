# Proposal: add-init-subcommand

## Summary

Add an `init` subcommand that generates a `.cflx.jsonc` configuration template file. The command supports a `--template` flag to select from predefined agent configurations (opencode, claude, codex), with claude as the default.

## Motivation

Currently, users must manually create the configuration file by copying examples or reading documentation. An `init` command streamlines the setup process and ensures users start with a working configuration tailored to their preferred AI agent.

## Scope

- Add `init` subcommand to CLI
- Support `--template` flag with options: `opencode`, `claude`, `codex` (default: `claude`)
- Generate `.cflx.jsonc` with appropriate commands and hooks
- Handle existing file (prompt for overwrite or error)

## Out of Scope

- Interactive configuration wizard
- Validation of generated config
- Auto-detection of installed agents
