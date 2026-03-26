## Implementation Tasks

- [x] 1. Update the shared row styling logic for blocked/uncommitted entries so focused rows preserve readable contrast in `src/tui/render.rs` (verification: blocked row foreground/background styles differ sufficiently in both `render_changes_list_select` and `render_changes_list_running`)
- [x] 2. Keep blocked-row semantics and badges intact while making the focused state visually distinct, including the uncommitted badge text if touched during the change (verification: rendering branches for `is_parallel_blocked`, badges, and cursor-highlight style remain covered in `src/tui/render.rs`)
- [x] 3. Add or update rendering-focused tests that verify selected blocked rows remain legible and both list modes use the same intended display behavior (verification: `cargo test` covers the affected `src/tui/render.rs` scenarios)

## Future Work

- Consider a broader TUI color-token pass if more overlapping emphasis states appear in other views.
