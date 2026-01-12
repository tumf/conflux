## MODIFIED Requirements

### Requirement: TUI Archive Loop

The TUI running mode SHALL archive all completed changes before starting the next apply operation.

#### Scenario: Archive before next apply
- **WHEN** TUI is in running mode
- **AND** one or more queued changes have reached 100% task completion
- **THEN** all complete changes are archived before any new apply command starts
- **AND** the archive process follows the same hooks (pre_archive, post_archive) as normal archiving

#### Scenario: Multiple complete changes
- **WHEN** TUI is in running mode
- **AND** multiple changes reach 100% completion simultaneously
- **THEN** all complete changes are archived in sequence
- **AND** processing continues only after all complete changes are archived

#### Scenario: Archive on loop iteration
- **WHEN** TUI orchestrator starts a new processing iteration
- **THEN** it first checks for any complete changes in the queue
- **AND** archives all complete changes before selecting the next change to apply

#### Scenario: Archive path validation uses correct directory
- **WHEN** archive command completes successfully
- **THEN** the system SHALL verify the change was moved to `openspec/changes/archive/`
- **AND** if the change directory still exists in `openspec/changes/` and not in `openspec/changes/archive/`, return ArchiveResult::Failed

#### Scenario: Archive failure does not block loop
- **WHEN** archive command succeeds but file movement verification fails
- **THEN** the change SHALL remain in pending_changes set
- **AND** the loop SHALL continue to process other changes
- **AND** the failed change SHALL be retried on next iteration
