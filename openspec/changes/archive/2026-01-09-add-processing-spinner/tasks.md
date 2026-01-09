# Tasks: add-processing-spinner

## Implementation Tasks

- [x] 1. Add `spinner_frame: usize` field to `AppState`
  - Initialize to 0 in `AppState::new()`
  - Add `SPINNER_CHARS` constant with Braille pattern

- [x] 2. Increment spinner frame on each render
  - Update spinner_frame in the main event loop
  - Use modulo to cycle through spinner characters

- [x] 3. Update `render_changes_list_running` to show spinner
  - For `QueueStatus::Processing` items, prepend spinner character
  - Display format: `⠋ [XX%]`

- [x] 4. Manual testing
  - Verify spinner animates during processing
  - Confirm spinner stops when processing completes
