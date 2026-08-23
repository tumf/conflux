## MODIFIED Requirements

### Requirement: Retry routing preserves reconciled evidence

Terminal error retry MUST use `ReducerCommand::RetryError`. Acceptance-stalled retry MUST reconcile the existing runtime hold and resume through the existing explicit acceptance retry path without rerunning apply. Unsupported, non-resumable, or identity-mismatched targets MUST retain their evidence. A settled terminal error carrying retained Apply iteration-limit evidence MUST be eligible for a later explicit individual, bulk, or Start-selected retry even while the persistent scheduler remains live. Bulk retry and Start-selected retry MUST dispatch accepted targets once and produce no scheduler effect when none remain.

An accepted terminal-error retry selected by Start MUST publish the same target-ID-bearing explicit-retry edge as an individual or bulk retry. Ordinary `AddToQueue`, generic scheduler notification, execution marks, and delayed mark settlement MUST NOT substitute for that edge or clear terminal error evidence. Retained Apply iteration-limit evidence MUST remain observational and MUST NOT block a new explicit retry boundary.

#### Scenario: Valid acceptance hold resumes acceptance

**Given**: A versioned acceptance hold matches repository, change, worktree, and apply revision evidence
**When**: The operator requests retry for that change
**Then**: The hold is consumed through explicit retry
**And**: Workspace preparation occurs
**And**: Processing resumes at acceptance rather than apply

#### Scenario: Individual retry starts a fresh boundary after Apply limit

**Given**: A terminal-error change retains typed Apply iteration-limit evidence from its settled invocation
**And**: The persistent scheduler remains live
**When**: An operator requests individual retry
**Then**: The service applies the ordinary terminal-error retry route exactly once
**And**: The retained error is consumed only by that explicit intent
**And**: The later invocation receives fresh Apply budget

#### Scenario: Bulk retry includes a settled Apply-limit target

**Given**: One requested terminal-error change retains settled Apply iteration-limit evidence
**And**: Other requested changes carry ordinary retryable terminal-error or resumable acceptance evidence
**When**: The operator requests bulk retry
**Then**: Every supported target is mutated and dispatched exactly once
**And**: The settled Apply-limit target enters a fresh execution boundary
**And**: Unrelated targets retain their independent retry routes

#### Scenario: No explicit retry produces no redispatch

**Given**: A terminal-error change retains typed Apply iteration-limit evidence from its settled invocation
**When**: Only queue reconciliation, generic scheduler notification, ordinary queue addition, or delayed mark settlement occurs
**Then**: The failed change is not retried
**And**: Its terminal error and diagnostic evidence remain intact

#### Scenario: Start-selected terminal error publishes one explicit-retry edge

**Given**: marked change `alpha` carries retry-eligible terminal Error evidence
**And**: Start admission selects retry routing for `alpha`
**When**: the prepared command commits
**Then**: the reducer applies `RetryError(alpha)` exactly once
**And**: one target-specific explicit-retry edge is published
**And**: the execution mark for `alpha` is restored
**And**: no ordinary queue-add hook or delayed mark-settlement admission substitutes for retry

#### Scenario: Start-selected unsupported retry preserves evidence

**Given**: a marked target is non-resumable, identity-mismatched, or otherwise unsupported
**When**: Start evaluates retry routing
**Then**: the target is refused or excluded with current evidence intact
**And**: no reducer, mark, queue, hook, retry-edge, notification, or scheduler-start effect occurs for that target

#### Scenario: Runtime-limit retry requires new operator intent

**Given**: an invocation produced typed runtime-limit termination and settled into terminal Error
**When**: no explicit operator retry has been accepted
**Then**: the scheduler MUST NOT redispatch the failed target from queue reconciliation or ordinary notification
**When**: a later Start request accepts the marked retry route
**Then**: the normal terminal-error retry transition and explicit-retry edge MAY release the target for analysis

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait when the reducer has not recorded archive completion for the target, and reject mark mutation in Error mode. A target with terminal display status or reducer-recorded archive completion MUST remain outside mark mutation in every mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons.

Every accepted individual, bulk, or API execution-mark mutation MUST add exactly the targets whose marks actually changed to one process-local settlement batch. After the existing stability window, settlement MUST re-read only those named targets from one coherent current snapshot and reconcile each target in both directions: a marked, tracked, parallel-eligible ordinary `not queued` target MUST gain queue intent; an unmarked ordinary pending target whose reducer queue intent is `Queued`, whose activity is idle, and which is outside active, in-flight, lane-wait, retry, MergeWait, ResolveWait, RejectWait, blocked, stalled, terminal, archive-complete, unknown-status, or otherwise ineligible states MUST lose queue intent. Targets not named by the mark-mutation batch MUST retain queue intent, including explicitly queued unmarked targets and marked targets explicitly removed from queue. Unknown or ambiguous status MUST fail closed with stable exclusion evidence.

Settlement-derived queue mutations MUST use an application-time guard under the authoritative reducer write boundary. A removal whose target became active, in-flight, waiting, terminal, or otherwise excluded MUST become a reasoned no-op and MUST NOT clear active lifecycle evidence. An addition whose target became terminal-error or otherwise excluded MUST become a reasoned no-op, MUST NOT route through `RetryError`, and MUST NOT publish an explicit-retry edge. Mark removal MUST NOT cancel, stop, dequeue, alter phase, or clear active lifecycle evidence.

A settled batch with one or more applied queue-membership mutations MUST notify the scheduler exactly once after all mutations; a batch with no applied membership mutation MUST NOT notify it. `on_queue_add` and `on_queue_remove` remain governed by their successful per-target mutation rules. Frontends and settlement MUST NOT start Analyze directly; the scheduler alone applies the capacity, candidate, edge, and runtime-signature rules in `parallel-execution`.

Bulk execution-mark classification MUST exclude a reducer-recorded archive-complete row before mutation, with stable reasons, choose one target state from the remaining eligible rows only, and update their marks atomically before the accepted mark deltas enter the common settlement batch. Retained Apply iteration-limit evidence on a settled terminal-error row MUST NOT add an exclusion of its own: the row MUST classify exactly as an ordinary terminal-error row does in the same request. An explicit per-target terminal-error queue addition that would route through `RetryError` remains explicit retry intent and MUST apply the same explicit-retry classification as individual retry, publishing the same target-specific explicit-retry edge when accepted.

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

#### Scenario: Settled limited row follows ordinary bulk classification

**Given**: A settled terminal-error row retaining typed Apply iteration-limit evidence and unrelated eligible rows exist in one bulk request
**When**: The service classifies and applies bulk execution marks
**Then**: The retained evidence adds no exclusion of its own
**And**: The row is classified exactly as an ordinary terminal-error row in the same mode
**And**: The remaining eligible rows still receive one coherent target state atomically

#### Scenario: Queue-intent alias retries a settled limited error explicitly

**Given**: A settled terminal-error change retains typed Apply iteration-limit evidence
**When**: A caller requests explicit per-target queue addition or `set_queue_intent=true` for that change
**Then**: The service applies the same explicit-retry classification as individual retry
**And**: An accepted alias applies `RetryError` exactly once and publishes one target-specific explicit-retry edge
**And**: The retained diagnostic remains observable and is consumed only by that explicit intent

#### Scenario: Mark settlement is scoped to changed targets

**Given**: An unmarked target was explicitly queued and another marked target was explicitly removed from queue
**When**: An unrelated individual, bulk, or API mark mutation settles
**Then**: Neither unrelated target's queue intent changes
**And**: Only targets named by accepted mark deltas are reconciled

#### Scenario: Unmark removes ordinary pending queue intent

**Given**: A named target is unmarked, idle, ordinary pending, and reducer-visible as `Queued`
**When**: Its mark-mutation batch settles
**Then**: Its queue intent becomes `NotQueued`
**And**: The TUI projects `not queued` without starting cancellation or dequeue

#### Scenario: Application-time guard preserves raced lifecycle evidence

**Given**: Settlement classified a queue addition or removal
**When**: The target becomes active, in-flight, waiting, terminal-error, terminal, or otherwise excluded before reducer application
**Then**: The settlement mutation is a reasoned no-op
**And**: It neither clears active evidence nor converts an addition into `RetryError` or an explicit-retry edge

#### Scenario: Settlement emits one scheduler notification

**Given**: One settled mark-mutation batch applies one or more queue membership changes
**When**: All target mutations finish
**Then**: The scheduler receives exactly one notification for the batch
**And**: A batch with zero applied membership changes emits none

<!-- Expected canonical result after archive: settled Apply-limit diagnostics remain observable but no longer prevent a later explicit retry from creating a fresh execution boundary, and mark/queue classification treats a settled limited row exactly as an ordinary terminal-error row. -->
