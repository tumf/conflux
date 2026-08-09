## MODIFIED Requirements

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
