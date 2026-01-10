# Tasks: Add Approval Workflow

## Implementation Tasks

- [x] Add approval module in `src/approval.rs`
  - [x] Implement `discover_md_files()` function
  - [x] Implement `compute_md5()` function
  - [x] Implement `parse_approved_file()` function
  - [x] Implement `check_approval()` function
  - [x] Implement `approve_change()` function
  - [x] Implement `unapprove_change()` function
  - [x] Add unit tests for approval module

- [x] Extend `Change` struct in `src/openspec.rs`
  - [x] Add `is_approved` field
  - [x] Update `list_changes_native()` to populate `is_approved`

- [x] Add CLI `approve` subcommand in `src/cli.rs`
  - [x] Add `ApproveArgs` and `ApproveAction` structs
  - [x] Add `Approve` variant to `Commands` enum

- [x] Implement CLI approve handlers in `src/main.rs`
  - [x] Handle `approve set {change_id}`
  - [x] Handle `approve unset {change_id}`
  - [x] Handle `approve status {change_id}`

- [x] Extend TUI for approval workflow in `src/tui.rs`
  - [x] Add `is_approved` field to `ChangeState`
  - [x] Add `@` key binding in selection mode
  - [x] Display approval badge in change list
  - [x] Auto-queue approved changes on TUI startup

- [x] Update orchestrator queue logic in `src/orchestrator.rs`
  - [x] Filter unapproved changes from queue
  - [x] Show warning for unapproved changes in CLI mode
  - [x] Support auto-queue of approved changes

- [x] Add integration tests
  - [x] Test approval file creation and validation
  - [x] Test CLI approve commands
  - [x] Test TUI approval toggle
  - [x] Test orchestrator queue filtering
