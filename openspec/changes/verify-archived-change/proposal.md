---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd/verify.rs
  - src/archive_layout.rs
  - src/openspec_cmd.rs
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/cli/spec.md
verifications:
  - id: archived-change-verification-resolution
    requirement: Runtime-supervised verification resolves one change from its active proposal or its canonical archive entry without weakening archive identity checks
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused Rust tests cover active precedence, dated and direct archive entries, archive-only execution, invalid nested layouts, ambiguous archives, and missing changes
    rerun: cargo test --lib openspec_cmd::verify
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Resolve archived changes in `cflx openspec verify`

**Change Type**: implementation

## Problem / Context

`cflx openspec verify <change-id>` resolves declarations only from `openspec/changes/<change-id>/proposal.md`. Native archive moves that proposal to `openspec/changes/archive/YYYY-MM-DD-<change-id>/proposal.md`, so the same logical verification ID becomes unreadable after successful archive even though the declaration and repository-level automation remain valid.

Conflux already owns canonical archive-entry recognition in `src/archive_layout.rs`. The verify path bypasses it and therefore disagrees with list/show, archive completion, task evidence, and dependency resolution about where one change may live.

## Proposed Solution

Introduce one shared proposal-location resolver for verification declarations:

- use the active proposal when the active change exists;
- otherwise resolve the exact direct or dated archive entry through the existing archive-layout rules;
- reject invalid nested layouts, suffix collisions, multiple valid archive candidates, missing proposal identities, and active/archive ambiguity rather than guessing;
- keep verification IDs and repository-relative `automation`, `evidence`, and `rerun` declarations unchanged across archive;
- run declared repository-local verification from the repository root, not from the archived directory.

Reuse or extend the existing archive-layout API instead of adding an independent archive-name parser in `verify.rs`.

## Acceptance Criteria

- `cflx openspec verify <change-id>` reads declarations from an active proposal exactly as before.
- When no active proposal exists, the command reads equivalent declarations from the sole valid direct or dated archive entry.
- An archived declaration with repository-level automation can be planned, rerun, and captured using the same verification ID as before archive.
- Active state has explicit precedence only when no conflicting valid archived identity exists; simultaneous active and archived identities fail closed with an actionable diagnostic.
- Multiple valid archive entries for the same change fail closed instead of depending on filesystem iteration order.
- Nested date directories, malformed dates, suffix collisions, missing `proposal.md`, and unrelated archive entries never satisfy resolution.
- Existing evidence binding, dirty-tree rejection, command supervision, and exit-status semantics remain unchanged.
- Focused tests exercise active, direct archive, dated archive, archive-only execution, active/archive ambiguity, duplicate archives, invalid nested layout, and missing change behavior.

## Explicit Completion Conditions

- `verify.rs` no longer constructs an active-only proposal path as its complete change-resolution policy.
- Active and archived verification declaration loading uses one archive identity contract shared with existing archive-aware code.
- `cargo test --lib openspec_cmd::verify` passes.
- `cflx openspec validate verify-archived-change --strict --evidence error` passes.
- `cflx openspec validate verify-archived-change --archive-gate` passes before archive.

## Out of Scope

- Rewriting physical paths inside archived proposal frontmatter.
- Allowing automation inside a moving change-local subtree.
- Changing evidence sidecar schema, reuse policy, or command supervision.
- Repairing malformed or duplicated archive directories automatically.
- Changing archive naming or migration policy.
