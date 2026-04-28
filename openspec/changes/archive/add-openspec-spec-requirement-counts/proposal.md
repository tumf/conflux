---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cli/spec.md
---

# Proposal: Add Requirement Counts to `cflx openspec list --specs`

**Change Type**: implementation

## Summary

Extend the human-readable `cflx openspec list --specs` output so each canonical spec shows how many requirements it contains.

## Problem

The current spec list only shows each spec name and path. When reviewing the canonical spec set, users cannot quickly tell which specs are broad versus narrow without opening each `spec.md` file.

## Solution

Count canonical requirements in each `openspec/specs/<name>/spec.md` file using the existing requirement heading convention (`### Requirement:`), store that count in the spec listing model, and render it in the `cflx openspec list --specs` human-readable output.

## Acceptance Criteria

- `cflx openspec list --specs` shows `Requirements: <n>` for every listed canonical spec.
- The requirement count is derived from the number of `### Requirement:` headings in the corresponding canonical `spec.md`.
- Specs with zero requirement headings are still listed and render `Requirements: 0`.
- The existing sort order and path output remain unchanged.

## Out of Scope

- Adding JSON output to `cflx openspec list`
- Changing how requirement sections are authored in canonical specs
- Showing scenario counts or any other spec metrics
