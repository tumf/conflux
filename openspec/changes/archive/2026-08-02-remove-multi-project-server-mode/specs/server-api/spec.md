## REMOVED Requirements

### Requirement: change-selection-state

Server-persisted multi-project change selection is removed.

#### Scenario: No server selection state

**Given**: The retained local product
**When**: selection state is inspected
**Then**: No server project registry owns it

### Requirement: toggle-change-selection-api

The server project selection API is removed.

#### Scenario: No project selection route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No multi-project selection route exists

### Requirement: selected-field-in-websocket

The server project WebSocket selection payload is removed.

#### Scenario: No project selection stream

**Given**: The retained router
**When**: streams are enumerated
**Then**: No multi-project selection stream exists

### Requirement: dashboard-change-checkbox

The multi-project dashboard checkbox is removed.

#### Scenario: No project dashboard checkbox

**Given**: Packaged interfaces
**When**: UI surfaces are inspected
**Then**: No multi-project dashboard is present

### Requirement: global-orchestration-status

The server-global orchestration status is removed.

#### Scenario: No server-global status

**Given**: A local process
**When**: status is observed
**Then**: It describes only that local instance

### Requirement: global-run-uses-selected-changes

Server-global execution across selected projects is removed.

#### Scenario: No global project run

**Given**: The retained API
**When**: commands are enumerated
**Then**: No multi-project global run command exists

### Requirement: auto-enqueue-new-projects-during-run

Server project auto-enqueue behavior is removed.

#### Scenario: No project auto-enqueue

**Given**: A local run
**When**: repositories are discovered
**Then**: No server project registry feeds its queue

### Requirement: websocket-orchestration-status

The server-global WebSocket status is removed.

#### Scenario: No server-global status stream

**Given**: The retained streams
**When**: payloads are inspected
**Then**: No multi-project global status is emitted

### Requirement: dashboard-global-run-stop

The multi-project dashboard run/stop controls are removed.

#### Scenario: No dashboard global controls

**Given**: Packaged interfaces
**When**: controls are inspected
**Then**: No multi-project dashboard controls exist

### Requirement: per-project-control-run

Per-project server run control is removed.

#### Scenario: No per-project run route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server project run route exists

### Requirement: per-project-control-stop

Per-project server stop control is removed.

#### Scenario: No per-project stop route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server project stop route exists

### Requirement: per-project-control-retry

Per-project server retry control is removed.

#### Scenario: No per-project retry route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server project retry route exists

### Requirement: per-project-run-stop-buttons

Per-project dashboard buttons are removed.

#### Scenario: No project buttons

**Given**: Packaged interfaces
**When**: controls are inspected
**Then**: No per-project dashboard buttons exist

### Requirement: Project state snapshots include sync-state metadata

Server project sync snapshots are removed.

#### Scenario: No project sync snapshot

**Given**: The retained API
**When**: resources are enumerated
**Then**: No server project sync-state resource exists

### Requirement: Per-change stop-and-dequeue API for running changes

The multi-project server stop-and-dequeue API is removed.

#### Scenario: No server stop-and-dequeue route

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server project stop-and-dequeue route exists

### Requirement: Dashboard can invoke stop-and-dequeue for active changes

The dashboard action for the removed server API is removed.

#### Scenario: No dashboard stop-and-dequeue action

**Given**: Packaged interfaces
**When**: actions are inspected
**Then**: No multi-project dashboard action exists

### Requirement: WebSocket change list uses reducer-derived status

The multi-project server WebSocket change-list contract is removed; retained local web status remains governed by `web-monitoring` and `remote-control-api`.

#### Scenario: No server project change list

**Given**: The retained router
**When**: streams are enumerated
**Then**: No multi-project server change-list stream exists
