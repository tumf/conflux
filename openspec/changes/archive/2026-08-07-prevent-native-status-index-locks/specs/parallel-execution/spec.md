## MODIFIED Requirements

### Requirement: Repository monitoring avoids optional Git index locks

Every Conflux-owned native read-only `git status` observation used for repository or managed-worktree classification or context capture MUST preserve its existing output and classification contract without requesting an optional Git index lock. This includes change-list monitoring, dirty/clean checks, human-readable conflict-resolution context, untrimmed porcelain reads, Apply and Archive phase classification, merge precondition observation, upstream cleanliness, and porcelain-v2 failure classification.

Optional-lock suppression MUST be expressed as the child command's Git global `--no-optional-locks` option before the `status` subcommand. Conflux MUST NOT implement this policy through process-wide `GIT_OPTIONAL_LOCKS` state, MUST NOT apply it to index-mutating Git commands, and MUST NOT delete or bypass an existing lock.

Each observation MUST retain its existing porcelain version, human-readable or machine-readable output shape, status-column fidelity, explicit untracked/ignored behavior, pathspec scope, and error mapping.

The monitoring query MUST continue to classify active change paths from staged, unstaged, renamed, and untracked state while excluding clean committed paths, archive entries, hidden change directories, ignored files, and unrelated repository paths. Results MAY represent a point-in-time observation and MAY converge on concurrent mutations during a later poll.

<!-- Expected canonical result after archive: all Conflux-owned native read-only status observations, not only change-list refresh, avoid optional index writes while preserving their existing classification contracts. -->

#### Scenario: Periodic refresh does not request an optional index lock

**Given**: A TUI refresh is classifying uncommitted OpenSpec changes in the root repository
**And**: A lifecycle hook may stage and commit files in the same repository
**When**: The refresh executes its Git status query
**Then**: The query disables Git optional locks for that child command
**And**: Repo-mutating Git commands retain their existing lock behavior

#### Scenario: Periodic root observation does not contend with an authorized commit

**Given**: A running Conflux frontend periodically checks root repository dirty or clean state
**And**: An `on_merged` hook, release command, or operator may stage and commit in the same repository
**When**: Conflux executes its native read-only Git status observation
**Then**: The child command passes `--no-optional-locks` before `status`
**And**: The observation does not request an optional index lock
**And**: The authorized mutating Git command retains normal lock behavior

#### Scenario: Active change classifications remain visible

**Given**: Active change paths include staged or unstaged additions, modifications, deletions, an untracked file, and a rename within the same change
**When**: Conflux classifies change IDs with uncommitted files
**Then**: The affected active change IDs are returned
**And**: A clean committed path is not returned as an uncommitted change

#### Scenario: Monitoring exclusions remain stable

**Given**: Paths exist under an active change, the archive directory, a hidden change directory, an ignored path, and an unrelated repository directory
**When**: Conflux classifies change IDs with uncommitted files
**Then**: Only the qualifying active change ID is returned

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

#### Scenario: Conflict-resolution context remains human-readable

**Given**: Conflict resolution captures native Git status text for a resolve prompt
**When**: Conflux obtains that context from the Git backend
**Then**: The child command disables optional locks before `status`
**And**: The prompt receives the same human-readable status content and error behavior as before

#### Scenario: Monitoring does not persist an optional index refresh

**Given**: A repository fixture has index stat information that a normal status command demonstrably persists
**When**: Conflux runs the uncommitted-change monitoring query
**Then**: The query reports current working-tree changes
**And**: The complete Git index bytes remain unchanged

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
