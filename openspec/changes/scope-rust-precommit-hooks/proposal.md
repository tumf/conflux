---
change_type: implementation
priority: medium
dependencies: []
references:
  - .pre-commit-config.yaml
  - Makefile
  - openspec/specs/documentation/spec.md
verifications:
  - id: precommit-selection-tests
    requirement: Rust quality hooks run for staged Rust-impacting files and skip proposal-only commits
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: .pre-commit-config.yaml
    evidence: prek hook-selection output asserting Skipped for stable non-Rust files and Passed for a Rust source file
    rerun: prek run rustfmt --files README.md | grep -q Skipped && prek run clippy --files openspec/AGENTS.md | grep -q Skipped && prek run rustfmt --files src/main.rs | grep -q Passed && prek run clippy --files src/main.rs | grep -q Passed
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Scope Rust pre-commit hooks to Rust-impacting files

**Change Type**: implementation

## Premise / Context

- `rustfmt` and `clippy` currently declare `always_run: true` in `.pre-commit-config.yaml`.
- Both hooks ignore passed filenames and validate the whole Rust workspace when selected.
- Proposal-only commits cannot change Rust behavior but currently pay the full Rust hook cost.
- CI and explicit full-check commands remain available for repository-wide validation.

## Problem / Context

The commit-time hook selection is broader than the files that can affect Rust compilation. This slows proposal-only and documentation-only commits without adding meaningful verification. The hook commands themselves should remain comprehensive when Rust-impacting files are staged.

## Proposed Solution

Remove `always_run: true` from the `rustfmt` and `clippy` local hooks and add this shared staged-file selector:

```yaml
files: ^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$
```

Keep `pass_filenames: false`, `cargo fmt --all`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` unchanged. Keep generic hygiene and beads hooks unchanged. Document that commit-time selection is path-scoped while explicit `make check` or CI remains the full validation path.

## Acceptance Criteria

- A commit staging only `openspec/**` or Markdown does not select `rustfmt` or `clippy`.
- A staged Rust source/test file, `Cargo.toml`, `Cargo.lock`, or root `build.rs` selects both Rust hooks.
- Once selected, each hook still validates the full configured Rust scope rather than only passed files.
- Existing non-Rust hygiene and beads hooks retain their current selection behavior.
- Explicit full repository validation remains available and documented.

## Explicit Completion Conditions

- `.pre-commit-config.yaml` contains the exact shared `files` expression on both Rust hooks and no `always_run` field on either.
- Runnable hook-selection checks demonstrate skip and run cases without bypassing hook command failures.
- The YAML configuration validates and the selected Rust hooks pass on the current repository.

## Split Rationale

The OpenAPI CLI/static-artifact migration is tracked separately as `add-openapi-command`. Hook path selection does not depend on that migration and can be reviewed and implemented in parallel.

## Out of Scope

- Narrowing the actual `cargo fmt` or `cargo clippy` command scope.
- Skipping Rust hooks for `Cargo.toml`, `Cargo.lock`, or `build.rs` changes.
- Changing CI path filters or removing full repository checks.
- Changing non-Rust pre-commit hooks.
