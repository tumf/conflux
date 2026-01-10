# Tasks

## Implementation Tasks

- [x] 1. Modify `render_changes_list_running` in `src/tui.rs` to display checkbox indicators (`[ ]`, `[@]`, `[x]`) based on approval and queue status
- [x] 2. Update `toggle_approval` method to implement new state transitions:
  - `[ ]` (unapproved) → `@` → `[x]` (approved + queued)
  - `[@]` (approved, not queued) → `@` → `[ ]` (unapproved)
  - `[x]` (queued, not processing) → `@` → `[ ]` (unapproved + removed from queue)
- [x] 3. Enable `toggle_approval` to work in Running/Completed modes for non-processing changes
- [x] 4. Add warning message when user attempts to toggle approval on a processing change: "Cannot change approval for processing change"
- [x] 5. Verify `toggle_selection` already blocks queue removal for Processing state (existing behavior)
- [x] 6. Update running mode panel title to include `@: approve` in help text
- [x] 7. Add unit tests for new approval state transitions
- [x] 8. Add unit tests for approval toggle blocked during Processing state
- [x] 9. Run `cargo fmt && cargo clippy && cargo test` to verify changes
