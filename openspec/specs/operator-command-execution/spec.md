## Purpose

Define a single process-local operator command service so that TUI and remote frontends produce identical orchestration transitions, side effects, and events for the same operator intent.

## Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application coordinator. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use one process-lifetime authoritative `EventSink` dispatch boundary shared by runner-local and orchestration-run producers, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each ordinary new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit plus authoritative outcome dispatch, exact revision capture, and scheduler preparation. Activity enabled by the command's later activation or wake MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation MUST leave no reducer, execution-mark, queue, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect. A command awaiting confirmed runtime termination MUST use the same coordinator's two-phase protocol and MUST NOT hold the application gate, authoritative dispatch transaction, or TUI event loop during that wait.

Start admission MUST apply the complete-request worktree eligibility fence to the full marked set, then classify current marked target evidence instead of using process mode as a proxy for retry eligibility. In Select and Stopped, ordinary startable targets MUST take priority and retry-only targets MUST be excluded rather than mixed into the same launch; when no ordinary target is startable, marked retry routes MAY be selected. In Running, Start MUST accept only marked retry routes and MUST NOT replace live ordinary mark settlement. Error MUST retain marked retry routing, and Stopping MUST refuse Start. Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent. All transaction, mode, mark, and reservation coordination MUST remain process-local and MUST NOT become durable workflow authority.

#### Scenario: TUI and remote Start create equivalent targets

**Given**: TUI and remote frontends have the same process-local marks and lifecycle state
**When**: Each requests Start through the shared application transaction
**Then**: Both produce the same explicit target IDs and ordinary or retry intent
**And**: Unmarked catalog or worktree entries are excluded
**And**: the accepted outcome is dispatched before the scheduler can emit progress
**And**: the scheduler activates or wakes exactly once

#### Scenario: Empty Start has no partial effect

**Given**: No marked change is startable through the route permitted by current mode and evidence
**When**: TUI or remote Start enters the shared application transaction
**Then**: the outcome is no-op or failed rather than succeeded
**And**: process mode, reducer state, marks, queue, explicit-retry edges, scheduler, and frontend projection remain unchanged

#### Scenario: Retry dispatches evidence-aware work atomically

**Given**: A target carries terminal Error or a resumable acceptance hold accepted by existing retry classification
**When**: TUI or remote retry is accepted
**Then**: the same retry route, mark, queue intent, and explicit-retry semantics are committed
**And**: the accepted outcome is dispatched before one scheduler activation or wake
**And**: unsupported or failed dispatch preserves the original target evidence and every pre-command side effect count

#### Scenario: Resolve reservation and projection are coherent

**Given**: A valid merge-wait target can reserve the single resolver
**When**: TUI or remote resolve is accepted
**Then**: an active reservation projects resolve-pending, `is_resolving=true`, and Running in one outcome dispatch
**And**: scheduling starts or wakes exactly once after that dispatch
**And**: a queued reservation remains FIFO without a second scheduler dispatch
**And**: duplicate reservation is a no-op

#### Scenario: Lifecycle controls share one mode authority

**Given**: TUI and remote adapters address the same running process
**When**: stop, cancel-stop, or force-stop is requested
**Then**: the shared transaction validates one Core-owned process mode and one safe-boundary classification
**And**: accepted stop projects Stopping, accepted cancel-stop projects Running, force-stop waiting for cleanup projects Stopping, and settled force-stop emits Stopped
**And**: an invalid-mode request changes neither stop flags nor projection

#### Scenario: Lifecycle events keep Core mode admissible

**Given**: Core mode entered Running for an accepted run
**When**: typed run activation such as `ProcessingStarted`, authoritative `Stopping`, `Stopped`, global `Error`, guarded `AllCompleted`, or a typed persistent-idle Ready event is dispatched
**Then**: the same Core mode and every frontend projection apply that lifecycle transition
**And**: natural completion returns to Select and admits a later Start
**And**: Stopped admits resume and Error retains explicit retry semantics

#### Scenario: Confirmed termination does not monopolize admission

**Given**: stop-and-dequeue has issued cancellation and is waiting for confirmed termination
**When**: another valid force-stop or unrelated operator command is submitted
**Then**: the second command can execute and settle without waiting for the dequeue timeout
**And**: TUI rendering and authoritative event fan-out remain live
**And**: stop-and-dequeue revalidates the target after reacquiring the gate before it commits
**And**: exact replay does not issue cancellation or start another waiter
**And**: timeout or failed revalidation commits no dequeue reducer, event, or projection effect

#### Scenario: Scheduler preparation failure rolls back staged intent

**Given**: Start, retry, or active resolve has valid targets but scheduler preparation fails
**When**: the shared application transaction returns failure
**Then**: reducer status, marks, dynamic queue, explicit-retry edges, resolve reservations, process mode, hooks, and frontend projection equal their pre-command state
**And**: no scheduler event is emitted

#### Scenario: CLI explicit targets use the shared scheduler contract

**Given**: CLI run explicitly targets `alpha`
**And**: Unrelated preserved worktree `beta` exists
**When**: The parallel scheduler starts
**Then**: `alpha` is an eligible initial target
**And**: `beta` is not admitted by worktree discovery

#### Scenario: Refresh does not create operator intent

**Given**: `alpha` has no execution mark, queue intent, retry intent, or lane-wait intent
**When**: A frontend refreshes the active change catalog or workspace observations
**Then**: `alpha` may become visible in snapshots
**And**: It does not become eligible for ordinary execution

#### Scenario: TUI and remote Start create equivalent retry targets outside Error mode

**Given**: TUI and remote frontends have the same process-local marks, change-local retry evidence, and lifecycle state
**When**: Each requests Start through the shared application transaction in Select, Running, or Stopped
**Then**: Both produce the same explicit retry target IDs, exclusions, reducer transitions, and scheduler effect
**And**: the accepted outcome projects the same revision before scheduler activity
**And**: neither frontend derives retry eligibility from its row cache or presentation mode

#### Scenario: Ordinary and retry targets are not mixed

**Given**: Select or Stopped has at least one marked ordinary startable target and at least one marked retry-only target
**When**: Start enters the shared application transaction
**Then**: the ordinary targets are admitted with ordinary Start semantics
**And**: retry-only targets are excluded with target-specific detail explaining that ordinary marks must be removed before retry-class Start can select them
**And**: no retry reducer command or explicit-retry edge is emitted for the excluded targets

#### Scenario: Running Start accepts retry only

**Given**: A scheduler is live in Running mode
**And**: marked targets contain retry-eligible change-local error `alpha` and ordinary `not queued` change `beta`
**When**: Start enters the shared application transaction
**Then**: only `alpha` MAY be accepted through explicit retry
**And**: `beta` remains governed by the existing live mark-settlement path
**And**: the scheduler is notified exactly once after the accepted outcome is dispatched

#### Scenario: Worktree-invalid retry-class Start is rejected atomically

**Given**: Marks include a worktree-ineligible retry-eligible target
**When**: Start evaluates ordinary or retry-class admission
**Then**: the complete request is rejected before class selection
**And**: no reducer, mark, queue, explicit-retry edge, scheduler, mode, hook, or projection effect occurs

<!-- Expected canonical result after archive: the shared Start transaction will choose ordinary or retry intent from current target evidence with explicit mode and worktree guards, preserving TUI/API parity and fail-atomic ordering. -->

### Requirement: Execution intent remains non-authoritative and process-local

Execution marks MUST remain distinct from queue intent, activity, hold state, terminal state, and display status. They MUST reset to false on process restart and MUST NOT become durable workflow-control evidence.

#### Scenario: Restart clears marks without changing workflow routing

**Given**: A process has marked changes and repository/worktree evidence is unchanged
**When**: The process restarts
**Then**: Every execution mark is false
**And**: The next workflow action remains derived from workspace and Git evidence

### Requirement: Mode-aware mark and queue behavior

The service MUST allow execution-mark mutation in Select and Stopped modes, resolve accepted marks into initial targets at Start, use reducer queue intent for ordinary Running additions, allow mark-only mutation for MergeWait and ResolveWait when the reducer has not recorded archive completion for the target, and reject mark mutation in Error mode. A target with terminal display status or reducer-recorded archive completion MUST remain outside mark mutation in every mode. Queue removal and successful stop-and-dequeue MUST revoke ordinary execution eligibility until explicit requeue or retry. Catalog refresh or eligibility re-evaluation MUST classify one coherent state, clear marks and queue presentation for changes that became ineligible, and report stable exclusion reasons.

Every accepted individual, bulk, or API execution-mark mutation MUST add exactly the targets whose marks actually changed to one process-local settlement batch. After the existing stability window, settlement MUST re-read only those named targets from one coherent current snapshot and reconcile each target in both directions: a marked, tracked, parallel-eligible ordinary `not queued` target MUST gain queue intent; an unmarked ordinary pending target whose reducer queue intent is `Queued`, whose activity is idle, and which is outside active, in-flight, lane-wait, retry, MergeWait, ResolveWait, RejectWait, blocked, stalled, terminal, archive-complete, unknown-status, or otherwise ineligible states MUST lose queue intent. Targets not named by the mark-mutation batch MUST retain queue intent, including explicitly queued unmarked targets and marked targets explicitly removed from queue. Unknown or ambiguous status MUST fail closed with stable exclusion evidence.

A command-capable owner whose persistent scheduler is live MUST retain the process-local settlement runtime binding and deadline task until each accepted changed-target batch either reconciles or produces stable observable exclusion evidence. Existing unrelated active work MUST NOT make an ordinary eligible marked target remain silently `not queued`. Frontends MUST NOT require or synthesize another Start to recover such a target. Failure to bind, arm, spawn, upgrade, or execute the settlement runtime MUST be observable with a stable reason.

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

#### Scenario: Running owner settles a newly marked target without another Start

**Given**: A command-capable persistent owner is already running one change and its scheduler remains live
**And**: Another tracked, parallel-eligible ordinary target is `not queued`
**When**: TUI, client, or API accepts a changed execution mark for that target and the stability window expires
**Then**: The target gains reducer queue intent without another Start
**And**: The scheduler receives exactly one reanalysis notification for the applied batch
**And**: The unrelated active change and unrelated queue intent remain unchanged

#### Scenario: Settlement lifecycle failure is observable

**Given**: A command-capable owner reports a live persistent scheduler and accepts a changed execution mark
**When**: Settlement cannot bind, arm, spawn, upgrade, or execute its runtime before reconciliation
**Then**: The owner exposes a stable reason for the incomplete settlement
**And**: It does not silently present the target as marked and `not queued` indefinitely
**And**: It does not synthesize Start, Retry, cancellation, or dequeue

<!-- Expected canonical result after archive: settled Apply-limit diagnostics remain observable but no longer prevent a later explicit retry from creating a fresh execution boundary, and mark/queue classification treats a settled limited row exactly as an ordinary terminal-error row. -->

### Requirement: Cancellation precedes active dequeue

For an active change, the service MUST request per-change cancellation and confirm task/process termination before applying dequeue state. It MUST preserve active state when the cancellation handle is absent, cancellation fails, or confirmation times out.

After confirmed termination and while the managed worktree is quiescent, the shared application coordinator MAY capture explanatory Git evidence before reacquiring its boundary so Git subprocess latency does not block unrelated operator admission. It MUST then reacquire the boundary, revalidate the target's current lifecycle state, read typed phase facts before `ReducerCommand::DequeueChange` clears them, and only then commit dequeue. Phase facts MUST be updated synchronously under the authoritative dispatch boundary so every typed fact dispatched before termination confirmation is visible at settlement.

A successful outcome MUST identify the typed phase active at settlement as the cancelled phase, the last completed lifecycle phase, and nullable final managed-worktree Apply commit evidence. An already-terminated target or target with no active typed phase MUST report `cancelled_phase: none`. The result MUST state that dequeue does not roll back previously completed worktree effects. Phase and Git evidence are explanatory non-authoritative observations; they MUST NOT become durable workflow-control state or cause unavailable evidence to be guessed.

#### Scenario: Active change terminates before dequeue

**Given**: A change is active and has a registered cancellation handle
**When**: The operator requests stop-and-dequeue
**Then**: Cancellation is issued
**And**: Termination is confirmed
**And**: Explanatory Git evidence may be read from the quiescent worktree without holding the application boundary
**And**: The boundary is reacquired, current lifecycle evidence is revalidated, and typed phase facts are read before dequeue clears them
**And**: Only then is `ReducerCommand::DequeueChange` applied
**And**: The successful outcome carries typed settlement evidence and reports no rollback of prior effects

#### Scenario: Apply completion races with cancellation settlement

**Given**: An Apply worker is active with deterministic synchronization around its final commit boundary
**And**: stop-and-dequeue has issued cancellation
**When**: The worker creates the final Apply commit, publishes Apply completion, enters Acceptance, and then confirms termination
**Then**: Settlement classifies Acceptance as the cancelled phase and Apply as the last completed phase
**And**: The exact final Apply commit OID is reported when repository evidence proves it
**And**: The Apply commit remains present after dequeue

#### Scenario: Already-terminated success reports no cancellation phase

**Given**: The registered task has already terminated and no typed phase remains active
**When**: stop-and-dequeue follows its existing already-terminated success path
**Then**: Settlement reads phase facts before dequeue
**And**: The result reports `cancelled_phase: none`
**And**: It does not infer a cancelled phase from historical display or logs

#### Scenario: Missing cancellation handle fails safely

**Given**: A change is active but no cancellation handle exists
**When**: The operator requests stop-and-dequeue
**Then**: The request fails
**And**: The change remains active
**And**: No successful settlement evidence or dequeue event is published

#### Scenario: Evidence failure does not invent a phase

**Given**: Termination is confirmed but current phase or managed-worktree Git evidence is unavailable or ambiguous
**When**: The coordinator settles stop-and-dequeue
**Then**: Unknown explanatory fields remain unknown
**And**: The coordinator does not derive them from task completion, display status, logs, or commit subject alone
**And**: Existing dequeue validity remains governed by shared lifecycle revalidation rather than observability evidence

<!-- Expected canonical result after archive: active dequeue remains cancellation-first and additionally fixes truthful phase and Apply-commit evidence at the settlement boundary without making observability authoritative. -->

### Requirement: Dynamic queue hooks reflect real mutations

The service MUST run `on_queue_add` and `on_queue_remove` exactly once after successful dynamic queue mutations. It MUST NOT run them for initial queue construction, failed requests, or no-op duplicate requests.

#### Scenario: Duplicate addition emits no hook

**Given**: A change is already in the dynamic queue
**When**: The operator requests another addition
**Then**: The request is a no-op
**And**: `on_queue_add` is not executed

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

### Requirement: Event-driven execution mark reconciliation

`ExecutionMarkStore` MUST remain the process-local authoritative execution-mark set across TUI, Web, and shared run-control target resolution. When a typed execution event creates a mark-revoking transition, the system MUST compare reducer evidence immediately before and after applying the event, update only the affected shared mark, and complete that reconciliation before frontend fan-out. Reducer pre-state capture, event application, post-state classification, and mark reconciliation MUST be ordered under the same process-local mutation boundary used by operator mark actions.

Mark-revoking transitions MUST include change-level transition into Error, terminal Rejected, rejected marker rows discovered by refresh, refresh classification that makes a marked target parallel-ineligible, successful per-change dequeue/legacy stop, and the first `on_merged` hook-failure transition into merge-wait recovery. Reconciliation MUST be target-scoped and idempotent.

For system-driven events, TUI row selection MUST mirror the reconciled store and MUST NOT remain an independent mark authority. Reducer `queued` status MUST remain queue presentation and MUST NOT synthesize an execution mark. Operator mark actions MUST use target-scoped shared-service mutation and then mirror the store; they MUST NOT replace the whole store from a cached TUI row set.

The system MUST preserve marks for unrelated changes, blocked/stalled/dependency-wait changes, ordinary MergeWait/ResolveWait, successful archive/merge/push/completion, global fatal Error without a target, and process-level Stopped. A steady Error or `on_merged` recovery row that was explicitly re-marked MUST NOT be cleared by an unrelated or duplicate later event.

#### Scenario: change-level Error clears stale intent before projection

- **GIVEN** changes `alpha` and `beta` are execution-marked
- **AND** `alpha` is not yet in reducer Error
- **WHEN** a processing, apply, acceptance, archive, push, or rejection-review failure transitions `alpha` into reducer Error
- **THEN** the shared execution mark for `alpha` is false before frontend fan-out
- **AND** `beta` remains marked
- **AND** TUI, Web, and Start target resolution observe the same result

#### Scenario: rejection and rejected refresh clear the target mark

- **GIVEN** a change is execution-marked
- **WHEN** `ChangeRejected` makes it terminal Rejected or `ChangesRefreshed` introduces it as a rejected marker row
- **THEN** only that change's shared mark is cleared
- **AND** the TUI rejected row and API snapshot both report it unmarked

#### Scenario: eligibility refresh clears invalid target intent

- **GIVEN** `alpha` and `beta` are execution-marked
- **AND** one refresh classifies `alpha` as parallel-ineligible while `beta` remains eligible
- **WHEN** that authoritative refresh dispatch is reconciled
- **THEN** only `alpha` loses its shared mark
- **AND** existing target-scoped queue cleanup remains coherent
- **AND** no whole-store row publication can remove `beta`

#### Scenario: explicit dequeue clears the shared target

- **GIVEN** a marked change completes stop-and-dequeue successfully
- **WHEN** `ChangeDequeued` or the legacy target-scoped stop event is projected
- **THEN** the shared mark and TUI row mark are false
- **AND** duplicate event delivery is a no-op

#### Scenario: queued presentation does not create a mark

- **GIVEN** a change has reducer queue intent but no execution mark
- **WHEN** a system event synchronizes its TUI row as queued
- **THEN** the row remains unmarked
- **AND** TUI and `/api/v2` continue to distinguish queue intent from execution marks

#### Scenario: duplicate failure after re-mark preserves fresh intent

- **GIVEN** a change-level Error or first `on_merged` recovery edge cleared the old mark
- **AND** an operator explicitly re-marked the steady recovery row through a supported route
- **WHEN** the same failure event is delivered again without creating a new reducer transition
- **THEN** the fresh shared mark remains true
- **AND** existing retry/start flow can consume it

#### Scenario: stale TUI rows cannot resurrect a revoked mark

- **GIVEN** TUI cached rows still show a target marked
- **AND** a system event revokes that target in `ExecutionMarkStore`
- **WHEN** a concurrent TUI operator action settles after the event
- **THEN** the action mutates only its requested target through the shared service
- **AND** it cannot replace the whole store or restore the revoked mark from stale row state

#### Scenario: wait and stop boundaries preserve marks

- **GIVEN** one or more changes are execution-marked
- **WHEN** they become dependency blocked, stalled, externally blocked, MergeWait, ResolveWait, archived, merged, pushed, or the process enters Stopped
- **THEN** their shared marks are preserved
- **AND** process-level Stopped can resume the same marked target set

### Requirement: Accepted command outcomes converge every same-process frontend

Every changed operator command MUST produce one typed authoritative outcome dispatch containing the process-level decision facts that are not already represented by an exact existing execution event. TUI, Web, `/api/v2`, and lifecycle projections MUST consume that same process-lifetime dispatch and MUST NOT independently rederive admission mode, resolver ownership, or scheduler acceptance from a frontend cache. TUI submission MUST NOT await application-gate ownership or termination confirmation inside its event-processing/render loop.

Existing exact event meanings MUST be reused: graceful stop uses `Stopping`, settled force stop uses `Stopped`, and successful target dequeue uses `ChangeDequeued`. New outcome vocabulary MUST NOT synthesize `ProcessingStarted` or another change lifecycle event merely to signal command acceptance.

TUI row marks changed by operator commands MUST be projected by target delta from the shared `ExecutionMarkStore`. A command MUST NOT replace the complete store from frontend `selected` rows. Event-driven Error, Rejected, refresh, and dequeue revocation remains governed by the shared mark-reconciliation requirement.

#### Scenario: Remote command reaches the next TUI render

**Given**: `cflx tui` and `/api/v2` share one process, reducer, mark store, resolve ledger, and dispatch owner
**When**: an API mark, queue, run, resolve, stop, cancel-stop, force-stop, or dequeue command changes state
**Then**: the next TUI event-processing pass observes the same target delta, row status, process mode, and resolver state
**And**: Web and `/api/v2` observe the same authoritative dispatch

#### Scenario: Local command preserves unrelated remote marks

**Given**: the API changed the shared execution mark for `beta`
**And**: the TUI row cache for `beta` has not yet rendered that delta
**When**: the TUI changes the mark or queue intent for `alpha`
**Then**: only `alpha` is written by the local command outcome
**And**: `beta` retains the API-provided shared mark

#### Scenario: Queue presentation does not become mark authority

**Given**: a Running row is checked because it carries queue intent while an Error row retains hidden explicit retry intent
**When**: a command outcome is projected to TUI rows
**Then**: queue presentation and execution marks remain separate axes
**And**: broad synchronization does not turn hidden retry intent into a checked Error row

#### Scenario: Late command projection cannot overwrite terminal state

**Given**: scheduler progress or cancellation reaches Error, Stopping, Stopped, or completion after a command is accepted
**When**: command and scheduler events are delivered through the authoritative dispatch boundary
**Then**: staged decision commit and outcome dispatch are atomic
**And**: scheduler activity enabled by the command's activation or wake is ordered afterwards
**And**: duplicate or late outcome delivery cannot restore an earlier mode

#### Scenario: Restart discards command coordination

**Given**: a process has application-transaction state, marks, process mode, or resolve reservations in memory
**When**: the process restarts with the same workspace and Git state
**Then**: those process-local values are discarded
**And**: the next workflow action is derived from workspace and Git evidence alone

### Requirement: Persistent-idle Ready remains a live run-control target

When a typed persistent-scheduler idle transition projects Ready/`select` while the scheduler task remains alive, TUI and Web MUST use the process-local `persistent_scheduler_idle` fact to distinguish this state from pre-run Select and retain live-scheduler controls; pre-run Select MUST continue to expose only Start. Web MUST expose Start, graceful stop, and force stop directly. TUI MUST expose Start plus its existing first-Esc graceful-stop hint, and after that request its existing second-Esc force-stop progression. The fact is presentation-only: shared run control MUST independently revalidate the existing scheduler liveness authority before executing each command.

Execution-mark mutations MUST remain Select-mode mark-only mutations. Accepted Start MUST resolve the authoritative marked target set and commit existing reducer queue or explicit-retry intent before publishing its outcome. When that accepted Start wakes the live scheduler with at least one committed target, the same authoritative outcome MUST project Running immediately in Core, TUI, and Web, clear `persistent_scheduler_idle`, and project the admitted targets as queued without spawning another scheduler task. Raw key input, refused Start, an empty target set, and a generic scheduler notification MUST NOT project Running. Accepted graceful stop and force stop MUST continue to address the live scheduler; graceful stop MUST wake the idle wait after recording the stop request so the scheduler can reach its existing stop boundary.

The Running projection acknowledges accepted operator intent; it MUST NOT by itself certify active lifecycle work or a typed execution phase. Existing execution-facts authorities MUST continue to derive dependency-analysis and admitted-work activity from their own typed events. The shared boundary MUST establish or preserve the idle-episode fact for every Ready state from which run control accepts graceful stop over a live parked scheduler, including Ready reached through `AllCompleted` settlement. A stop accepted from such a state is an idle-origin stop. When no executable, queued, admitted, active, resolve, merge, or cleanup work remains, that graceful stop MUST settle to inactive `Stopped`/Ready and MUST NOT retain `Stopping` while waiting for a nonexistent work boundary. If the accepted intent produces no admitted work and the persistent scheduler parks again, a newly rearmed idle edge MUST project Ready again. Existing typed workspace or base-lane work-start evidence MUST still clear an idle fact and project Running when no accepted Start already did so, including queue admission through non-Start paths, and MUST preserve Stopping when a graceful-stop request arrived first. Cancel-stop MUST remain valid only after graceful stop has projected Stopping; it MUST restore Ready when the idle-episode fact is true, whether set by the original idle transition or a later rearmed one, and Running when accepted Start or admitted work has cleared the fact.

<!-- replaces-scenario: Start wakes the existing idle scheduler -->
#### Scenario: Accepted Start wakes the existing idle scheduler and projects Running

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **AND** an eligible change is execution-marked
- **WHEN** Start is accepted through shared run control
- **THEN** existing reducer queue intent is added for the marked target
- **AND** the live scheduler is notified
- **AND** no second scheduler task is spawned
- **AND** the accepted outcome projects Running and clears `persistent_scheduler_idle`

#### Scenario: idle Ready marks remain mark-only

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **WHEN** an operator changes one or all execution marks
- **THEN** the process-local mark set changes under existing Select-mode rules
- **AND** no Running-mode queue mutation is synthesized until Start is accepted

#### Scenario: idle Ready exposes live-scheduler controls

- **GIVEN** `app_mode` is `select` with `persistent_scheduler_idle: true`
- **WHEN** TUI or Web renders lifecycle controls
- **THEN** Web exposes Start, graceful stop, and force stop
- **AND** TUI exposes Start and a first-Esc graceful-stop hint
- **AND** after graceful stop, TUI retains its second-Esc force-stop progression
- **AND** ordinary pre-run Select without the idle fact continues to expose only Start

#### Scenario: graceful stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive in its event-driven wait
- **WHEN** graceful stop is accepted
- **THEN** the existing graceful-stop request is recorded
- **AND** the idle scheduler is notified to reach its stop boundary
- **AND** the frontend projects Stopping while retaining `persistent_scheduler_idle: true`

<!-- replaces-scenario: cancel stop returns to idle Ready -->
#### Scenario: Cancel idle-origin stop does not invent Running

- **GIVEN** Ready was reached with the scheduler parked and no remaining work through a typed idle edge or `AllCompleted` settlement over the live scheduler
- **AND** graceful stop was accepted from that idle-origin state
- **AND** no accepted Start or typed work-start event opened a later run episode
- **WHEN** cancel-stop is accepted
- **THEN** the graceful-stop request is withdrawn
- **AND** the frontend returns to Ready / `app_mode: select`
- **AND** it does not claim Running without an accepted Start or typed work-start event

<!-- replaces-scenario: accepted Start makes later cancel stop return to Running -->
#### Scenario: Cancel stop after real work restores Running

- **GIVEN** accepted Start from persistent-idle Ready projected Running and cleared `persistent_scheduler_idle`
- **AND** graceful stop then projected Stopping
- **WHEN** cancel-stop is accepted before a later idle transition
- **THEN** the graceful-stop request is withdrawn
- **AND** the frontend returns to Running

#### Scenario: work start wins before cancel stop

- **GIVEN** graceful stop originated from persistent-idle Ready
- **AND** a typed work-start event arrives while the frontend is Stopping
- **WHEN** that event is projected
- **THEN** Stopping is preserved
- **AND** `persistent_scheduler_idle` becomes false
- **AND** a later accepted cancel-stop returns the frontend to Running rather than Ready

#### Scenario: force stop addresses idle Ready scheduler

- **GIVEN** Ready presentation was produced by a typed persistent-scheduler idle transition
- **AND** the scheduler task remains alive
- **WHEN** force stop is accepted
- **THEN** the same scheduler task is cancelled
- **AND** existing stop classification and shutdown-barrier behavior remain authoritative
- **AND** the terminal Stopped or Error projection clears `persistent_scheduler_idle`

#### Scenario: Refused idle Start remains Ready

- **GIVEN** the frontend reports persistent-idle Ready
- **WHEN** Start has no marked eligible target or scheduler liveness no longer validates
- **THEN** Start is refused or settles without a dispatch
- **AND** Ready and `persistent_scheduler_idle: true` remain unchanged
- **AND** no scheduler is started or notified by the refused command

#### Scenario: No-work wake returns to Ready

- **GIVEN** accepted Start from persistent-idle Ready projected Running and woke the existing scheduler
- **AND** the scheduler reconciled the committed queue or retry intent
- **WHEN** analysis admits no workspace or base-lane work and the scheduler parks again
- **THEN** one newly rearmed persistent-idle edge projects Ready
- **AND** `persistent_scheduler_idle` becomes true again
- **AND** unchanged or generic wakeups emit no duplicate idle edge

#### Scenario: Start feedback does not certify active work

- **GIVEN** accepted Start from persistent-idle Ready has projected `app_mode: running`
- **AND** no dependency-analysis or lifecycle start event has occurred yet
- **WHEN** execution status is observed
- **THEN** scheduler liveness MAY be true
- **AND** `has_active_work` remains false
- **AND** no current lifecycle phase is invented from Start acceptance, queue intent, marks, or application mode

<!-- Expected canonical result after archive: accepted persistent-idle Start will project Running immediately while preserving live-scheduler controls, non-Start admission, refusal, stop/cancel races, and no-work return-to-Ready. -->
#### Scenario: No-work graceful stop reaches inactive Ready

- **GIVEN** Ready was reached with a live parked scheduler and no executable, queued, admitted, active, resolve, merge, or cleanup work
- **WHEN** graceful stop is accepted and the scheduler settles the request
- **THEN** Core, TUI, and Web leave `Stopping`
- **AND** the process reaches inactive `Stopped` whose TUI header is Ready
- **AND** no synthetic queue intent, work-start event, or mark mutation is introduced

### Requirement: Target-scoped force-stop transaction

The shared operator application transaction MUST provide `ForceStopChange` for exactly one named change. It MUST validate action eligibility from the admitted authoritative revision before side effects, bypass the graceful SIGTERM escalation window, immediately send SIGKILL to only the managed process group owned by that change, wait for confirmed termination and process reaping, atomically clear that change's queue admission intent and execution mark, and settle it as stopped without rolling back completed worktree effects. The transaction MUST preserve every unrelated change's processes, marks, queue intent, execution identity, subscription binding, and progress, and MUST NOT change process-wide run mode, scheduler state, or stop state.

A queued or dependency-blocked admitted target without a live process MUST be eligible for dequeue-only settlement with its execution mark revoked. Applying, accepting, rejecting, archiving, and resolving targets are eligible only while they own live managed activity. Merge-wait, resolve-wait without a live resolver, terminal, rejected, unknown, and unadmitted targets MUST be ineligible with typed reasons. The transaction MUST use the managed ownership graph rather than unscoped PID lookup. Stale revision MUST fail before signalling. Exact idempotent replay MUST return the original result without repeating cancellation or affecting a later execution episode.

#### Scenario: One concurrent change is killed

- **GIVEN** changes `alpha` and `beta` have active managed phase processes
- **AND** `alpha` publishes `force_stop_change` as allowed
- **WHEN** the operator force-stops `alpha`
- **THEN** only `alpha`'s managed processes are cancelled, terminated, and reaped
- **AND** `alpha` settles stopped and is no longer queued
- **AND** `beta` keeps its process, marks, queue intent, execution identity, and progress
- **AND** the scheduler and process-wide run mode remain unchanged

#### Scenario: Completed effects are preserved

- **GIVEN** `alpha` completed Apply and is active in a later managed phase
- **WHEN** the operator force-stops `alpha`
- **THEN** the later phase is terminated and reaped before settlement
- **AND** the completed Apply worktree commit remains present
- **AND** the typed result reports `effects_rolled_back: false`

#### Scenario: Queued target is dequeued without signalling another process

- **GIVEN** `alpha` is admitted and queued or dependency-blocked without a live managed process
- **WHEN** the operator force-stops `alpha`
- **THEN** `alpha` receives dequeue-only stopped settlement
- **AND** its queue intent and execution mark are cleared atomically
- **AND** no process belonging to another change is signalled

#### Scenario: Later mark settlement does not re-admit the target

- **GIVEN** `alpha` was marked and active before targeted force-stop
- **WHEN** its force-stop settles and the owner's later mark settlement runs
- **THEN** `alpha` remains unmarked and not queued
- **AND** no new execution episode is created without a new operator mark

#### Scenario: Ineligible target changes nothing

- **GIVEN** `alpha` is unknown, terminal, rejected, unadmitted, in merge-wait, or in resolve-wait without a live resolver
- **WHEN** `ForceStopChange` addresses `alpha`
- **THEN** the command returns a typed no-op or failure
- **AND** no managed process, mark, queue intent, execution identity, scheduler state, or process-wide mode changes

#### Scenario: Stale request has no termination side effect

- **GIVEN** the caller's expected revision is stale
- **WHEN** it requests `ForceStopChange` for `alpha`
- **THEN** revision validation fails before cancellation
- **AND** neither `alpha` nor any unrelated change is signalled

#### Scenario: Exact replay does not kill a later episode

- **GIVEN** a `ForceStopChange` command for `alpha` settled successfully
- **AND** `alpha` later starts a new execution episode
- **WHEN** the exact original command is replayed with its idempotency key
- **THEN** the original settled result is returned unchanged
- **AND** the new execution episode is not cancelled

#### Scenario: Stop notification and wait remain truthful

- **GIVEN** `alpha` has a proposal subscription and an observing client wait
- **WHEN** targeted force-stop settles its current execution episode
- **THEN** the subscription emits the ordinary terminal `stopped` event for that exact execution ID
- **AND** client wait releases with `change_requires_action` and exit status 27
