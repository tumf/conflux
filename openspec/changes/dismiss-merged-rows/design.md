# Design

## Authority and precedence

The visual authority is the existing production Changes view and confirmation-overlay language at Git revision `47821e12d80ed6afd2488fbcbe3855ef54c4df26`.

Precedence:

1. This change's interaction and safety contract.
2. Existing production row layout, colors, selection background, panel borders, and dynamic hint composition.
3. Existing native terminal behavior.

No new visual system, color, row anatomy, or navigation surface is introduced.

## State inventory

| ID | State | Trigger | Expected rendering/transition | Production owner | Verification |
|---|---|---|---|---|---|
| MRD-01 | Focused merged row | Changes view, no overlay | Existing merged row plus `d: dismiss`; bulk hint also appears when any merged row exists | `src/tui/render.rs` | deterministic render test |
| MRD-02 | Focused non-merged row with other merged rows | Changes view, no overlay | No individual hint; `D: dismiss all merged` appears | `src/tui/render.rs` | deterministic render test |
| MRD-03 | No visible merged rows | Changes view, no overlay | Neither dismissal hint appears | `src/tui/render.rs` | deterministic render test |
| MRD-04 | Individual confirmation | `d` on merged row | Centered bordered confirmation names exact ID and shows `Y: confirm`, `N/Esc: cancel`; underlying execution mode remains current | `src/tui/types.rs`, `src/tui/render.rs`, `src/tui/key_handlers.rs` | modal/key tests and render test |
| MRD-05 | Bulk confirmation | `D` with one or more projected merged rows | Centered bordered confirmation names exact count and shows `Y: confirm`, `N/Esc: cancel`; all merged IDs in the Changes-list projection are bound at open time regardless of scroll position | same | modal/key tests and render test |
| MRD-06 | Cancelled confirmation | `N` or `Esc` | Overlay closes; rows, cursor, marks, filter, and dismissed set unchanged | `src/tui/key_handlers.rs`, AppState modal logic | unit test |
| MRD-07 | Confirmed individual dismissal | `Y` | Bound merged row disappears; nearest surviving row receives focus | AppState presentation logic | unit test |
| MRD-08 | Confirmed bulk dismissal | `y` or `Y` | All bound IDs still merged disappear; non-merged rows retain order and state. If status recheck leaves no eligible IDs, the overlay closes without changing rows, dismissed state, or logs | AppState presentation logic | unit test |
| MRD-09 | Empty result | Confirm dismissal of only/all rows | Changes pane remains valid and unselected with existing empty presentation | AppState selection logic and renderer | unit/render test |
| MRD-10 | Refresh after dismissal | Catalog or reducer refresh | Retained terminal/merged state for dismissed IDs remains absent; an active non-terminal catalog row with the same ID clears dismissal and is rendered; no workflow authority is changed | `src/tui/state/processing_logic.rs`, reducer cache synchronization boundary | unit test |
| MRD-11 | Overlay input ownership | Any dismissal confirmation open | Unrelated row, run, edit, bulk-toggle, and navigation keys do nothing; only case-insensitive `y`/`n` or `Esc` act | `src/tui/key_handlers.rs` | key-routing test |
| MRD-12 | Worktrees view | `d` or `D` | Existing worktree-delete flow remains unchanged | `src/tui/key_handlers.rs`, existing worktree confirmation renderer | existing and regression tests |

Terminal width does not create a new interaction variant. Existing title clipping/layout behavior remains authoritative; tests use a width sufficient to render complete new hints and overlay copy.

## State ownership

`dismissed_merged_ids` is a `HashSet<String>` on `AppState`. It is presentation-only and empty at startup. It is not serialized, sent through `TuiCommand`, copied into `OrchestratorState`, or exposed through `/api/v2`.

"Visible" means present in the current Changes-list projection regardless of scroll position. The individual modal binds one ID. The bulk modal binds every projected merged ID captured when `D` is pressed, not a count-only or later recomputed target. Confirmation rechecks each bound row's current display status and dismisses only IDs still `merged`; this prevents a stale overlay from hiding a row whose status changed while the modal was open. If none remains eligible, confirmation is a mutation-free no-op and produces no informational log.

## Projection and cursor behavior

Confirmed dismissal performs one local projection update:

1. Add eligible confirmed IDs to `dismissed_merged_ids`.
2. Remove those rows from `changes`.
3. Remove their IDs from `known_change_ids` and recompute `new_change_count`.
4. If the selected-proposal log filter targets a removed row, disable that filter without deleting buffered logs.
5. Preserve the old cursor index when a row now occupies it; otherwise clamp to the final row; clear selection when the list is empty.

Refresh suppresses fetched and retained rows against `dismissed_merged_ids` only while they represent terminal/merged presentation state. If a later catalog refresh re-observes a dismissed ID as an active non-terminal change, refresh removes the ID from `dismissed_merged_ids` and renders the active row normally. Current reducer cache synchronization mutates existing rows only; catalog processing is the row-generation boundary and therefore owns this suppression/reactivation rule.

## Accessibility and interaction

Meaning remains textual. Hints name the keys and actions; confirmations name the destructive-looking presentation action and make clear that only TUI rows are hidden. Confirmation uses the existing keyboard-only modal pattern. Color is not required to identify state or available action.

## Preserved boundaries

- No repository file, archive, branch, or worktree deletion.
- No change to scheduler, reducer, run-control, execution marks, queues, lifecycle output, or API/Web state.
- No removal from the bounded log buffer.
- Worktree deletion remains owned by the Worktrees view and its existing confirmations.
