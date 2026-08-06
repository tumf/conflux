## MODIFIED Requirements

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait, and reject mark mutation in Error mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons. Bulk execution-mark classification MUST exclude an active-run-limited terminal-error row with `apply_iteration_limit_active` before mutation, choose one target state from the remaining eligible rows only, and update their marks plus Running queue intent atomically. A terminal-error queue addition that would route through `RetryError` MUST consult the same active typed Apply iteration-limit eligibility before changing reducer state, marks, queue state, hooks, or explicit-retry edges; while limited it MUST be rejected with the same stable reason as explicit retry.

#### Scenario: Eligibility refresh cleans invalid intent

**Given**: Marked or queued changes become ineligible under current repository and worktree evidence
**When**: The operator service applies catalog refresh or eligibility re-evaluation
**Then**: It clears those execution marks and queue presentation atomically
**And**: The outcome identifies each excluded change and reason

#### Scenario: Bulk mark updates one coherent target set

**Given**: Eligible and excluded changes exist in one admitted state
**When**: The operator requests bulk execution-mark mutation
**Then**: The service derives one target mark from eligible changes only
**And**: It updates eligible marks and Running queue intent atomically
**And**: Excluded changes retain coherent intent and receive stable reasons

#### Scenario: Bulk mark excludes active limited queue aliases before mutation

**Given**: An active-run-limited terminal-error row and unrelated eligible rows exist in one Running-mode bulk request
**When**: The service classifies and applies bulk execution marks
**Then**: It excludes the limited row with `apply_iteration_limit_active`
**And**: The limited row's mark and queue intent remain unchanged
**And**: It atomically applies one coherent target state and queue intent to the remaining eligible rows
**And**: The terminal-error alias guard cannot abort a partially applied bulk operation

#### Scenario: Queue intent cannot alias an active limited retry

**Given**: A terminal-error change carries typed Apply iteration-limit evidence owned by the active run
**When**: A caller requests queue addition or `set_queue_intent=true`
**Then**: The service rejects the request with `apply_iteration_limit_active`
**And**: It does not apply `RetryError` or clear the retained error
**And**: It does not change marks, dynamic queue, explicit-retry edges, hooks, or scheduler state

### Requirement: Retry routing preserves reconciled evidence

Terminal error retry MUST use `ReducerCommand::RetryError`. Acceptance-stalled retry MUST reconcile the existing runtime hold and resume through the existing explicit acceptance retry path without rerunning apply. Before either route mutates state, the shared service MUST reject a target carrying typed Apply iteration-limit evidence owned by the active run. Unsupported, non-resumable, identity-mismatched, or active-run-limited targets MUST retain their evidence. Bulk retry MUST exclude such targets, dispatch other accepted targets once, and produce no scheduler effect when none remain.

#### Scenario: Valid acceptance hold resumes acceptance

**Given**: A versioned acceptance hold matches repository, change, worktree, and apply revision evidence
**When**: The operator requests retry for that change
**Then**: The hold is consumed through explicit retry
**And**: Workspace preparation occurs
**And**: Processing resumes at acceptance rather than apply

#### Scenario: Individual active-limit retry is mutation-free

**Given**: A terminal-error change carries typed Apply iteration-limit evidence owned by the active run
**When**: An operator requests individual retry
**Then**: The service rejects it with `apply_iteration_limit_active`
**And**: Reducer status, error detail, failed classification, execution mark, queue contents, and explicit-retry publications remain unchanged
**And**: No queue hook, scheduler notification, or scheduler start occurs

#### Scenario: Bulk retry skips only active limited targets

**Given**: One requested change is limited by its active run
**And**: Other requested changes carry ordinary retryable terminal-error or resumable acceptance evidence
**When**: The operator requests bulk retry
**Then**: The limited change retains all state and is not reported as accepted
**And**: Its `apply_iteration_limit_active` reason remains readable in the authoritative snapshot at the result revision
**And**: The other retryable changes are mutated and dispatched exactly once

#### Scenario: All-limited bulk retry is a no-op

**Given**: Every candidate in a bulk retry carries active-run Apply iteration-limit evidence
**When**: The operator requests bulk retry
**Then**: The service returns a no-op with no retryable target
**And**: No reducer, mark, queue, hook, explicit-retry, notification, or scheduler-start effect occurs

#### Scenario: Later boundary uses ordinary retry routing

**Given**: The boundary that owned typed iteration-limit evidence has closed and retired its gate
**And**: Workspace evidence still classifies the change as retryable
**When**: An operator requests retry
**Then**: The service applies the ordinary reconciled retry route
**And**: A new scheduler boundary may start with fresh active-run state
