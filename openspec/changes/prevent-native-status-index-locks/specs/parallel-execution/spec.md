## MODIFIED Requirements

### Requirement: Repository monitoring avoids optional Git index locks

Every Conflux-owned native read-only `git status` observation used for repository or managed-worktree classification MUST preserve its existing output and classification contract without requesting an optional Git index lock. This includes change-list monitoring, dirty/clean checks, untrimmed porcelain reads, Apply and Archive phase classification, merge precondition observation, upstream cleanliness, and porcelain-v2 failure classification.

Optional-lock suppression MUST be expressed as the child command's Git global `--no-optional-locks` option before the `status` subcommand. Conflux MUST NOT implement this policy through process-wide `GIT_OPTIONAL_LOCKS` state, MUST NOT apply it to index-mutating Git commands, and MUST NOT delete or bypass an existing lock.

Each observation MUST retain its existing porcelain version, status-column fidelity, explicit untracked/ignored behavior, pathspec scope, and error mapping. Results MAY represent a point-in-time observation and MAY converge on concurrent mutations during a later poll.

<!-- Expected canonical result after archive: all Conflux-owned native read-only status observations, not only change-list refresh, avoid optional index writes while preserving their existing classification contracts. -->

#### Scenario: Periodic root observation does not contend with an authorized commit

**Given**: A running Conflux frontend periodically checks root repository dirty or clean state
**And**: An `on_merged` hook, release command, or operator may stage and commit in the same repository
**When**: Conflux executes its native read-only Git status observation
**Then**: The child command passes `--no-optional-locks` before `status`
**And**: The observation does not request an optional index lock
**And**: The authorized mutating Git command retains normal lock behavior

#### Scenario: Managed-worktree phase classification preserves status semantics

**Given**: A managed worktree contains staged, unstaged, deleted, renamed, untracked, ignored, or conflicted state
**When**: Apply, Archive, merge, or resume classification reads native Git status
**Then**: The existing clean, dirty, staged, unstaged, untracked, ignored, and conflict classifications are unchanged
**And**: An untrimmed porcelain caller retains both status columns on the first line
**And**: The read-only child command does not request an optional index lock

#### Scenario: Upstream porcelain-v2 observation remains machine-readable

**Given**: Upstream integration needs working-tree cleanliness or post-failure porcelain-v2 evidence
**When**: The native upstream Git adapter runs its status observation
**Then**: The child command disables optional locks before `status`
**And**: `--porcelain=v2` output remains porcelain v2
**And**: Existing upstream error mapping and routing classification remain unchanged

#### Scenario: Read-only status does not persist an optional index refresh

**Given**: A repository fixture has stale index stat information that a normal status command demonstrably persists
**When**: Representative Conflux production status paths observe current repository state
**Then**: They return the current clean or dirty evidence required by their callers
**And**: The complete Git index bytes remain unchanged

#### Scenario: Optional-lock suppression stays command-local

**Given**: The Conflux process also executes add, commit, merge, reset, checkout, tag, push, or release publication commands
**When**: Read-only status observations and mutating commands run in the same process
**Then**: Only the read-only status children receive `--no-optional-locks`
**And**: Process-wide `GIT_OPTIONAL_LOCKS` is not set by this policy
**And**: Mutating commands retain Git's normal lock semantics
