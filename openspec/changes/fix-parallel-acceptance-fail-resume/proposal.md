---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
---

# Change: Fix parallel acceptance FAIL not returning to apply on resumed workspaces

**Change Type**: implementation

## Why
When a parallel workspace is resumed with `ResumeAction::Acceptance`, the `skip_apply` flag is set once and never cleared. If acceptance returns FAIL, the outer loop `continue`s back to the top, but `skip_apply` remains `true`, causing the next cycle to skip apply and run acceptance again—indefinitely. The intended behavior (apply → acceptance → fail → apply retry) is broken only for resumed workspaces.

## What Changes
- Make `skip_apply` mutable and reset it to `false` after the first cycle completes, so that any FAIL/Continue-exceeded/CommandFailed path correctly re-enters the apply step on the next iteration.

## Impact
- Affected specs: parallel-execution
- Affected code: `src/parallel/dispatch.rs` (the `skip_apply` variable and the apply+acceptance loop)
