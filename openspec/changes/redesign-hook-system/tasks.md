# Tasks

## Phase 1: Core Hook System

- [ ] Update `HookType` enum to remove old types and add new ones
  - Remove: `OnFirstApply`, `OnIterationStart`, `OnIterationEnd`, `OnQueueChange`
  - Add: `OnChangeStart`, `OnChangeEnd`, `OnQueueAdd`, `OnQueueRemove`, `OnApprove`, `OnUnapprove`

- [ ] Update `HookContext` struct with new fields
  - Add: `changes_processed`, `remaining_changes`, `apply_count`
  - Remove: `iteration` (replaced by `changes_processed`)

- [ ] Update `HooksConfig` struct to match new hook types
  - Remove: `on_first_apply`, `on_iteration_start`, `on_iteration_end`, `on_queue_change`
  - Add: `on_change_start`, `on_change_end`, `on_queue_add`, `on_queue_remove`, `on_approve`, `on_unapprove`

- [ ] Update environment variable generation in `to_env_vars()`
  - Add new variables: `OPENSPEC_CHANGES_PROCESSED`, `OPENSPEC_REMAINING_CHANGES`, `OPENSPEC_APPLY_COUNT`
  - Remove: `OPENSPEC_ITERATION`

- [ ] Update placeholder expansion in `expand_placeholders()`
  - Add: `{changes_processed}`, `{remaining_changes}`, `{apply_count}`
  - Remove: `{iteration}`

## Phase 2: Orchestrator Integration (CLI mode)

- [ ] Add change tracking state to `Orchestrator`
  - Add: `current_change_id: Option<String>`
  - Add: `completed_change_ids: HashSet<String>`
  - Add: `apply_counts: HashMap<String, u32>`

- [ ] Implement change lifecycle detection logic
  - Detect when processing switches to a new change
  - Track apply count per change

- [ ] Update `run()` method hook calls
  - Add `on_change_start` call when change switches
  - Add `on_change_end` call after successful archive
  - Remove old hook calls

- [ ] Update context creation for each hook
  - Pass `apply_count` to pre_apply/post_apply contexts
  - Pass `changes_processed` and `remaining_changes` to all contexts

## Phase 3: TUI Integration

- [ ] Add change tracking state to TUI orchestrator
  - Mirror the tracking logic from CLI mode

- [ ] Implement `on_change_start` hook in TUI
  - Call when starting to process a new change

- [ ] Implement `on_change_end` hook in TUI
  - Call after successful archive

- [ ] Implement `on_queue_add` hook in TUI
  - Call when user presses Space to add a change to queue
  - Pass change_id in context

- [ ] Implement `on_queue_remove` hook in TUI
  - Call when user presses Space to remove a change from queue
  - Pass change_id in context

- [ ] Implement `on_approve` hook in TUI
  - Call when user presses @ to approve a change
  - Pass change_id and task progress in context

- [ ] Implement `on_unapprove` hook in TUI
  - Call when user presses @ to unapprove a change
  - Pass change_id in context
  - Note: on_queue_remove is NOT called separately when unapproving removes from queue

- [ ] Ensure hook parity between TUI and CLI modes
  - All change lifecycle hooks called at same logical points
  - Same context data provided
  - Note: User interaction hooks (on_queue_add/on_queue_remove/on_approve/on_unapprove) are TUI-only

## Phase 4: Configuration Templates

- [ ] Update CLAUDE_TEMPLATE hooks section
  - Add commented examples for all 14 hook types
  - Each example uses `echo` with all available placeholders for that hook
  - Use simple string format (no object format with timeout/continue_on_failure)
  - Group hooks by category: Run lifecycle, Change lifecycle, User interaction (TUI only)

- [ ] Update OPENCODE_TEMPLATE hooks section (same structure)

- [ ] Update CODEX_TEMPLATE hooks section (same structure)

- [ ] Update template tests to verify new hook examples

## Phase 5: Testing

- [ ] Update unit tests for `HookType`
  - Test new config keys
  - Test display formatting

- [ ] Update unit tests for `HookContext`
  - Test new environment variables
  - Test new placeholder expansion

- [ ] Add integration tests for change lifecycle
  - Test on_change_start called once per change
  - Test on_change_end called after archive
  - Test apply_count increments correctly

- [ ] Add TUI hook tests
  - Verify hook parity with CLI mode

- [ ] Add on_queue_add/on_queue_remove tests
  - Test hooks called on Space key press
  - Test change_id is correctly passed

- [ ] Add on_approve/on_unapprove tests
  - Test hooks called on @ key press
  - Test change_id and task progress passed correctly
  - Test on_queue_remove NOT called when unapproving removes from queue

## Phase 6: Documentation

- [ ] Update README.md hooks section
  - Document new hook types (on_change_start, on_change_end, on_queue_add, on_queue_remove, on_approve, on_unapprove)
  - Remove old hook types (on_first_apply, on_iteration_start, on_iteration_end, on_queue_change)
  - Add placeholder availability table
  - Add configuration template example

- [ ] Remove old hook requirements from configuration/spec.md
  - Remove on_first_apply scenarios
  - Remove on_iteration_* scenarios
  - Remove on_queue_change scenarios

- [ ] Add cross-reference to hooks/spec.md from configuration/spec.md
