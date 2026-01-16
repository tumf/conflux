# Code Style and Conventions

## Rust Conventions
- Standard Rust naming: snake_case for functions/variables, PascalCase for types
- Use `thiserror` for error types
- Use `anyhow` for error propagation in application code
- Use `tracing` for logging

## Testing
- Unit tests in `#[cfg(test)]` modules within source files
- Integration tests in `tests/` directory
- Use `tempfile` crate for temporary test files/directories

## Documentation
- English for code comments and documentation
- Japanese for specifications and proposals (in `openspec/changes/`)

## Configuration
- JSONC format for config files (supports comments and trailing commas)
- Config file: `.cflx.jsonc`

## Project Organization
- Specifications in `openspec/specs/`
- Change proposals in `openspec/changes/`
