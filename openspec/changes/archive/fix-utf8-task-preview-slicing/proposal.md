---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - https://github.com/tumf/conflux/issues/5
---

# Proposal: Fix UTF-8-unsafe task preview slicing in `cflx openspec validate`

**Change Type**: implementation

## Summary

Prevent `cflx openspec validate --strict` from panicking when a tasks.md bare-task warning includes multi-byte UTF-8 characters such as `§`.

## Problem

The native OpenSpec validator currently builds the "Possible task without checkbox" preview by slicing the trimmed task line with a byte-count cap. When the 50-byte cutoff lands inside a multi-byte UTF-8 code point, Rust panics before validation can report structured errors or warnings.

## Solution

Replace the byte-based preview slice in `validate_tasks_content` with the existing char-safe display truncation helper so the validator always renders a bounded preview on valid UTF-8 input. Add regression coverage for bare-task validation containing multi-byte characters near the preview boundary.

## Acceptance Criteria

- `cflx openspec validate <change-id> --strict` does not panic when `tasks.md` contains a bare task line with multi-byte UTF-8 characters.
- The validator still reports `Possible task without checkbox` for qualifying lines after the fix.
- Preview text remains truncated for long bare-task lines, but truncation is computed on character boundaries.
- Regression tests cover a bare-task line whose 50-byte boundary would otherwise split a multi-byte character.

## Explicit Completion Conditions

- `src/openspec_cmd.rs` no longer slices bare-task preview text by raw byte offset inside `validate_tasks_content`.
- Automated test coverage demonstrates the multi-byte regression case and keeps the warning behavior intact.
- `cflx openspec validate fix-utf8-task-preview-slicing --strict` passes for this proposal.

## Out of Scope

- Changing any other validator messaging or preview lengths.
- Reworking unrelated UTF-8 handling outside the bare-task preview path.
