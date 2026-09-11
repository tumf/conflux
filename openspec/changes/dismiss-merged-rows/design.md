# Design

## Authority and precedence

The visual authority is the existing production Changes view at Git revision `47821e12d80ed6afd2488fbcbe3855ef54c4df26`.

Precedence:

1. This change's interaction and safety contract.
2. Existing production row layout, colors, selection background, panel borders, and dynamic hint composition.
3. Existing native terminal behavior.

No new visual system, color, row anatomy, or navigation surface is introduced.

## State inventory

| ID | State | Trigger | Expected rendering/transition | Production owner | Verification |
|---|---|---|---|---|---|
| MRD-01 | Focused merged row | Changes view | Existing merged row plus `d: dismiss`; bulk hint also appears when any merged row exists | `src/tui/render.rs` | deterministic render test |
| MRD-02 | Focused non-merged row with other merged rows | Changes view | No individual hint; `D: dismiss all merged` appears | `src/tui/render.rs` | deterministic render test |
| MRD-03 | No visible merged rows | Changes view | Neither dismissal hint appears | `src/tui/render.rs` | deterministic render test |
| MRD-04 | Immediate individual dismissal | `d` on merged row | Focused merged row disappears immediately; nearest surviving row receives focus; no modal opens | AppState presentation logic and `src/tui/key_handlers.rs` | unit/key test |
| MRD-05 | Immediate bulk dismissal | `D` with one or more projected merged rows | Every projected merged row disappears immediately regardless of scroll position; non-merged rows retain order and state; no modal opens | same | unit/key test |
| MRD-06 | Empty result | Immediate dismissal removes the only/all rows | Changes pane remains valid and unselected with existing empty presentation | AppState selection logic and renderer | unit/render test |
| MRD-07 | Refresh after dismissal | Catalog or reducer refresh | Retained terminal/merged state for dismissed IDs remains absent; an active non-terminal catalog row with the same ID clears dismissal and is rendered; no workflow authority is changed | `src/tui/state/processing_logic.rs`, reducer cache synchronization boundary | unit test |
| MRD-08 | Worktrees view | `d` or `D` | Existing worktree-delete flow remains unchanged | `src/tui/key_handlers.rs`, existing worktree confirmation renderer | existing and regression tests |

Terminal width does not create a new interaction variant. Existing title clipping/layout behavior remains authoritative; tests use a width sufficient to render the complete new hints.

## State ownership

`dismissed_merged_ids` is a `HashSet<String>` on `AppState`. It is presentation-only and empty at startup. It is not serialized, sent through `TuiCommand`, copied into `OrchestratorState`, or exposed through `/api/v2`. No merged-row dismissal variant is added to `ModalState`.

"Visible" means present in the current Changes-list projection regardless of scroll position. Each key handler derives its targets from the current projection and current display statuses in the same input-handling turn, then applies dismissal immediately. There is no confirmation interval or stale target snapshot.

## Projection and cursor behavior

Dismissal performs one local projection update:

1. Add eligible target IDs to `dismissed_merged_ids`.
2. Remove those rows from `changes`.
3. Remove their IDs from `known_change_ids` and recompute `new_change_count`.
4. If the selected-proposal log filter targets a removed row, disable that filter without deleting buffered logs.
5. Preserve the old cursor index when a row now occupies it; otherwise clamp to the final row; clear selection when the list is empty.

Refresh suppresses fetched and retained rows against `dismissed_merged_ids` only while they represent terminal/merged presentation state. If a later catalog refresh re-observes a dismissed ID as an active non-terminal change, refresh removes the ID from `dismissed_merged_ids` and renders the active row normally. Current reducer cache synchronization mutates existing rows only; catalog processing is the row-generation boundary and therefore owns this suppression/reactivation rule.

## Accessibility and interaction

Meaning remains textual. Hints name the keys and actions. Color is not required to identify state or available action. Because dismissal changes presentation state only and is restored by restarting the TUI, no confirmation UI is shown.

## Preserved boundaries

- No repository file, archive, branch, or worktree deletion.
- No change to scheduler, reducer, run-control, execution marks, queues, lifecycle output, or API/Web state.
- No removal from the bounded log buffer.
- Worktree deletion remains owned by the Worktrees view and its existing confirmations.
