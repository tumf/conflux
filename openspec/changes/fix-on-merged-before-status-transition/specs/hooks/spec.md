## MODIFIED Requirements

### Requirement: on_merged hook

The orchestrator SHALL run `on_merged` after a change is successfully merged into the base branch and before the change transitions to terminal `Merged` status.

`on_merged` SHALL run only once for a successful merge of a given change, including immediate parallel merge success, deferred merge retry success, manual TUI resolve success, and conflictless merge-ready retry paths.

A stale retry or repeated scheduler trigger for a change already integrated into the base branch SHALL NOT execute `on_merged` again.

When `hooks.on_merged` is configured with `continue_on_failure=false`, `on_merged` SHALL act as a merged-transition gate. If the hook fails, times out, or exits non-zero, the orchestrator SHALL NOT emit `MergeCompleted` for that change and SHALL NOT transition the change to terminal `Merged` status.

#### Scenario: deferred merge retry hook failure blocks merged

**Given**: `hooks.on_merged` is configured with `continue_on_failure=false`
**And**: change `alpha` is in deferred merge retry
**And**: repository-visible merge integration succeeds
**When**: `on_merged` exits non-zero
**Then**: `MergeCompleted` is not emitted for `alpha`
**And**: `alpha` does not transition to terminal `Merged`
**And**: `alpha` remains in an operator-visible failure or blocking state that explains hook failure

#### Scenario: immediate parallel merge hook failure blocks merged

**Given**: `hooks.on_merged` is configured with `continue_on_failure=false`
**And**: change `alpha` merges successfully immediately after archive completion
**When**: `on_merged` exits non-zero
**Then**: `MergeCompleted` is not emitted for `alpha`
**And**: `alpha` does not transition to terminal `Merged`

#### Scenario: successful hook still permits merged transition

**Given**: `hooks.on_merged` is configured with `continue_on_failure=false`
**And**: change `alpha` is successfully merged into the base branch
**When**: `on_merged` completes successfully
**Then**: `MergeCompleted` is emitted only after hook completion
**And**: `alpha` may transition to terminal `Merged`

### Requirement: on_merged root repo lock diagnostics

For repo-mutating `on_merged` commands, the hook runner SHALL provide repository-verifiable diagnostics around root `.git/index.lock` waiting and execution readiness.

At minimum, the logs SHALL make it observable whether root `.git/index.lock` was already present before hook execution, whether it was released during the configured wait window, or whether execution proceeded after timeout.

These diagnostics are observational only and MUST NOT introduce hidden out-of-worktree durable workflow-control state.

#### Scenario: pre-existing root lock is logged before hook execution

**Given**: root `.git/index.lock` exists before `on_merged` starts
**When**: the hook runner prepares `on_merged`
**Then**: the logs indicate that root lock waiting began
**And**: the logs later indicate whether the lock was released or the wait timed out

#### Scenario: timeout does not hide unsafe execution context

**Given**: root `.git/index.lock` remains present until `index_lock_wait_secs` expires
**When**: the hook runner proceeds to execute `on_merged`
**Then**: the logs explicitly indicate that execution continued after lock wait timeout
**And**: a later hook failure can be correlated with the unsafe root lock condition from repository-verifiable logs
