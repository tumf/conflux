---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/CONSTITUTION.md
---

# Fix Archive Gate Verification Parsing

**Change Type**: implementation

## Problem / Context

Runtime log mining after `.last-checked` found repeated archive/acceptance failures where `cflx openspec validate <change> --archive-gate` rejected completed tasks even though their task lines visibly contained `(verification: ...)` notes. The reproduced current case is `add-s3-workspace-persistence`, where `tasks.md:21` contains a manual verification note with source paths and a runnable strict-validation command, but the archive gate still reports `Verification note should cite repository-verifiable evidence such as source paths, tests, or runnable commands`.

The current parser in `src/openspec_cmd.rs` extracts inline verification text with `(?i)\(verification:\s*([^)]*?)\)`. That stops at the first `)` inside the note, so task text containing commands such as `cflx openspec validate ... --strict` is parsed incompletely when the command itself is wrapped in backticks or parentheses nearby, and repository-evidence hints after the early close can be ignored. This creates false archive blockers and repeated apply/acceptance loops despite repository-verifiable evidence being present.

This is distinct from the existing `fix-dependency-target-handling` and `resume-archived-dirty-workspaces` changes: those address dependency classification and archived-dirty retry ownership, while this change is limited to OpenSpec task verification parsing and diagnostics.

## Proposed Solution

Make archive-gate task verification parsing robust for real task prose:

- Parse inline `(verification: ...)` spans without truncating at unrelated or nested parentheses inside quoted/backticked command text.
- Preserve existing accepted forms: inline verification before completion prose and standalone indented `verification:` continuation lines.
- Ensure repository-evidence detection evaluates the complete verification note before deciding that source paths, tests, or runnable commands are missing.
- Add regression coverage using observed manual verification wording with source paths and `cflx openspec validate <id> --strict` commands.
- Keep archive-gate strictness intact: tasks with genuinely missing verification, missing ownership markers, or no repository-verifiable evidence must still fail.

## Acceptance Criteria

- Archive-gate validation accepts a checked task whose inline verification note includes `manual`, source paths, and `cflx openspec validate <id> --strict` even when the surrounding task text contains additional completion prose.
- Archive-gate validation accepts a checked task whose inline verification note contains parenthesized or backticked command/prose segments without truncating the evidence-bearing portion.
- Archive-gate validation continues to reject behavior-changing tasks with no verification note.
- Archive-gate validation continues to warn/error on verification notes that lack ownership markers or repository-verifiable evidence.
- Existing archive-gate final-validation self-reference protection remains unchanged.

## Explicit Completion Conditions

Complete only when `src/openspec_cmd.rs` has parser and tests proving full inline verification extraction for evidence-bearing manual notes, current accepted verification formats remain accepted, invalid weak/missing verification formats remain rejected, and targeted Rust tests for the OpenSpec validator pass.

## Out of Scope

- Changing OpenSpec proposal/task syntax beyond making existing `(verification: ...)` parsing robust.
- Relaxing archive-gate evidence requirements.
- Changing dependency scheduling, merge/resolve retry semantics, or archived-dirty workspace recovery.
- Editing downstream product proposals such as `add-s3-workspace-persistence` or `add-e2b-workspace-sticky-runtime` directly.
