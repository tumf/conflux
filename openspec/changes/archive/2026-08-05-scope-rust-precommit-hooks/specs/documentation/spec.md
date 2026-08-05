## ADDED Requirements

### Requirement: Path-scoped Rust commit hooks

The repository's commit-time `rustfmt` and `clippy` hooks MUST be selected only when staged paths can affect Rust formatting or compilation. Both hooks MUST use `^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$` as their staged-file selector, MUST NOT run unconditionally, and MUST continue to execute their full configured workspace commands once selected. Non-Rust hygiene hooks and explicit full repository validation MUST remain available.

#### Scenario: Proposal-only commit skips Rust hooks

**Given**: the staged paths contain only files below `openspec/changes/`
**When**: commit-time hooks are selected
**Then**: `rustfmt` and `clippy` are skipped
**And**: applicable generic hygiene and beads hooks retain their configured behavior

#### Scenario: Rust source selects full Rust hooks

**Given**: a staged path matches `src/**/*.rs` or `tests/**/*.rs`
**When**: commit-time hooks are selected
**Then**: both `rustfmt` and `clippy` run
**And**: they execute their full configured commands without receiving staged filenames

#### Scenario: Rust build metadata selects full Rust hooks

**Given**: `Cargo.toml`, `Cargo.lock`, or root `build.rs` is staged
**When**: commit-time hooks are selected
**Then**: both `rustfmt` and `clippy` run

#### Scenario: Full validation remains explicit

**Given**: a developer needs repository-wide validation regardless of staged paths
**When**: the documented full-check command is run
**Then**: formatting, linting, tests, hooks, and the other configured full checks execute independently of commit-time path selection
