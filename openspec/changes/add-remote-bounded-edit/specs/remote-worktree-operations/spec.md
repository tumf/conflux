## MODIFIED Requirements

### Requirement: Exact remote worktree resources and closed commands

V2 MUST expose `GET /api/v2/worktrees`, `GET /api/v2/worktrees/{worktree_id}`, and the bounded text read `GET /api/v2/worktrees/{worktree_id}/text?path=<relative>`. It MUST extend the closed command enum only with `create_worktree`, `delete_worktree`, `merge_worktree`, and `write_worktree_text`. Every worktree command MUST inherit v2 authentication, `expected_revision`, idempotency, capacity admission, correlation validation, and typed errors.

`create_worktree` MUST accept only `target.change_id` and an empty `params` object. `delete_worktree` and `merge_worktree` MUST accept only `target.worktree_id` and an empty `params` object. `write_worktree_text` MUST accept only `target.worktree_id` and the typed `params` (`path`, `expected`, `content`) defined by `remote-control-api`. Unknown target or parameter fields MUST fail schema validation. The text read and write MUST stay inside the addressed worktree: a path that descends into a nested worktree, submodule, or embedded repository below it MUST be refused with `nested_repository`, and that nested worktree MUST be addressable only by its own opaque ID. The worktree MUST be reported non-editable and the write refused with `managed_process_live` while the owner's managed writer coverage for that worktree directory is `Busy` or `Unknown` as defined by `remote-control-api`; the owner MUST NOT kill, sweep, or reset anything to make the worktree editable.

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

#### Scenario: Text read is addressed by opaque ID only

**Given**: A worktree appears in the v2 list with an opaque ID
**When**: An authenticated client reads `/api/v2/worktrees/{worktree_id}/text?path=README.md`
**Then**: It receives the bounded text read for that worktree's file
**And**: A retired ID returns `worktree_not_found` and no filesystem path is accepted in its place
**And**: The main worktree's ID does not reach a file inside another change's worktree placed under the project root
**And**: `editable: false` with `managed_process_live` is reported while a writer the owner launched is bound to the worktree or its coverage is unknown

### Requirement: Restricted worktree capability surface

V2 MUST expose only managed change-worktree list/detail/create/delete/base-merge behavior and the bounded, opaque-ID-addressed text read/write defined by `remote-control-api`. It MUST NOT expose arbitrary worktree commands, editor launch, temporary sessions, UI preference synchronization, teardown bypass, force deletion, client-selected base/branch/path, remote merge resolve/abort, unsafe recovery permissions, directory listing, binary content, or file rename/delete.

#### Scenario: Capabilities report recovery boundary

**Given**: A client reads v2 capabilities
**When**: It inspects worktree operations
**Then**: Only list, detail, create, delete, merge, and text are advertised
**And**: Merge-conflict recovery is identified as local or TUI required
**And**: `text_edit` names the bounded limits and excluded paths
