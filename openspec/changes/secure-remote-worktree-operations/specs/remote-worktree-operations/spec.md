## ADDED Requirements

### Requirement: Opaque worktree mutation identity

Every observed worktree MUST receive a random process-local 128-bit hexadecimal `worktree_id`. V2 mutation commands MUST target only this ID and MUST NOT accept absolute paths or branch names as mutation identity.

#### Scenario: Recreated path receives a new identity

**Given**: A worktree ID was allocated and its resource was removed
**When**: A worktree is later created at the same filesystem path
**Then**: The retired ID is not reused
**And**: A new random worktree ID is allocated

### Requirement: Redacted worktree observations

V2 worktree responses MUST expose repository-relative display paths and a 16-character hexadecimal repository correlation ID derived by FNV-1a 64-bit from canonical repository identity. They MUST NOT expose canonical or absolute repository roots.

#### Scenario: Worktree list does not disclose root

**Given**: A repository root and managed worktrees exist
**When**: A client lists v2 worktrees
**Then**: Paths are repository-relative
**And**: The canonical absolute repository root is absent from the response

### Requirement: Fail-closed dirty state

Dirty observation MUST support true, false, and unknown. Unknown MUST serialize as `null` and MUST make delete ineligible.

#### Scenario: Dirty check failure blocks delete

**Given**: The server cannot determine whether a worktree is dirty
**When**: A client requests deletion
**Then**: The command is rejected with conflict
**And**: The worktree is not removed

### Requirement: Restricted worktree command set

V2 MUST expose only managed change-worktree creation, deletion, and base merge. It MUST NOT expose arbitrary worktree commands, editor launch, temporary sessions, UI preference synchronization, teardown bypass, force deletion, or unsafe recovery permissions.

#### Scenario: Generic command is unsupported

**Given**: A client can authenticate to v2
**When**: It submits a generic shell/worktree command type
**Then**: Schema validation rejects the command
**And**: No process is launched

### Requirement: Managed remote worktree deletion

Remote worktree deletion MUST require an idempotency key and expected revision, resolve the current opaque ID, pass existing eligibility guards, and complete managed teardown before removing the resource. Failure MUST retain the resource and its identity binding.

#### Scenario: Successful delete retires identity after teardown

**Given**: A known clean eligible worktree and matching revision
**When**: An authenticated client submits delete with an idempotency key
**Then**: Managed teardown completes
**And**: The worktree is removed
**And**: Its opaque ID is retired

### Requirement: Shared conflict-preserving worktree merge

Remote merge MUST use the same base merge, root/base guard, `on_merged` hook, reducer transition, and event path as TUI. A Git conflict MUST retain intermediate repository state and return conflict file evidence; the operation MUST NOT automatically abort the merge.

#### Scenario: Conflict remains available for resolution

**Given**: A current eligible worktree whose branch conflicts with base
**When**: An authenticated client submits merge with matching revision and idempotency key
**Then**: The command fails with conflict evidence
**And**: The intermediate Git merge state is retained
**And**: `on_merged` is not executed

#### Scenario: Successful merge runs shared completion effects

**Given**: A current eligible worktree whose branch merges cleanly
**When**: The merge command completes
**Then**: The base contains the worktree result
**And**: `on_merged` runs exactly once
**And**: Shared reducer and event state report completion

### Requirement: Authenticated and concurrency-safe worktree mutations

Every v2 worktree mutation MUST require bearer authentication when v2 protection is active and MUST use the v2 idempotency and revision contracts. Root-busy, stale, missing, and ineligible targets MUST fail without mutation.

#### Scenario: Stale worktree delete does not target changed state

**Given**: A client observed a deletable worktree at an earlier revision
**When**: It submits delete after state revision changed
**Then**: The server returns HTTP 409
**And**: It does not run teardown or remove the worktree
