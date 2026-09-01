## MODIFIED Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application coordinator. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use one process-lifetime authoritative `EventSink` dispatch boundary shared by runner-local and orchestration-run producers, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each ordinary new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit plus authoritative outcome dispatch, exact revision capture, and scheduler preparation. Activity enabled by the command's later activation or wake MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation MUST leave no reducer, execution-mark, queue, terminal-state, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect. A command awaiting confirmed runtime termination MUST use the same coordinator's two-phase protocol and MUST NOT hold the application gate, authoritative dispatch transaction, or TUI event loop during that wait.

Start admission MUST apply the complete-request worktree eligibility fence to the full marked set, then classify current marked target evidence instead of using process mode as a proxy for retry eligibility. In Select and Stopped, ordinary startable targets MUST take priority and retry-only targets MUST be excluded rather than mixed into the same launch; when no ordinary target is startable, marked retry routes MAY be selected. In Stopped, an explicit Start MUST additionally treat a preserved marked target whose terminal classification is operator `Stopped`, with stop-produced `NotQueued` intent and no non-stop terminal or waiting evidence, as ordinary resumable work: the same fail-atomic transaction MUST clear only that `Stopped` terminal classification and stop-produced dequeue/runtime residue that prevents ordinary admission, preserve the mark, establish ordinary queue intent, and start one fresh scheduler boundary. Mark mutation and delayed settlement alone MUST NOT resume stopped work. In Running, Start MUST accept only marked retry routes and MUST NOT replace live ordinary mark settlement. Error MUST retain marked retry routing, and Stopping MUST refuse Start. Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent. All transaction, mode, mark, and reservation coordination MUST remain process-local and MUST NOT become durable workflow authority.

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


#### Scenario: Explicit Start resumes preserved stopped marks

**Given**: Core mode is Stopped
**And**: `alpha` is execution-marked, has `queue_intent=not_queued`, and its terminal classification is operator Stopped with no non-stop terminal or waiting evidence
**And**: `alpha` is otherwise ordinary, unblocked, and worktree-eligible
**When**: TUI F5 or remote Start enters the shared application transaction
**Then**: both adapters select `alpha` as ordinary resumable work
**And**: the transaction clears only the Stopped classification and stop-produced dequeue/runtime residue that prevents ordinary admission, preserves the execution mark, and establishes ordinary queue intent
**And**: the accepted outcome is dispatched before exactly one fresh scheduler boundary starts
**And**: that boundary produces a new dependency-analysis attempt without owner restart

#### Scenario: Mark alone cannot resume stopped work

**Given**: `alpha` is execution-marked and stopped
**When**: an operator marks, bulk-marks, or re-marks `alpha`, or a mark-settlement deadline expires
**Then**: `alpha` remains stopped and not queued
**And**: no terminal evidence, queue intent, scheduler boundary, or analysis edge changes

#### Scenario: Stopped resume preserves non-ordinary terminal evidence

**Given**: Core mode is Stopped
**And**: marks include ordinary stopped target `alpha` plus targets carrying Error, rejected, archived, merged, pushed, blocked/stalled, acceptance-hold, unsupported, or worktree-ineligible evidence
**When**: Start evaluates the complete marked request
**Then**: any worktree-ineligible marked target rejects the complete request before class selection with no mutation
**And**: otherwise only ordinary stopped/not-queued targets are resumed
**And**: every other non-ordinary target retains its evidence and receives a target-specific exclusion
**And**: none is converted to ordinary queue work or implicit retry

#### Scenario: Stopped resume preparation failure is fail-atomic

**Given**: a preserved marked stopped target is otherwise resumable
**And**: scheduler preparation fails
**When**: TUI or remote Start enters the shared transaction
**Then**: target terminal state, marks, queue intent, process mode, hooks, revision-visible effects, and scheduler counts equal their pre-command state
**And**: no analysis edge is emitted

<!-- Expected canonical result after archive: the shared Start transaction will choose ordinary or retry intent from current target evidence with explicit mode and worktree guards, preserving TUI/API parity and fail-atomic ordering. -->
