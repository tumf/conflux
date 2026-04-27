---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - src/cli.rs
  - src/main.rs
  - skills/cflx-proposal/SKILL.md
  - skills/cflx-proposal/scripts/cflx.py
  - skills/tests/test_cflx_proposal_change_types.py
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/specs/cli/spec.md
---

# Change: retire cflx.py validator

**Change Type**: implementation

## Problem / Context

Conflux already exposes `cflx openspec list/show/validate/archive` as the native OpenSpec utility surface, and bundled skill installation no longer ships `scripts/cflx.py`. However, the repository still retains `skills/cflx-proposal/scripts/cflx.py` as a development-only validator implementation and as the import target for Python regression tests.

That split leaves the proposal validation contract duplicated between Rust and Python. The duplication became more visible after tightening behavior-centric proposal validation guardrails: the new warning paths were added to `cflx.py`, but the native validator remains the real runtime surface that proposal authors and future automation should trust.

As long as `cflx.py` remains the easiest place to add validator behavior, proposal validation can drift between the helper and the product. This makes it harder to guarantee that proposal/apply/acceptance all enforce the same “provably delivered behavior” standard and harder to prove that `cflx.py` is actually obsolete.

## Proposed Solution

- Port the remaining proposal-validator responsibilities from `skills/cflx-proposal/scripts/cflx.py` into the native Rust implementation behind `cflx openspec validate`.
- Extend the native validator so behavior-changing proposals receive the same behavior-centric warnings now expected by the proposal workflow: verification ownership, artifact-heavy task mixes, missing runnable verification, executable surface without runnable checks, and runtime-behavior claims without implementation-facing tasks.
- Move validator regression coverage from the Python test harness into Rust tests so native CLI behavior is the sole source of truth.
- Update canonical specs and active skill-facing docs to describe the native CLI as the only supported validator surface.
- Delete `skills/cflx-proposal/scripts/cflx.py` only after native validator behavior, tests, and specs prove it has no remaining role.

## Acceptance Criteria

- `cflx openspec validate <change-id> --strict --evidence warn|error` emits behavior-centric validation findings for proposal tasks that rely on artifacts alone, omit verification ownership, omit runnable verification for executable surfaces, or claim runtime behavior without implementation-facing tasks.
- Native Rust tests cover the validation cases that previously depended on importing `OpenSpecManager` from `skills/cflx-proposal/scripts/cflx.py`, so the repository no longer depends on Python-only validator tests for proposal validation behavior.
- Active canonical specs and skill-facing docs describe `cflx openspec ...` as the validator/list/show/archive surface and no longer present `cflx.py` as an executable contract.
- `skills/cflx-proposal/scripts/cflx.py` is removed, and repository quality gates still prove that proposal validation, skill embedding/distribution, and representative `cflx openspec validate` behavior work without it.

## Explicit Completion Conditions

- Rust validation helpers under `src/openspec_cmd.rs` (and any adjacent native CLI modules) contain the behavior-centric warning logic currently expected by proposal guidance.
- Rust tests fail if a future change reintroduces the old gap: native validator missing behavior-centric warnings, missing ownership parsing, or missing executable-surface runnable checks.
- Repo search over active sources/specs/docs no longer finds executable instructions that rely on `skills/cflx-proposal/scripts/cflx.py`.
- The repo no longer contains `skills/cflx-proposal/scripts/cflx.py`, and validation/test commands named in `tasks.md` succeed afterward.

## Out of Scope

- Rewriting archived historical proposals or tasks that mention `cflx.py`.
- Changing proposal/apply/acceptance orchestration flow outside the validator-surface unification required to retire `cflx.py`.
