# OpenSpec Orchestrator - Project Overview

## Purpose
Automates the OpenSpec change workflow: list → dependency analysis → apply → archive.

## Tech Stack
- Language: Rust (Edition 2021)
- Framework: Tokio (async runtime)
- TUI: Ratatui + Crossterm
- CLI: Clap v4
- Serialization: Serde + Serde JSON

## Project Structure
```
src/
├── main.rs           # Entry point
├── cli.rs            # CLI argument parsing
├── config.rs         # Configuration file parsing (JSONC)
├── agent.rs          # Agent runner (configurable commands)
├── error.rs          # Error types
├── openspec.rs       # OpenSpec wrapper (list, archive)
├── opencode.rs       # OpenCode runner (legacy)
├── progress.rs       # Progress display
├── tui.rs            # Interactive TUI dashboard
├── hooks.rs          # Hook system
└── orchestrator.rs   # Main orchestration loop
```

## Key Features
- Automated OpenSpec change workflow
- LLM dependency analysis
- Real-time progress display
- TUI dashboard with selection mode
- Configurable agent commands (JSONC)
- Hook system for workflow customization

## Specifications
- `openspec/specs/cli/spec.md` - CLI and TUI specification
- `openspec/specs/configuration/spec.md` - Configuration specification
