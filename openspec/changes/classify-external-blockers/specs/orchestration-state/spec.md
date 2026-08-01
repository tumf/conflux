## ADDED Requirements

### Requirement: Orchestrator classifies external blockers separately from stalls

Conflux SHALL make the final lifecycle classification from validated structured evidence. A change SHALL be `blocked` when a concrete non-repository prerequisite prevents useful execution and the evidence includes a verifiable unblock condition. A change SHALL be `stalled` when automatic execution stops because of no semantic progress, repeated findings, or exhausted retry or repair policy.

Agents SHALL report blocker facts but SHALL NOT directly assign canonical lifecycle state. Compatibility verdict token text alone SHALL NOT determine the classification.

#### Scenario: Structured external prerequisite becomes blocked

- **GIVEN** Apply or Acceptance reports a supported external prerequisite with non-empty evidence, prerequisite or owner, verifiable unblock condition, next action, and resumability
- **AND** repository-only work or a test double cannot satisfy the prerequisite
- **WHEN** the orchestrator validates the report
- **THEN** the proposal lifecycle status is `blocked`
- **AND** blocker metadata identifies the blocker kind as external and preserves its origin and evidence

#### Scenario: Exhausted execution remains stalled

- **GIVEN** a change repeats the same finding, makes no semantic progress, or exhausts its retry or repair budget
- **WHEN** the orchestrator finalizes the automatic execution outcome
- **THEN** the proposal lifecycle status is `stalled`
- **AND** it is not classified as external `blocked` merely because progress stopped

#### Scenario: Bare compatibility token does not choose lifecycle

- **GIVEN** an agent emits `gated` or legacy `blocked` without complete structured blocker evidence
- **WHEN** the orchestrator parses the compatibility input
- **THEN** bounded protocol correction handles the incomplete output
- **AND** the token text alone creates neither external `blocked` nor `stalled`

### Requirement: Blocked lifecycle preserves blocker kind

A blocked proposal SHALL expose whether it is waiting on a proposal dependency or an external prerequisite. External blocker metadata SHALL include origin, category, evidence, prerequisite or owner, unblock condition, next action, and resumability. Dependency blocking SHALL continue to derive from the proposal graph and SHALL NOT be represented as an external blocker.

#### Scenario: Dependency and external waits share status without losing meaning

- **GIVEN** one proposal waits on an unarchived dependency and another waits on a validated external prerequisite
- **WHEN** operator-facing state is rendered
- **THEN** both lifecycle statuses are `blocked`
- **AND** their blocker kinds are respectively dependency and external
- **AND** the external wait does not create a synthetic proposal dependency edge

### Requirement: External blocked state remains workspace-local and process-lifetime

External blocked runtime state SHALL remain in memory for the current process and SHALL NOT introduce out-of-worktree durable workflow control. Restart routing SHALL be recomputed from workspace file state, workspace git state, and base-branch comparison. Runtime blocker metadata SHALL NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or integration.

#### Scenario: Restart does not trust blocked runtime state

- **GIVEN** a process held a validated external blocked state
- **WHEN** Conflux restarts with the same workspace and git evidence
- **THEN** previous in-memory status has no routing authority
- **AND** Conflux re-evaluates the current repository evidence
- **AND** a complete unarchived Apply revision returns to Acceptance rather than inferring PASS or archive readiness

#### Scenario: Explicit retry reruns the blocked phase

- **GIVEN** a proposal is externally blocked
- **WHEN** an operator requests retry
- **THEN** Conflux validates current workspace identity
- **AND** a matching workspace is dispatched to rerun the blocked phase without requiring an external-state oracle
- **AND** the new execution result supplies the evidence for clearing or restoring the blocked classification
- **AND** an identity mismatch prevents unsafe dispatch without silently discarding prior blocker evidence

#### Scenario: Dependent proposal waits on an externally blocked dependency

- **GIVEN** proposal `beta` depends on proposal `alpha`
- **AND** `alpha` is externally blocked
- **WHEN** scheduler eligibility is derived
- **THEN** `beta` remains `blocked` with dependency blocker kind
- **AND** `beta` does not inherit `alpha`'s external blocker kind
- **AND** unrelated ready proposals remain eligible

## MODIFIED Requirements

### Requirement: Rejected terminal state remains distinct from errors

The terminal result MUST include `Rejected` as a permanent terminal state distinct from `Error`. A rejected change is one where rejecting review has confirmed the specification is unimplementable or otherwise out of scope for completion, requiring a rollback to the base branch with a documented reason.

Acceptance-gate and rejecting-review holds that are not confirmed as rejected MUST remain non-terminal. A validated non-repository prerequisite MUST display as `blocked`; no-progress, repeated-finding, exhausted-policy, and other intervention holds that do not validate as external prerequisites MUST display as `stalled`.

#### Scenario: rejecting-confirmed change becomes rejected terminal state

- **GIVEN** a change is in `Rejecting`
- **AND** the rejection flow completes (`REJECTED.md` committed and worktree removed)
- **WHEN** the reducer applies the terminal rejection event
- **THEN** the terminal result becomes `Rejected`
- **AND** the derived display status is `rejected`

#### Scenario: validated external acceptance hold remains blocked

- **GIVEN** acceptance reports a validated non-repository prerequisite
- **AND** rejecting review has not confirmed terminal rejection
- **WHEN** the reducer exposes the paused lifecycle state
- **THEN** the terminal result remains `None`
- **AND** the derived display status is `blocked`

#### Scenario: non-external acceptance hold remains stalled

- **GIVEN** acceptance stops after no semantic progress, repeated findings, or exhausted repair policy
- **WHEN** the reducer exposes the paused lifecycle state
- **THEN** the terminal result remains `None`
- **AND** the derived display status is `stalled`

### Requirement: WebSocket change status consistency with TUI

Server-mode WebSocket API SHALL produce the same set of display status strings as `ChangeRuntimeState.display_status()`. The system MUST NOT maintain a separate mapping from workspace states to display strings that diverges from the reducer-derived status vocabulary. Blocked payloads SHALL expose a machine-readable dependency or external blocker kind and SHALL preserve validated external blocker detail.

#### Scenario: All display statuses are representable in WebSocket payloads

- **GIVEN** the reducer can produce any of: `not queued`, `queued`, `blocked`, `stalled`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, `merge wait`, `resolve pending`, `archived`, `pushed`, `merged`, `rejected`, `error`, `stopped`
- **WHEN** a WebSocket client receives a change list
- **THEN** the status field for each change is one of the above values
- **AND** a blocked change carries its blocker kind

### Requirement: Stalled blocker metadata

When a change enters non-terminal `stalled`, reducer-owned in-memory state and authoritative blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the stall from dependency blocking, validated external blocking, protocol error, and terminal rejection. Validated non-repository prerequisites MUST use external `blocked` metadata instead of stalled metadata.

Acceptance-generated stalled evidence MUST live in the in-memory `OrchestratorState` only for the lifetime of the current process. It MUST NOT be persisted to `~/.local/state/cflx/acceptance-stalls/` or any other out-of-worktree durable location. Process restart MUST clear all in-memory stall state. When repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

Runtime stalled metadata MAY control dispatch suppression, stalled display, and Acceptance retry preparation only. It MUST NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration, and its mutation or deletion MUST NOT dirty the managed worktree.

#### Scenario: non-external stalled evidence is process-lifetime only

- **GIVEN** validated no-progress, repeated-finding, or exhausted-policy evidence exists in current in-memory state
- **WHEN** the current Conflux process displays the status
- **THEN** reducer state presents the recorded evidence, resumability, and next action
- **AND** display status remains execution `stalled`
- **AND** the managed worktree remains clean

#### Scenario: validated external evidence does not become stalled metadata

- **GIVEN** Apply or Acceptance supplies a validated non-repository prerequisite
- **WHEN** runtime classifies the evidence
- **THEN** it creates external `blocked` metadata
- **AND** it emits no stalled lifecycle transition for that prerequisite

#### Scenario: restart clears stall and re-runs Acceptance

- **GIVEN** a previously stalled in-memory record is gone after restart
- **AND** repository evidence still shows a complete unarchived Apply revision
- **WHEN** Conflux restarts
- **THEN** it does not reconstruct stalled state, PASS, or archive readiness
- **AND** it routes the change to Acceptance rather than Apply or archive

#### Scenario: bare GATED produces no stalled metadata

- **GIVEN** Acceptance emits bare GATED or legacy blocked compatibility input without valid structured blocker evidence
- **WHEN** runtime handles the result
- **THEN** it records no stalled or external blocked metadata
- **AND** bounded Acceptance protocol retry handles the result
