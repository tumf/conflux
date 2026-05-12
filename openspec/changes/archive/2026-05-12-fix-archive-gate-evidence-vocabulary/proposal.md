---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - skills/cflx-proposal/SKILL.md
  - openspec/CONSTITUTION.md
---

# Fix Archive Gate Evidence Vocabulary

**Change Type**: implementation

## Problem / Context

Runtime log mining found repeated archive-gate failures where completed task verification notes were reported as lacking repository-verifiable evidence even after agents rewrote the notes to mention generic evidence fields such as source paths, runnable commands, or project build artifacts.

The validator currently recognizes a finite list of concrete hints such as `src/`, `tests/`, `cargo test`, `npm run`, and file extensions. That keeps weak narrative notes rejected, but it does not cover the vocabulary already recommended by bundled proposal guidance and validator diagnostics: `source paths`, `runnable command`, and common repository artifacts/commands such as `Dockerfile`, `.toml`, and `docker build`. As a result, agents can follow the diagnostic wording and still remain blocked by `cflx openspec validate <id> --archive-gate`.

This proposal is limited to generic Conflux validator behavior and MUST NOT encode any project-specific log content, paths, product names, or private change IDs.

## Proposed Solution

Expand and test the native archive-gate evidence vocabulary so the validator accepts generic repository-verifiable evidence wording that Conflux itself recommends, while preserving strict rejection for weak or ownership-free verification notes.

- Add evidence hints for diagnostic/guidance vocabulary such as `source path`, `source paths`, `test file`, `test files`, `runnable command`, and `runnable commands`.
- Add hints for common repository artifacts and runnable commands that are not currently covered, such as `Dockerfile`, `.toml`, and `docker build`.
- Add regression tests covering verification notes that use these generic evidence phrases and artifact/command forms.
- Keep ownership-marker enforcement unchanged.
- Keep weak notes such as `manual review` rejected.

## Acceptance Criteria

- `cflx openspec validate <id> --archive-gate` accepts implementation tasks whose verification notes include a valid ownership marker plus generic repository-evidence wording that the diagnostic/guidance text recommends.
- The validator accepts verification notes citing common repository build artifacts or build commands that are concrete and repository-verifiable.
- The validator continues to reject verification notes with no ownership marker or no repository-verifiable evidence.
- The bundled proposal guidance and validator behavior no longer contradict each other for generic evidence vocabulary.

## Explicit Completion Conditions

Complete only when `src/openspec_cmd.rs` evidence matching and tests prove the new vocabulary is accepted, existing weak-verification rejection tests still pass, the canonical `cflx-proposal-validation` spec delta documents the behavior, and focused OpenSpec validator tests pass.

## Out of Scope

- Relaxing archive-gate validation to accept narrative-only verification.
- Changing archive, acceptance, merge, or resolve scheduling semantics.
- Encoding any private runtime log content or downstream project identifiers in repository files.
- Editing active downstream project proposals.
