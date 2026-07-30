## MODIFIED Requirements

### Requirement: on_queue_add hook

The orchestrator SHALL execute `on_queue_add` exactly once after the shared operator command service successfully adds a change to the dynamic queue, regardless of whether the request originated from TUI or another frontend. Initial queue construction, rejected requests, and no-op duplicate additions SHALL NOT execute the hook.

#### Scenario: TUI adds change to queue

- **GIVEN** `hooks.on_queue_add` is configured
- **AND** TUI is in Running or Stopped mode
- **WHEN** the user presses Space on an eligible unqueued change
- **THEN** the shared operator command service mutates the dynamic queue
- **AND** `on_queue_add` executes exactly once with the change ID

#### Scenario: Remote frontend adds change to queue

- **GIVEN** `hooks.on_queue_add` is configured
- **WHEN** a remote frontend requests an eligible dynamic queue addition through the shared operator command service
- **THEN** the queue mutation and hook behavior are identical to the TUI path

#### Scenario: on_queue_add not called for initial queue

- **GIVEN** `hooks.on_queue_add` is configured
- **AND** the operator marks changes before starting orchestration
- **WHEN** orchestration constructs its initial queue
- **THEN** `on_queue_add` is NOT called

#### Scenario: on_queue_add not called for no-op

- **GIVEN** a change is already in the dynamic queue
- **WHEN** any frontend requests the same addition
- **THEN** the request is a no-op
- **AND** `on_queue_add` is NOT called

### Requirement: on_queue_remove hook

The orchestrator SHALL execute `on_queue_remove` exactly once after the shared operator command service successfully removes a change from the dynamic queue, regardless of whether the request originated from TUI or another frontend. Rejected requests and no-op removals SHALL NOT execute the hook.

#### Scenario: TUI removes change from queue

- **GIVEN** `hooks.on_queue_remove` is configured
- **AND** TUI is in Running or Stopped mode
- **WHEN** the user presses Space on an eligible queued change
- **THEN** the shared operator command service mutates the dynamic queue
- **AND** `on_queue_remove` executes exactly once with the change ID

#### Scenario: Remote frontend removes change from queue

- **GIVEN** `hooks.on_queue_remove` is configured
- **WHEN** a remote frontend requests an eligible dynamic queue removal through the shared operator command service
- **THEN** the queue mutation and hook behavior are identical to the TUI path

#### Scenario: on_queue_remove not called for no-op

- **GIVEN** a change is not in the dynamic queue
- **WHEN** any frontend requests its removal
- **THEN** the request is a no-op
- **AND** `on_queue_remove` is NOT called

### Requirement: Available hook types

The orchestrator SHALL support the following hook types:

**Run lifecycle:**
- `on_start`: Run loop started
- `on_finish`: Run loop finished
- `on_error`: Error occurred

**Change lifecycle:**
- `on_change_start`: Change processing started once per change
- `pre_apply`: Before apply execution
- `post_apply`: After successful apply
- `on_change_complete`: Change reached complete task state
- `pre_archive`: Before archive execution
- `post_archive`: After successful archive
- `on_change_end`: Change processing ended after archive
- `on_merged`: Change merged to base branch

**Frontend-independent operator interaction:**
- `on_queue_add`: Shared operator service dynamically added a change to the queue
- `on_queue_remove`: Shared operator service dynamically removed a change from the queue

**TUI-only interaction:**
- `on_approve`: User approved a change with the TUI approval control
- `on_unapprove`: User removed approval with the TUI approval control

#### Scenario: Complete hook list in configuration

- **GIVEN** config contains all hook types
- **WHEN** the orchestrator loads config
- **THEN** all hooks are registered
- **AND** queue hooks are triggered by successful shared-service mutations
- **AND** approval hooks remain TUI-only
