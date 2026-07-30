### Requirement: Exact remote worktree resources and closed commands

V2 MUST expose `GET /api/v2/worktrees` and `GET /api/v2/worktrees/{worktree_id}`. It MUST extend the closed command enum only with `create_worktree`, `delete_worktree`, and `merge_worktree`. Every worktree command MUST inherit v2 authentication, `expected_revision`, idempotency, capacity admission, correlation validation, and typed errors.

`create_worktree` MUST accept only `target.change_id` and an empty `params` object. `delete_worktree` and `merge_worktree` MUST accept only `target.worktree_id` and an empty `params` object. Unknown target or parameter fields MUST fail schema validation.

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

### Requirement: Opaque worktree mutation identity

Every observed worktree MUST receive a random process-local 128-bit hexadecimal `worktree_id`. Delete and merge commands MUST target only this ID and MUST NOT accept paths, repository IDs, or branch names as mutation identity.

#### Scenario: Recreated path receives a new identity

**Given**: A worktree ID was allocated and its resource was removed
**When**: A worktree is later created at the same filesystem path
**Then**: The retired ID is not reused
**And**: A new random worktree ID is allocated

### Requirement: Redacted non-confidential worktree observations

V2 worktree responses MUST expose repository-relative display paths and a 16-character hexadecimal repository correlation ID derived by FNV-1a 64-bit from canonical repository identity. They MUST NOT directly serialize canonical or absolute repository roots. `repository_id` MUST NOT be accepted as authorization or mutation identity, and the contract MUST NOT claim that it prevents dictionary inference of likely paths.

#### Scenario: Worktree list does not directly disclose root

**Given**: A repository root and managed worktrees exist
**When**: An authenticated client lists v2 worktrees
**Then**: Paths are repository-relative
**And**: The canonical absolute repository root is absent from the response
**And**: The repository ID is documented as correlation rather than a secret

### Requirement: Deterministic managed change-worktree creation

`create_worktree` MUST resolve an existing managed, non-archived, eligible change from `target.change_id`. It MUST derive branch and path server-side and use the current managed base HEAD. Clients MUST NOT select branch, path, or base commit. If a current worktree already exists for the change, the command MUST return HTTP 409 with `worktree_exists` and MUST NOT mutate Git.

#### Scenario: Eligible change creates from current base

**Given**: An eligible managed change has no worktree and expected revision matches
**When**: An authenticated client submits `create_worktree` with its change ID, empty params, and an idempotency key
**Then**: The shared service creates the worktree from current managed base HEAD
**And**: The response includes its newly allocated opaque worktree ID

#### Scenario: Existing worktree conflicts

**Given**: A current worktree already exists for a change
**When**: A client submits `create_worktree` for that change
**Then**: The server returns `worktree_exists`
**And**: It does not treat the request as a successful no-op

### Requirement: Fail-closed dirty state

Dirty observation MUST support true, false, and unknown. Unknown MUST serialize as `null` and MUST make delete ineligible.

#### Scenario: Dirty check failure blocks delete

**Given**: The server cannot determine whether a worktree is dirty
**When**: A client requests deletion
**Then**: The command returns `worktree_dirty_unknown`
**And**: The worktree is not removed

### Requirement: Managed remote worktree deletion

Remote worktree deletion MUST resolve the current opaque ID, require empty params, pass existing eligibility guards, and complete managed teardown before removing the resource. Failure MUST retain the resource and its identity binding. V2 MUST NOT expose skip-teardown, force, unsafe recovery, path, or branch controls.

#### Scenario: Successful delete retires identity after teardown

**Given**: A known clean eligible worktree and matching revision
**When**: An authenticated client submits delete by worktree ID with an idempotency key
**Then**: Managed teardown completes
**And**: The worktree is removed
**And**: Its opaque ID is retired

### Requirement: Shared conflict-preserving worktree merge

Remote merge MUST resolve the current opaque ID, require empty params, and use the same base merge, root/base guard, `on_merged` hook, reducer transition, and event path as TUI. A Git conflict MUST retain intermediate repository state and return repository-relative conflict files plus `local_or_tui_required` recovery guidance. It MUST NOT automatically abort the merge or expose remote resolve/abort. Incompatible mutations MUST return `root_busy` until an existing local or TUI flow resolves or aborts the merge.

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

### Requirement: Restricted worktree capability surface

V2 MUST expose only managed change-worktree list/detail/create/delete/base-merge behavior. It MUST NOT expose arbitrary worktree commands, editor launch, temporary sessions, UI preference synchronization, teardown bypass, force deletion, client-selected base/branch/path, remote merge resolve/abort, or unsafe recovery permissions.

#### Scenario: Capabilities report recovery boundary

**Given**: A client reads v2 capabilities
**When**: It inspects worktree operations
**Then**: Only list, detail, create, delete, and merge are advertised
**And**: Merge-conflict recovery is identified as local or TUI required
