# Proposal: write-readme

## Summary

Update README.md to reflect current features and create README.ja.md as a Japanese localization.

## Motivation

The current README.md has some gaps and outdated information:

1. **TUI is now the default** - When running without subcommand, TUI launches (not help message)
2. **`init` command undocumented** - New command with template support (claude, opencode, codex)
3. **Project structure outdated** - Missing `hooks.rs`, `task_parser.rs`, `templates.rs`
4. **State persistence path changed** - Now `.opencode/` (documented) vs actual implementation
5. **Japanese prompt in English README** - The dependency analysis section has Japanese text
6. **No Japanese version** - Project has Japanese-speaking users but only English docs

## Scope

### In Scope

- Update README.md with:
  - Correct default behavior (TUI mode)
  - Document `init` subcommand with all templates
  - Update project structure
  - Translate Japanese prompt examples to English
  - Add badges (Rust, License)
  - Clarify supported AI agents (Claude Code, OpenCode, Codex)

- Create README.ja.md with:
  - Full Japanese translation of README.md
  - Keep technical terms and code examples consistent

### Out of Scope

- Changes to code or configuration
- Additional documentation files beyond README
- Changelog generation

## Success Criteria

1. README.md accurately reflects all current features
2. README.ja.md is a complete Japanese translation
3. Both files are well-formatted and consistent with each other
4. No outdated or incorrect information remains
