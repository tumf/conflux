## MODIFIED Requirements

### Requirement: Shared operator command service

The system MUST route TUI and remote orchestration actions through one process-local operator application coordinator. Authoritative workflow transitions MUST use `ReducerCommand`, notifications MUST use one process-lifetime authoritative `EventSink` dispatch boundary shared by runner-local and orchestration-run producers, and Start target resolution MUST use the shared run-control boundary. Equivalent accepted TUI and remote intent MUST produce identical target resolution, process-mode transition, reducer and mark/queue effects, resolve reservation, scheduler activation or wake, cancellation classification, typed outcome, and error.

For each ordinary new command, the transaction MUST serialize final mode/status/eligibility validation, fail-atomic intent commit plus authoritative outcome dispatch, exact revision capture, and scheduler preparation. Activity enabled by the command's later activation or wake MUST NOT emit an event before the accepted command effect is dispatched. A failed preparation MUST leave no reducer, execution-mark, queue, explicit-retry, resolve-reservation, graceful-stop, mode, hook, scheduler, or frontend effect. A command awaiting confirmed runtime termination MUST use the same coordinator's two-phase protocol and MUST NOT hold the application gate, authoritative dispatch transaction, or TUI event loop during that wait.

Start admission MUST classify current marked target evidence instead of using process mode as a proxy for retry eligibility. In Select and Stopped, ordinary startable targets MUST take priority and retry-only targets MUST be excluded rather than mixed into the same launch; when no ordinary target is startable, marked retry routes MAY be selected. In Running, Start MUST accept only marked retry routes and MUST NOT replace live ordinary mark settlement. Error MUST retain marked retry routing, and Stopping MUST refuse Start. Catalog refresh, observation refresh, and worktree discovery MUST NOT bypass the shared intent boundaries or synthesize queue intent. All transaction, mode, mark, and reservation coordination MUST remain process-local and MUST NOT become durable workflow authority.

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
**And**: retry-only targets are excluded with target-specific detail
**And**: no retry reducer command or explicit-retry edge is emitted for the excluded targets

#### Scenario: Running Start accepts retry only

**Given**: A scheduler is live in Running mode
**And**: marked targets contain retry-eligible change-local error `alpha` and ordinary `not queued` change `beta`
**When**: Start enters the shared application transaction
**Then**: only `alpha` MAY be accepted through explicit retry
**And**: `beta` remains governed by the existing live mark-settlement path
**And**: the scheduler is notified exactly once after the accepted outcome is dispatched

#### Scenario: Failed evidence-aware Start has no partial effect

**Given**: No marked target satisfies the Start route permitted by the current mode and evidence
**When**: TUI or remote Start enters the shared application transaction
**Then**: the outcome is failed rather than succeeded
**And**: process mode, reducer state, marks, queue, explicit-retry edges, scheduler, and frontend projection remain unchanged

<!-- Expected canonical result after archive: the shared Start transaction will choose ordinary or retry intent from current target evidence with explicit mode guards, preserving TUI/API parity and fail-atomic ordering. -->

### Requirement: Retry routing preserves reconciled evidence

Terminal error retry MUST use `ReducerCommand::RetryError`. Acceptance-stalled retry MUST reconcile the existing runtime hold and resume through the existing explicit acceptance retry path without rerunning apply. Before either route mutates state, the shared service MUST reject a target carrying typed Apply iteration-limit evidence owned by the active run. Unsupported, non-resumable, identity-mismatched, or active-run-limited targets MUST retain their evidence. Bulk retry and Start-selected retry MUST exclude such targets, dispatch other accepted targets once, and produce no scheduler effect when none remain.

An accepted terminal-error retry selected by Start MUST publish the same target-ID-bearing explicit-retry edge as an individual or bulk retry. Ordinary `AddToQueue`, generic scheduler notification, execution marks, and delayed mark settlement MUST NOT substitute for that edge or clear terminal error evidence.

#### Scenario: Start-selected terminal error publishes one explicit-retry edge

**Given**: marked change `alpha` carries retry-eligible terminal Error evidence
**And**: Start admission selects retry routing for `alpha`
**When**: the prepared command commits
**Then**: the reducer applies `RetryError(alpha)` exactly once
**And**: one target-specific explicit-retry edge is published
**And**: the execution mark for `alpha` is restored
**And**: no ordinary queue-add hook or delayed mark-settlement admission substitutes for retry

#### Scenario: Start-selected unsupported retry preserves evidence

**Given**: a marked target is non-resumable, identity-mismatched, unsupported, or active-run-limited
**When**: Start evaluates retry routing
**Then**: the target is refused or excluded with current evidence intact
**And**: no reducer, mark, queue, hook, retry-edge, notification, or scheduler-start effect occurs for that target

#### Scenario: Runtime-limit retry requires new operator intent

**Given**: an invocation produced typed runtime-limit termination and settled into terminal Error
**When**: no explicit operator retry has been accepted
**Then**: the scheduler MUST NOT redispatch the failed target from queue reconciliation or ordinary notification
**When**: a later Start request accepts the marked retry route
**Then**: the normal terminal-error retry transition and explicit-retry edge MAY release the target for analysis

<!-- Expected canonical result after archive: Start-selected retries consume the same reconciled evidence and target-specific edge as explicit retry commands, including mutation-free active-limit and unsupported-evidence handling. -->
