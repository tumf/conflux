# cli Specification Delta

## ADDED Requirements

### Requirement: Native Task Progress Parsing

The system SHALL parse `tasks.md` files natively to determine task completion status, independent of the openspec CLI.

#### Scenario: Parse bullet list tasks
- **WHEN** a `tasks.md` file contains bullet list checkboxes (`- [ ]`, `- [x]`)
- **THEN** the system counts each `- [ ]` as an incomplete task
- **AND** the system counts each `- [x]` as a completed task
- **AND** case-insensitive matching is used for `[x]` and `[X]`

#### Scenario: Parse numbered list tasks
- **WHEN** a `tasks.md` file contains numbered list checkboxes (`1. [ ]`, `1. [x]`)
- **THEN** the system counts each numbered `[ ]` as an incomplete task
- **AND** the system counts each numbered `[x]` as a completed task

#### Scenario: Ignore non-task lines
- **WHEN** a `tasks.md` file contains markdown headers, plain text, or indented sub-items
- **THEN** those lines are not counted as tasks
- **AND** only top-level checkbox items are counted

#### Scenario: Fallback when tasks.md not found
- **WHEN** the `tasks.md` file does not exist for a change
- **THEN** the system uses the task count from openspec CLI output
- **AND** no error is raised

### Requirement: Task Progress Fallback Behavior

The system SHALL use native task parsing as primary source when openspec CLI returns zero task counts.

#### Scenario: CLI returns zero tasks
- **WHEN** openspec CLI returns `completedTasks: 0, totalTasks: 0` for a change
- **AND** a `tasks.md` file exists for that change
- **THEN** the system uses native parsing to determine actual task counts
- **AND** the TUI displays the native-parsed task counts

#### Scenario: CLI returns non-zero tasks
- **WHEN** openspec CLI returns non-zero task counts for a change
- **THEN** the system uses the CLI-provided task counts
- **AND** native parsing is not performed for that change
