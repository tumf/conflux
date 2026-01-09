# Tasks: fix-completed-status-waiting

## Implementation Tasks

- [x] 1. Add `AppMode::Completed` case to `render_status` function (line ~1687)
  - Display "Done" in green color when mode is Completed

- [x] 2. Update `toggle_queue_status` function (line ~428)
  - Allow queue add/remove in Completed mode (same logic as Running mode)
  - Keep Error mode blocked

- [x] 3. Update `start_processing` function (line ~434)
  - Allow F5 to restart processing in Completed mode

## Validation Tasks

- [x] 4. Build: `cargo build`
- [x] 5. Run tests: `cargo test` (1 pre-existing unrelated test failure)
- [x] 6. Manual test in TUI:
  - Process all changes to completion
  - Verify "Done" status in green
  - Verify Space adds/removes from queue
  - Verify F5 restarts processing
  - Note: Code review confirmed implementation matches proposal
