## MODIFIED Requirements

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait when the reducer has not recorded archive completion for the target, and reject mark mutation in Error mode. A target with terminal display status or reducer-recorded archive completion MUST remain outside mark mutation in every mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons. Bulk execution-mark classification MUST exclude a reducer-recorded archive-complete row and an active-run-limited terminal-error row before mutation, with stable reasons, choose one target state from the remaining eligible rows only, and update their marks plus Running queue intent atomically. A terminal-error queue addition that would route through `RetryError` MUST consult the same active typed Apply iteration-limit eligibility before changing reducer state, marks, queue state, hooks, or explicit-retry edges; while limited it MUST be rejected with the same stable reason as explicit retry.

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

#### Scenario: Archive-complete wait target does not admit invisible mark intent

**Given**: A MergeWait or ResolveWait target has reducer-recorded archive completion
**When**: A single-row or bulk execution-mark request classifies the target
**Then**: The target is excluded with a stable archive-complete reason
**And**: Its execution mark, queue intent, retry/resolve state, hooks, and scheduler state remain unchanged

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

<!-- Expected canonical result after archive: `operator-command-execution` will preserve wait-state mark intent only until reducer-recorded archive completion and will exclude post-archive rows before atomic single or bulk mutation. -->
