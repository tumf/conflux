## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting

Parallel analysis MUST target only changes made eligible by explicit invocation targets or current reducer-owned intent. Initial explicit targets from TUI, CLI, or remote Start are eligible scheduler inputs. After startup, ordinary queue reconciliation MUST derive eligibility from current reducer `QueueIntent::Queued`; dynamic queue notifications are wake-up hints only.

Repository-wide worktree discovery, active-change catalog refresh, `ChangesRefreshed`, and workspace observation MUST NOT create ordinary queue intent or append unrelated IDs to scheduler-local queued work or dependency analysis. For an already eligible ID that is absent from the active catalog, the scheduler MAY inspect that ID's preserved workspace and reconstruct an archived-dirty repair candidate. Repository evidence determines the resume phase but MUST NOT create execution intent.

`RemoveFromQueue` and `DequeueChange` revoke ordinary eligibility. A preserved worktree or stale local/dynamic entry MUST NOT reacquire the change until accepted explicit requeue or retry restores reducer queued intent. Reducer-owned `ResolveWait` and `RejectWait` remain independently scheduler-consumable lane intent. Final terminal and terminal-error stop gates remain independently enforced after eligibility evaluation.

#### Scenario: Catalog refresh does not admit unselected archived-dirty work

**Given**: A parallel run has initial explicit target `fresh`
**And**: Preserved worktree `stale` is archived-dirty and not merged
**And**: `stale` has no queue or lane-wait intent
**When**: `ChangesRefreshed` registers both `fresh` and `stale`
**And**: Queue reconciliation scans catalog and worktree state
**Then**: `stale` is not added to scheduler-local queued work or dependency analysis
**And**: Apply, acceptance, archive finalization, resolve, reject, and merge do not start for `stale`
**And**: `stale` repository and worktree evidence remains unchanged

#### Scenario: Explicit target recovers archived-dirty workspace

**Given**: Archived-dirty `stale` is an initial explicit TUI, CLI, or remote target
**And**: It is absent from the active change catalog because its change directory was moved into the workspace archive
**When**: The scheduler resolves initial candidates
**Then**: It may inspect only `stale`'s preserved workspace and reconstruct repair work
**And**: It resumes the repository-derived archive-finalization or archive-complete phase
**And**: It does not rerun completed apply work

#### Scenario: Reducer queued intent recovers archived-dirty workspace

**Given**: Archived-dirty `stale` was not an initial target
**And**: An accepted queue addition or terminal-error retry sets `QueueIntent::Queued` for `stale`
**When**: Queue reconciliation cannot load `stale` from the active catalog
**Then**: It may inspect `stale`'s preserved workspace and reconstruct repair work
**And**: Recovery uses current workspace, Git, and base-tree evidence

#### Scenario: Queue revocation prevents worktree reacquisition

**Given**: `stale` was previously eligible and has a preserved archived-dirty worktree
**When**: `RemoveFromQueue` or `DequeueChange` revokes its ordinary queue eligibility
**And**: Later reconciliation observes stale local entries or the preserved worktree
**Then**: `stale` is not re-added to ordinary queued work
**And**: A later accepted explicit requeue is required before ordinary recovery can resume

#### Scenario: Lane waits remain distinct from ordinary recovery

**Given**: The ordinary queued set is empty
**And**: The reducer owns `ResolveWait` for one change or `RejectWait` for another
**When**: The base-mutating lane becomes available
**Then**: The scheduler consumes the matching lane intent
**And**: It does not require or synthesize ordinary queued intent

#### Scenario: Unrequested residue does not prevent drain

**Given**: No ordinary queued, active, resolve-wait, or reject-wait intent remains
**And**: An unrequested archived-dirty worktree exists
**When**: The scheduler evaluates completion
**Then**: The unrequested worktree is not treated as current-run work
**And**: The run may drain or complete

#### Scenario: Eligible merged and terminal-error changes retain independent stop gates

**Given**: Explicit intent makes change `alpha` visible to candidate evaluation
**And**: Repository or reducer evidence classifies `alpha` as merged or terminal error
**When**: Dispatch eligibility is evaluated
**Then**: Merged evidence prevents ordinary dispatch permanently
**And**: Terminal error prevents ordinary dispatch until accepted `RetryError`
