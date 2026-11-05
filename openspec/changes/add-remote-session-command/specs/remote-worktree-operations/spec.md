## MODIFIED Requirements

### Requirement: Exact remote worktree resources and closed commands

V2 MUST expose `GET /api/v2/worktrees` and `GET /api/v2/worktrees/{worktree_id}`. It MUST extend the closed command enum only with `create_worktree`, `delete_worktree`, `merge_worktree`, `run_worktree_command`, `create_session_worktree`, and `stop_worktree_session`. Every worktree command MUST inherit v2 authentication, `expected_revision`, idempotency, capacity admission, correlation validation, and typed errors.

`create_worktree` MUST accept only `target.change_id` and an empty `params` object. `delete_worktree`, `merge_worktree`, and `run_worktree_command` MUST accept only `target.worktree_id` and an empty `params` object. `create_session_worktree` MUST accept only an empty `params` object. `stop_worktree_session` MUST accept only `session_id`. Unknown target or parameter fields MUST fail schema validation.

#### Scenario: Client reads one worktree by opaque ID

**Given**: A worktree appears in the v2 list with an opaque ID
**When**: An authenticated client reads `/api/v2/worktrees/{worktree_id}`
**Then**: It receives that current resource
**And**: An unknown or retired ID returns `worktree_not_found`

#### Scenario: Generic worktree input is unsupported

**Given**: A client can authenticate to v2
**When**: It submits a path, branch, base commit, generic shell command, or unknown worktree parameter
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: No process is launched and no Git mutation occurs

#### Scenario: Session commands carry no command text

**Given**: A client can authenticate to v2
**When**: It submits `run_worktree_command` or `create_session_worktree` with a `command`, `argv`, `env`, or `path` field
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: No process is launched

### Requirement: Restricted worktree capability surface

V2 MUST expose only managed change-worktree list/detail/create/delete/base-merge behavior and the configured-command session surface defined by `remote-control-api` (launch of the configured `worktree_command` in an existing worktree, creation of a `ws-session-*` worktree with the same setup and cleanup as the TUI `+` key, stop, and session observation). It MUST NOT expose arbitrary worktree commands, argv or environment input, editor launch, UI preference synchronization, teardown bypass, force deletion, client-selected base/branch/path, remote merge resolve/abort, or unsafe recovery permissions.

#### Scenario: Capabilities report recovery boundary

**Given**: A client reads v2 capabilities
**When**: It inspects worktree operations
**Then**: Only list, detail, create, delete, merge, and session are advertised
**And**: Merge-conflict recovery is identified as local or TUI required
**And**: `worktree_sessions` reports whether the configured command is available and its limits
