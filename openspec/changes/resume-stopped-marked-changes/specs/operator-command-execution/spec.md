## MODIFIED Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application coordinator. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use one process-lifetime authoritative `EventSink` dispatch boundary shared by runner-local and orchestration-run producers, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each ordinary new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit plus authoritative outcome dispatch, exact revision capture, and scheduler preparation. Activity enabled by the command's later activation or wake MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation MUST leave no reducer, execution-mark, queue, terminal-state, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect. A command awaiting confirmed runtime termination MUST use the same coordinator's two-phase protocol and MUST NOT hold the application gate, authoritative dispatch transaction, or TUI event loop during that wait.

Start admission MUST apply the complete-request worktree eligibility fence to the full marked set, then classify current marked target evidence instead of using process mode as a proxy for retry eligibility. In Select, ordinary `not queued` targets MUST take priority and retry-only targets MUST be excluded rather than mixed into the same launch; when no ordinary target is startable, marked retry routes MAY be selected. In Stopped, an explicit Start MUST additionally treat a preserved marked target whose only terminal evidence is operator Stopped as ordinary resumable work: the same fail-atomic transaction MUST clear only stop-owned terminal residue, preserve the mark, establish ordinary queue intent, and start one fresh scheduler boundary. Mark mutation and delayed settlement alone MUST NOT resume stopped work. In Running, Start MUST accept only marked retry routes and MUST NOT replace live ordinary mark settlement. Error MUST retain marked retry routing, and Stopping MUST refuse Start. Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent. All transaction, mode, mark, and reservation coordination MUST remain process-local and MUST NOT become durable workflow authority.

#### Scenario: Explicit Start resumes preserved stopped marks

**Given**: Core mode is Stopped
**And**: `alpha` is execution-marked, has `queue_intent=not_queued`, and its only terminal evidence is operator Stopped
**And**: `alpha` is otherwise ordinary, unblocked, and worktree-eligible
**When**: TUI F5 or remote Start enters the shared application transaction
**Then**: both adapters select `alpha` as ordinary resumable work
**And**: the transaction clears only the stop-owned terminal residue, preserves the execution mark, and establishes ordinary queue intent
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
**Then**: existing complete-request worktree fences remain authoritative
**And**: when the request is otherwise admissible, only ordinary stopped/not-queued targets are resumed
**And**: every non-ordinary target retains its evidence and receives a target-specific exclusion
**And**: none is converted to ordinary queue work or implicit retry

#### Scenario: Stopped resume preparation failure is fail-atomic

**Given**: a preserved marked stopped target is otherwise resumable
**And**: scheduler preparation fails
**When**: TUI or remote Start enters the shared transaction
**Then**: target terminal state, marks, queue intent, process mode, hooks, revision-visible effects, and scheduler counts equal their pre-command state
**And**: no analysis edge is emitted

<!-- Expected canonical result after archive: explicit Start can resume preserved ordinary stopped marks through the shared fail-atomic transaction without making mark settlement a lifecycle control. -->
