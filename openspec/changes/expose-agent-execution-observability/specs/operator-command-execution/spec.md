## MODIFIED Requirements

### Requirement: Cancellation precedes active dequeue

For an active change, the service MUST request per-change cancellation and confirm task/process termination before applying dequeue state. It MUST preserve active state when the cancellation handle is absent, cancellation fails, or confirmation times out.

After confirmed termination, the shared application coordinator MUST reacquire its boundary, revalidate the target's current lifecycle state, and capture typed settlement evidence before committing dequeue. A successful outcome MUST identify the phase actually cancelled, the last completed lifecycle phase, and nullable final managed-worktree Apply commit evidence. It MUST state that dequeue does not roll back previously completed worktree effects. Phase and Git evidence are explanatory non-authoritative observations; they MUST NOT become durable workflow-control state or cause unavailable evidence to be guessed.

#### Scenario: Active change terminates before dequeue

**Given**: A change is active and has a registered cancellation handle
**When**: The operator requests stop-and-dequeue
**Then**: Cancellation is issued
**And**: Termination is confirmed
**And**: The application boundary is reacquired and current lifecycle evidence is revalidated
**And**: Only then is `ReducerCommand::DequeueChange` applied
**And**: The successful outcome carries typed settlement evidence and reports no rollback of prior effects

#### Scenario: Apply completion races with cancellation settlement

**Given**: An Apply worker is active with deterministic synchronization around its final commit boundary
**And**: stop-and-dequeue has issued cancellation
**When**: The worker creates the final Apply commit, publishes Apply completion, enters Acceptance, and then confirms termination
**Then**: Settlement classifies Acceptance as the cancelled phase and Apply as the last completed phase
**And**: The exact final Apply commit OID is reported when repository evidence proves it
**And**: The Apply commit remains present after dequeue

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
