# Contributing

Thanks for contributing to Conflux.

## Development Setup

Prerequisites:

- Rust 1.70 or later
- Cargo
- `prek` for Git hooks (recommended)

Build and test locally:

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Format, lint, and test
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Useful extras:

```bash
# Coverage
cargo llvm-cov --all-features

# Run with debug logging
RUST_LOG=debug cargo run -- run
```

## Git Hooks

This project uses [prek](https://prek.j178.dev/) for Git hooks.

If you previously used `pre-commit`, uninstall it first:

```bash
pre-commit uninstall
```

Install and enable hooks:

```bash
brew install prek
prek install
```

Common commands:

```bash
# Run all hooks on all files
prek run --all-files

# Run selected hooks (--all-files, because rustfmt and clippy are path-scoped)
prek run rustfmt clippy --all-files

# List available hooks
prek list
```

Hook configuration lives in `.pre-commit-config.yaml`. Running `prek run --all-files` also runs `make check-openapi`, which fails if `docs/openapi.yaml` no longer matches the generated contract. See [The API contract](#the-api-contract).

### Path-scoped Rust hooks vs. full validation

The `rustfmt` and `clippy` hooks are **path-scoped at commit time**. They are
selected only when the staged paths can affect Rust compilation:

```
^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$
```

A proposal-only or docs-only commit — anything under `openspec/`, Markdown,
`web/`, `skills/` — does not select them, so it does not pay the full Rust hook
cost. Only *selection* is narrowed: as soon as one Rust-impacting file is
staged, both hooks still run `cargo fmt --all` and
`cargo clippy --locked --all-targets --all-features -- -D warnings` over the
whole workspace, not just the staged files.

Because selection is path-based, `prek run rustfmt` with no file arguments
checks nothing. Pass `--all-files` for an explicit manual run.

Commit-time scoping never replaces full validation. `make check`
(`fmt` + `lint` + `test` + hooks + `audit`) and CI validate the whole
repository regardless of which paths changed.

## The API contract

`docs/openapi.yaml` is the single canonical description of `/api/v2`, and it is
generated — never hand-edited. There is no second OpenAPI file in the
repository, because a duplicate cannot be kept honest and a stale one is worse
than none: a generated client will believe it.

| Command | Effect |
| --- | --- |
| `make openapi` | Regenerate `docs/openapi.yaml` from `src/web/openapi.rs`. |
| `make check-openapi` | Fail on any drift. Generates to a temporary file and never writes to your working tree. |

`make check-openapi` also runs `tests/openapi_contract_tests.rs`, which holds the
tracked artifact against the running router: every published path must be bound
and enforce the authentication it declares, the command union must match the
advertised command set, the error and event vocabularies must be complete, and
real serialized DTOs must satisfy the published schemas.

So: change a route, DTO field, command variant, error code, or security
declaration, then run `make openapi` and commit the regenerated artifact with the
code. A running instance serves the same bytes at `GET /api/v2/openapi.yaml`.

## Project Structure

High-level layout:

```text
src/
  main.rs            # CLI entry point
  cli.rs             # Command-line parsing
  orchestrator.rs    # Main orchestration loop
  agent/             # AI agent execution
  config/            # Configuration loading and defaults
  execution/         # Apply/archive execution logic
  orchestration/     # Shared orchestration steps
  parallel/          # Parallel execution and workspaces
  remote/            # Remote server client
  server/            # Multi-project server daemon
  tui/               # Terminal UI
  vcs/               # VCS abstraction and git backend
  web/               # Web monitoring
tests/               # Integration and end-to-end tests
```

For a broader walkthrough, see `docs/guides/DEVELOPMENT.md`.

## Contribution Notes

- Keep user-facing usage and product overview in `README.md`.
- Put contributor workflow, build/test steps, and repository internals in `CONTRIBUTING.md`.
- When adding CLI features, update both `README.md` and `README.ja.md` if the user-facing behavior changes.
- When changing release or API behavior, update the relevant docs under `docs/`.
