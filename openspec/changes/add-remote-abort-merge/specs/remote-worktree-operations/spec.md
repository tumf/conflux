## MODIFIED Requirements

### Requirement: Exact remote worktree resources and closed commands

V2 MUST expose `GET /api/v2/worktrees` and `GET /api/v2/worktrees/{worktree_id}`. It MUST extend the closed command enum only with `create_worktree`, `delete_worktree`, `merge_worktree`, and `merge_worktree_abort_on_conflict`. Every worktree command MUST inherit v2 authentication, `expected_revision`, idempotency, capacity admission, correlation validation, and typed errors.

`create_worktree` MUST accept only `target.change_id` and an empty `params` object. `delete_worktree`, `merge_worktree`, and `merge_worktree_abort_on_conflict` MUST accept only `target.worktree_id` and an empty `params` object. Unknown target or parameter fields MUST fail schema validation.

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

### Requirement: Shared conflict-preserving worktree merge

Remote `merge_worktree` MUST resolve the current opaque ID, require empty params, and use the same base merge, root/base guard, `on_merged` hook, reducer transition, and event path as TUI. A Git conflict MUST retain intermediate repository state and return repository-relative conflict files plus `local_or_tui_required` recovery guidance. `merge_worktree` MUST NOT automatically abort the merge or expose remote resolve. Abort-on-conflict MUST be reachable remotely only through the separate, explicitly chosen `merge_worktree_abort_on_conflict` command defined by `remote-control-api`, which refuses predicted conflicts, verifies restoration from repository evidence, and never reports a restoration it did not confirm. Incompatible mutations MUST return `root_busy` until an existing local or TUI flow resolves or aborts a retained merge, including one a failed remote abort left behind.

#### Scenario: Conflict remains available for local resolution

**Given**: A current eligible worktree whose branch conflicts with base
**When**: An authenticated client submits merge by worktree ID with matching revision and idempotency key
**Then**: The command fails with `merge_conflict` and conflict file evidence
**And**: The intermediate Git merge state is retained
**And**: Recovery is reported as local or TUI required
**And**: `on_merged` is not executed

#### Scenario: Incompatible mutation remains blocked after conflict

**Given**: A remote merge left an intermediate conflict state
**When**: A client requests another incompatible mutation before local recovery
**Then**: The server returns `root_busy`
**And**: The retained conflict state is not discarded

#### Scenario: Successful merge runs shared completion effects

**Given**: A current eligible worktree whose branch merges cleanly
**When**: The merge command completes
**Then**: The base contains the worktree result
**And**: `on_merged` runs exactly once
**And**: Shared reducer and event state report completion

#### Scenario: Abort is a separate explicit command

**Given**: A current eligible worktree whose branch conflicts with base
**When**: A client submits `merge_worktree_abort_on_conflict`
**Then**: The owner refuses a predicted conflict before mutation or aborts a real one and verifies restoration
**And**: `merge_worktree` for the same worktree still preserves the conflict

### Requirement: Restricted worktree capability surface

V2 MUST expose only managed change-worktree list/detail/create/delete/base-merge behavior and the explicit abort-on-conflict merge defined by `remote-control-api`. It MUST NOT expose arbitrary worktree commands, editor launch, temporary sessions, UI preference synchronization, teardown bypass, force deletion, client-selected base/branch/path, remote merge resolve, or unsafe recovery permissions.

#### Scenario: Capabilities report recovery boundary

**Given**: A client reads v2 capabilities
**When**: It inspects worktree operations
**Then**: Only list, detail, create, delete, merge, and merge_abort are advertised
**And**: Merge-conflict recovery for `merge_worktree` is identified as local or TUI required
**And**: `merge_conflict_preserves_state` is `true` and describes `merge_worktree`, while `merge_abort` names the verified-restoration command
