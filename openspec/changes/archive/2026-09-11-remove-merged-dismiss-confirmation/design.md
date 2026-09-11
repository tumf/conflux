# Design

## Authority and scope

The production Changes view at Git revision `ecddffc1191cbf8069f5b4eab6cdf5935196333d` remains authoritative for row layout, selection, colors, panel titles, and hints. This correction removes the merged-dismissal overlay and introduces no replacement surface.

## State transitions

| ID | State | Trigger | Required transition | Owner | Verification |
|---|---|---|---|---|---|
| IMD-01 | Focused exact-`merged` row | Changes-view `d` | Immediately add its ID to process-local dismissal state, remove it, and repair navigation; no modal | key handler + merged-row dismissal state | key/state tests |
| IMD-02 | Mixed projected rows | Changes-view `D` | Immediately remove all exact-`merged` rows across the full projection and preserve all others; no modal | same | key/state tests |
| IMD-03 | Empty target | Ineligible `d` or `D` | No row, modal, log, or external state change | same | key/state tests |
| IMD-04 | Refresh | Dismissed terminal row reappears in catalog/reducer input | Keep suppressed for this process | processing logic | refresh tests |
| IMD-05 | Active ID reuse | Dismissed ID returns as non-terminal | Clear dismissal and show active row | processing logic | refresh tests |
| IMD-06 | Worktrees view | `d` or `D` | Existing delete confirmation remains unchanged | worktree key/modal paths | regression tests |

## Implementation boundary

- Keep `dismissed_merged_ids`, status predicate, projection target derivation, row removal, navigation repair, refresh filtering, and informational completion log.
- Replace request/confirm/cancel sequencing with direct individual and bulk functions that derive current targets and immediately call the shared mutation.
- Retain the existing overlay/input-ownership guard inside the direct helpers as defense in depth, even though normal key routing already prevents view actions while another overlay owns input.
- Delete only merged-dismissal variants and branches from `ModalState`, modal validity, modal key ownership, popup renderer, title selection, and associated tests.
- Do not alter unrelated modal exhaustiveness behavior except removing the deleted variants from matches.

## Safety

The direct action is appropriate because it hides only local rows and is reversible by restarting the TUI. Exact status checks occur in the same input turn, eliminating stale bound targets. No command is emitted and no authoritative workflow state changes.
