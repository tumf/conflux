## MODIFIED Requirements

### Requirement: post-merge success path honors on_merged gate

Parallel merge success paths SHALL treat non-continuable `on_merged` execution as part of merged-success completion rather than as a warning-only side effect.

When repository-visible merge success is followed by `on_merged` with `continue_on_failure=false`, the scheduler SHALL emit `MergeCompleted` only if the hook attempt completes successfully. A failing `on_merged` MUST block terminal merged transition for that change.

#### Scenario: deferred retry success does not emit MergeCompleted after hook failure

**Given**: change `alpha` succeeds through deferred merge retry
**And**: `hooks.on_merged` is configured with `continue_on_failure=false`
**When**: `on_merged` fails after merge integration but before final status transition
**Then**: the scheduler does not emit `MergeCompleted` for `alpha`
**And**: reducer-owned merged success is not recorded

#### Scenario: immediate merge success does not emit MergeCompleted after hook failure

**Given**: change `alpha` succeeds through immediate archive-followed merge handling
**And**: `hooks.on_merged` is configured with `continue_on_failure=false`
**When**: `on_merged` fails after merge integration but before final status transition
**Then**: the scheduler does not emit `MergeCompleted` for `alpha`
**And**: UI and reducer state do not claim `alpha` is merged

### Requirement: root-repo lock contention remains non-terminal

If `on_merged` fails because the root repository is not safe for repo-mutating hook execution, such as root `.git/index.lock` contention, Conflux SHALL treat that as a hook failure that blocks merged transition when `continue_on_failure=false`.

#### Scenario: root index lock contention blocks merged transition

**Given**: change `alpha` is repository-visible merged
**And**: `hooks.on_merged` runs a repo-mutating command such as `make bump-patch`
**And**: root `.git/index.lock` contention causes the hook to exit non-zero
**When**: the scheduler handles hook completion
**Then**: `alpha` does not transition to terminal `Merged`
**And**: `MergeCompleted` is not emitted for `alpha`
**And**: the operator-visible failure context includes the hook failure details
