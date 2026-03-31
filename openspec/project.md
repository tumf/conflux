# Project Context

## Purpose
Conflux automates the OpenSpec change workflow: list → dependency analysis → apply (implementation phase) → acceptance → archive → resolve (when needed) → merged. It enables AI agents to process changes automatically with progress tracking and lifecycle hooks.

## Tech Stack
- **Language**: Rust (Edition 2021)
- **Async Runtime**: Tokio
- **TUI Framework**: Ratatui + Crossterm
- **CLI Parser**: Clap v4
- **Serialization**: Serde + Serde JSON
- **Hashing**: md5 crate

## Project Conventions

### Code Style
- Follow Rust standard formatting (`cargo fmt`)
- Use Clippy for linting (`cargo clippy -- -D warnings`)
- Prefer `Result<T>` with custom `OrchestratorError` for error handling
- Use `tracing` for logging (debug, info, warn, error levels)

### Architecture Patterns
- **Module-based organization**: Each concern in its own module
- **Configuration-driven**: JSONC config files for customization
- **Event-driven hooks**: Lifecycle hooks for workflow customization
- **Async-first**: Use `async/await` for I/O operations

### Testing Strategy
- Unit tests in module files (`#[cfg(test)]`)
- Use `tempfile` for filesystem tests
- Run with `cargo test`
- Coverage: `cargo llvm-cov`

### Git Workflow
- Feature branches for development
- Conventional commits (feat:, fix:, docs:, etc.)
- Run `cargo fmt && cargo clippy && cargo test` before commit

## Domain Context
- **OpenSpec**: A specification management system for AI-assisted development
- **Orchestration**: Top-level runtime that manages one or more Projects and the end-to-end workflow lifecycle
- **Project**: A scoped execution unit backed by one `OrchestratorState`, containing a set of Changes processed together in Serial or Parallel mode
- **Change**: A unit of work defined in `openspec/changes/{change_id}/` and executed within exactly one Project
- **Task**: Individual items within a change's `tasks.md` file
- **Agent**: AI tool (Claude Code, OpenCode, Codex) that processes changes

The system hierarchy is `Orchestration 1--* Project 1--* Change`.

## Important Constraints
- Support multiple AI agents through configurable commands
- Maintain backward compatibility with existing configurations
- Handle large numbers of changes efficiently
- Support both interactive (TUI) and non-interactive (run) modes

## External Dependencies
- **OpenSpec CLI**: `npx @fission-ai/openspec@latest` (configurable)
- **jj (Jujutsu)**: Optional, for parallel execution with workspaces
- **AI Agents**: Claude Code, OpenCode, or Codex (configurable)
