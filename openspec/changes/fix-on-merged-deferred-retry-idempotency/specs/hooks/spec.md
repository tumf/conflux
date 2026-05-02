## MODIFIED Requirements

### Requirement: on_merged hook

The orchestrator SHALL run `on_merged` after a change is successfully merged into the base branch and before the change transitions to terminal `Merged` status.

`on_merged` SHALL run only once for a successful merge of a given change, including immediate parallel merge success, deferred merge retry success, manual TUI resolve success, and conflictless merge-ready retry paths.

A stale retry or repeated scheduler trigger for a change already integrated into the base branch SHALL NOT execute `on_merged` again.

#### Scenario: deferred merge retry invokes on_merged once

**Given**: `hooks.on_merged` is configured
**And**: change `alpha` is in `ResolveWait` for a deferred merge retry
**When**: the scheduler retries the merge and repository-visible merge integration succeeds
**Then**: `on_merged` is executed once with `{change_id}=alpha`
**And**: `MergeCompleted` is emitted only after the hook execution attempt completes

#### Scenario: stale retry does not duplicate on_merged

**Given**: change `alpha` has already been integrated into the base branch
**And**: a stale retry trigger or stale resolve-wait entry for `alpha` is observed
**When**: the scheduler evaluates deferred merge retries
**Then**: it clears the stale retry intent for `alpha`
**And**: it does not execute `on_merged` for `alpha` again
**And**: it does not start AI conflict resolution for `alpha`

#### Scenario: repeated retry trigger after success is idempotent

**Given**: change `alpha` completed deferred merge retry successfully
**And**: `on_merged` already ran for that successful merge
**When**: a later scheduler loop synchronizes retry state and receives another retry dispatch trigger
**Then**: `alpha` is not re-added as retryable work
**And**: no second `on_merged` execution is emitted for `alpha`
