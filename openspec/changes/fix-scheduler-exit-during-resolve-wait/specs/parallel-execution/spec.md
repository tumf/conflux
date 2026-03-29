## MODIFIED Requirements

### Requirement: Scheduler Loop Termination

The scheduler loop SHALL NOT terminate while any change is in ResolveWait state (auto-resumable merge deferred) or while a manual resolve is actively running.

The scheduler loop SHALL terminate when all of the following conditions are met:
- `queued` changes list is empty
- `in_flight` changes set is empty
- `resolve_wait_changes` set is empty (no auto-resumable deferred merges pending)
- Manual resolve counter is zero (no resolve commands actively executing)
- `join_set` is empty (no spawned tasks running)

Changes in MergeWait state (requiring user intervention) SHALL NOT prevent scheduler loop termination.

#### Scenario: ResolveWait prevents scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in ResolveWait state (auto_resumable merge deferred)
**And**: The queued list and in_flight set are empty
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL continue running
**And**: Dynamic queue notifications SHALL be processed (new changes can be analyzed and dispatched)

#### Scenario: MergeWait does not prevent scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in MergeWait state (requires user intervention)
**And**: No changes are in ResolveWait state
**And**: Manual resolve counter is zero
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL terminate and send AllCompleted

#### Scenario: Queue addition during ResolveWait triggers analysis

**Given**: The scheduler loop is running with one change in ResolveWait
**And**: Run slots are available (in_flight + resolve count < max_parallelism)
**When**: A new change is added to the dynamic queue
**Then**: The scheduler SHALL analyze and dispatch the new change

### Requirement: Merge Deferred State Separation

The parallel executor SHALL maintain two separate sets for tracking deferred merge changes:

- `resolve_wait_changes`: Changes with auto-resumable deferral reasons (e.g., another merge in progress). These are considered "in progress" and keep the scheduler alive.
- `merge_wait_changes`: Changes requiring user intervention (e.g., uncommitted changes on base). These are considered "suspended" and do not keep the scheduler alive.

When a `MergeAttempt::Deferred` result is received, the change SHALL be added to `resolve_wait_changes` if `auto_resumable` is true, or `merge_wait_changes` if `auto_resumable` is false.

The `retry_deferred_merges` method SHALL only retry changes in `resolve_wait_changes`. If a retry results in a non-auto-resumable deferral, the change SHALL be moved from `resolve_wait_changes` to `merge_wait_changes`.

#### Scenario: Auto-resumable deferral tracked as resolve_wait

**Given**: A change completes apply and archive successfully
**When**: The merge attempt returns `Deferred` with `auto_resumable=true`
**Then**: The change is added to `resolve_wait_changes`
**And**: The scheduler loop does not terminate

#### Scenario: Manual-intervention deferral tracked as merge_wait

**Given**: A change completes apply and archive successfully
**When**: The merge attempt returns `Deferred` with `auto_resumable=false`
**Then**: The change is added to `merge_wait_changes`
**And**: The scheduler loop may terminate if no other active work remains
