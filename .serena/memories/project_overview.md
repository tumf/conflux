# OpenSpec Orchestrator - Project Overview

## Purpose
Automates the OpenSpec change workflow: list → dependency analysis → apply → archive.

## Tech Stack
- Language: Rust (Edition 2021)
- Framework: Tokio (async runtime)
- TUI: Ratatui + Crossterm
- CLI: Clap v4
- Serialization: Serde + Serde JSON
- Hashing: md5

## Project Structure
```
src/
├── main.rs           # Entry point
├── cli.rs            # CLI argument parsing (run, tui, init, approve)
├── config.rs         # Configuration file parsing (JSONC)
├── agent.rs          # Agent runner (configurable commands)
├── approval.rs       # Approval workflow (checksum validation)
├── history.rs        # Apply attempt history tracking
├── hooks.rs          # Lifecycle hooks system
├── templates.rs      # Configuration templates (claude, opencode, codex)
├── task_parser.rs    # Task file parsing and progress calculation
├── error.rs          # Error types (OrchestratorError)
├── openspec.rs       # OpenSpec wrapper (list, archive)
├── opencode.rs       # OpenCode runner (legacy)
├── progress.rs       # Progress display (indicatif)
├── tui.rs            # Interactive TUI dashboard
├── jj_workspace.rs   # Parallel execution with jj workspaces
└── orchestrator.rs   # Main orchestration loop
```

## Key Features
- Automated OpenSpec change workflow
- LLM dependency analysis
- Real-time progress display
- TUI dashboard with selection mode
- Configurable agent commands (JSONC)
- Lifecycle hooks for workflow customization
- Approval workflow with checksum validation
- Parallel execution using jj workspaces
- Multiple change processing (comma-separated)

## CLI Commands
- `run` - Non-interactive orchestration loop
- `tui` - Interactive TUI dashboard
- `init` - Initialize configuration file
- `approve set|unset|status` - Manage change approval

## Specifications
- `openspec/specs/cli/spec.md` - CLI and TUI specification
- `openspec/specs/configuration/spec.md` - Configuration specification
