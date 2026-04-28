---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/changes/use-dated-archive-names/tasks.md
---

# Proposal: Fix standalone verification-line validation panic

**Change Type**: implementation

## Summary

Make `cflx openspec validate --strict --evidence ...` handle standalone `verification:` continuation lines without panicking on UTF-8 content, and align native validation with the task authoring pattern already used in active proposals.

## Problem

The native validator in `src/openspec_cmd.rs` only recognizes inline `(verification: ...)` notes embedded inside a checkbox task line. Active proposal authoring in this repository also uses indented continuation lines such as `verification: unit - ...` under a checkbox task. When the validator encounters that standalone form, it can misclassify the line as a bare task and attempt to render a truncated preview. If the preview cutoff lands inside a multi-byte character, `cflx openspec validate` panics with a char-boundary error instead of returning a structured validation finding.

This makes proposal validation fragile for valid UTF-8 task files and breaks the current repository authoring workflow for completion-condition and standalone verification blocks.

## Solution

Teach `validate_tasks_content` to treat indented `verification:` continuation lines as verification metadata attached to the immediately preceding checkbox task, rather than as independent bare-task content. Update the validator so continuation-line parsing and any fallback preview rendering remain UTF-8 safe. Add regression coverage for both the standalone-verification parsing path and the multi-byte panic case seen in Japanese task text.

## Acceptance Criteria

- `cflx openspec validate <change-id> --strict --evidence warn` does not panic when a checkbox task is followed by an indented standalone `verification:` line containing multi-byte UTF-8 text.
- The native validator accepts repository task authoring that uses checkbox task lines followed by `Completion condition:` and `verification:` continuation lines.
- Standalone `verification:` continuation lines contribute to behavior-task evidence and ownership checks the same way inline `(verification: ...)` notes do.
- Lines that are still invalid task content produce structured validation findings without UTF-8 boundary panics.
- Regression tests cover the exact failure mode where a standalone `verification:` preview would otherwise split a multi-byte character.

## Explicit Completion Conditions

- `src/openspec_cmd.rs` recognizes indented `verification:` continuation lines in `validate_tasks_content` and does not treat them as bare tasks.
- Native validator tests demonstrate both accepted standalone-verification parsing and UTF-8-safe handling of invalid preview text.
- `cflx openspec validate fix-verification-line-validation-panic --strict --evidence warn` passes for this proposal.

## Out of Scope

- Redesigning the entire proposal task grammar beyond standalone `Completion condition:` / `verification:` continuation support.
- Changing unrelated proposal validation heuristics outside task parsing and UTF-8-safe reporting.
