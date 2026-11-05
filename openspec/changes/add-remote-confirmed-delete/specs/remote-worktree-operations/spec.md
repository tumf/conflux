## MODIFIED Requirements

### Requirement: Exact remote worktree resources and closed commands

V2 MUST expose `GET /api/v2/worktrees`, `GET /api/v2/worktrees/{worktree_id}`, and `GET /api/v2/worktrees/{worktree_id}/delete-preview`. It MUST extend the closed command enum only with `create_worktree`, `delete_worktree`, `merge_worktree`, and `delete_worktree_confirmed`. Every worktree command MUST inherit v2 authentication, `expected_revision`, idempotency, capacity admission, correlation validation, and typed errors.

`create_worktree` MUST accept only `target.change_id` and an empty `params` object. `delete_worktree` and `merge_worktree` MUST accept only `target.worktree_id` and an empty `params` object. `delete_worktree_confirmed` MUST accept only `target.worktree_id` and the typed consent params defined by `remote-control-api`. Unknown target or parameter fields MUST fail schema validation.

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

#### Scenario: Ordinary delete still accepts no destructive field

**Given**: A client can authenticate to v2
**When**: It submits `delete_worktree` with `skip_teardown`, `discard_known_dirty`, or `discard_commits_ahead`
**Then**: The server returns HTTP 422 with `validation_failed`
**And**: Nothing is torn down or removed

### Requirement: Managed remote worktree deletion

Remote `delete_worktree` MUST resolve the current opaque ID, require empty params, pass existing eligibility guards, and complete managed teardown before removing the resource. Failure MUST retain the resource and its identity binding. `delete_worktree` MUST NOT expose skip-teardown, force, unsafe recovery, path, or branch controls. Skip-teardown, known-dirty discard, and commits-ahead discard MUST be reachable remotely only through the separate `delete_worktree_confirmed` command defined by `remote-control-api`, bound to explicit consent over branch, HEAD, Git identity, observed dirty and ahead facts, the separate known-ignored-content fact, merge base, discardable content identity, and ignored content identity, re-verified under the root mutation guard before teardown and again before removal. Because forced removal deletes ignored paths, known ignored content MUST require the same `discard_known_dirty` permission as known uncommitted work even when the worktree is Git-clean, and content the oracle cannot bound (a nested repository or a status-reported submodule, or records past its documented limits) MUST fail the confirmed command closed rather than be discarded.

#### Scenario: Successful delete retires identity after teardown

**Given**: A known clean eligible worktree and matching revision
**When**: An authenticated client submits delete by worktree ID with an idempotency key
**Then**: Managed teardown completes
**And**: The worktree is removed
**And**: Its opaque ID is retired

#### Scenario: Consented destructive delete is a separate command

**Given**: A known dirty worktree and a preview whose consent names its current content identity
**When**: An authenticated client submits `delete_worktree_confirmed` with `discard_known_dirty: true` and that consent
**Then**: The shared service re-verifies every consented fact under the root guard
**And**: The worktree is removed only when nothing drifted
**And**: `delete_worktree` for the same worktree still refuses `worktree_dirty`
**And**: A Git-clean worktree with ignored content is removed by `delete_worktree_confirmed` only with `discard_known_dirty: true`

### Requirement: Restricted worktree capability surface

V2 MUST expose only managed change-worktree list/detail/create/delete/base-merge behavior and the consent-bound confirmed deletion defined by `remote-control-api`. It MUST NOT expose arbitrary worktree commands, editor launch, temporary sessions, UI preference synchronization, unconsented teardown bypass, unconsented force deletion, client-selected base/branch/path, remote merge resolve/abort, or unsafe recovery permissions beyond the three explicitly consented ones.

#### Scenario: Capabilities report recovery boundary

**Given**: A client reads v2 capabilities
**When**: It inspects worktree operations
**Then**: Only list, detail, create, delete, merge, and delete_confirmed are advertised
**And**: Merge-conflict recovery is identified as local or TUI required
**And**: `delete_requires_teardown` is `true` and describes `delete_worktree`, while `confirmed_delete` names the consented bypass
