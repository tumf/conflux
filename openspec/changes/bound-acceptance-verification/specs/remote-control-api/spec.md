## MODIFIED Requirements

### Requirement: Shared lifecycle scheduling semantics

Start, retry, stop, cancel stop, force stop, and resolve MUST use shared application-service semantics across TUI and v2. Retry MUST preserve reconciled evidence, resolve MUST enforce one active resolver with FIFO waiting, and force stop MUST report the actual runtime-activity classification. `retry_change`, `retry_errors`, and a terminal-error `set_queue_intent=true` alias MUST share the same explicit-retry classification. A settled terminal error that retains typed Apply iteration-limit evidence MUST classify through the ordinary terminal-error retry route; no scheduler-liveness limit guard may refuse it. A command whose candidates are all currently ineligible MUST settle truthfully without notifying or starting a scheduler. For a non-resumable Acceptance deadline hold, shared admission and mark settlement MUST recompute the workspace-derived Acceptance fingerprint. An unchanged fingerprint MUST expose and refuse retry with `unchanged_acceptance_input`; a changed fingerprint MAY publish one retry edge while preserving valid gate evidence.

#### Scenario: Retry dispatches reconciled work

**Given**: A marked error, stalled acceptance hold, or resumable external blocker is valid for retry
**When**: Retry is accepted
**Then**: The shared service applies the correct retry route
**And**: The scheduler is notified or started
**And**: Unsupported holds retain their blocker evidence

#### Scenario: Resolve queues behind an active resolver

**Given**: One merge resolution is active
**When**: Another valid merge-wait change is submitted for resolve
**Then**: It is reserved once in FIFO order
**And**: Duplicate submission does not create another queue entry

#### Scenario: V2 retry accepts a settled Apply-limit error

**Given**: The authoritative snapshot exposes `retry_change` for settled terminal-error `alpha`, which retains typed Apply iteration-limit evidence
**And**: The persistent scheduler task remains live
**When**: A client submits `retry_change` or terminal-error `set_queue_intent=true` for `alpha` at the current revision
**Then**: The shared service accepts the ordinary terminal-error retry route exactly once
**And**: One target-specific explicit-retry edge is published
**And**: The later invocation receives fresh Apply budget

#### Scenario: V2 bulk retry includes a settled Apply-limit target

**Given**: `alpha` retains settled Apply iteration-limit evidence and `beta` is ordinarily retryable
**When**: A client submits `retry_errors` for both at the current revision
**Then**: Both targets are retried and dispatched exactly once
**And**: A genuinely unsupported target in the same request remains excluded with its evidence intact

#### Scenario: Retry after run closure starts a later boundary

**Given**: The scheduler boundary that limited `alpha` has completed finish-hook ownership and closed
**When**: A current-revision retry for `alpha` is accepted
**Then**: It cannot notify the closed scheduler
**And**: It may start a new scheduler boundary with workspace-derived state and a fresh budget

<!-- Expected canonical result after archive: the authoritative snapshot treats retained Apply-limit evidence as diagnostic, exposes retry eligibility consistent with explicit command admission, and shared lifecycle semantics classify settled Apply-limit errors through the ordinary explicit terminal-error retry route. -->


#### Scenario: Acceptance deadline hold has terminal operator projection

**Given**: `alpha` exhausted its bounded semantic review deadline and cleanup succeeded
**When**: state, subscriptions, or `cflx client wait` project the result
**Then**: `alpha` has display status `stalled`, blocker kind `none`, `resumable: false`, category `acceptance_review_deadline_exhausted`, and retry disallowed while its fingerprint is unchanged
**And**: `cflx client wait` releases with `change_requires_action` rather than continuing to wait

#### Scenario: Unchanged retry is refused consistently

**Given**: current Acceptance fingerprint equals the persisted non-resumable deadline-hold fingerprint
**When**: `retry_change`, `retry_errors`, Start/F5, terminal-error queue intent, or automatic mark settlement evaluates `alpha`
**Then**: shared admission refuses `alpha` with `unchanged_acceptance_input`
**And**: detail lists the fingerprint components whose repository-visible change can restore eligibility
**And**: no scheduler notification or work dispatch occurs

#### Scenario: Changed fingerprint restores retry eligibility

**Given**: a prior Acceptance deadline hold exists
**And**: repository evidence now produces a different fingerprint
**When**: state and command admission evaluate the current revision
**Then**: retry is allowed and exactly one target-specific edge may be published
**And**: valid declared gate evidence remains reusable

#### Scenario: Bulk retry reports mixed eligibility

**Given**: `alpha` has unchanged Acceptance input and `beta` has changed eligible input
**When**: `retry_errors` evaluates both
**Then**: its settled result reports `alpha` with `unchanged_acceptance_input` and admits `beta` exactly once
**And**: neither target's evidence is discarded
