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

<!-- Expected canonical result after archive: settled Apply-limit diagnostics remain observable but no longer prevent a later explicit retry from creating a fresh execution boundary. -->
