## MODIFIED Requirements

### Requirement: Stalled blocker metadata

When a change enters non-terminal `stalled`, reducer-owned in-memory state and authoritative blocker evidence MUST preserve operator-facing metadata sufficient to distinguish the blocker from dependency blocking, protocol error, and terminal rejection.

Acceptance-generated stalled evidence MUST live in the in-memory `OrchestratorState` only for the lifetime of the current process. It MUST NOT be persisted to `~/.local/state/cflx/acceptance-stalls/` or any other out-of-worktree durable location. The in-memory state binds change ID, blocker category, evidence, next action, resumability, and timestamps. Process restart MUST clear all in-memory stall state. When repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

Runtime stalled metadata MAY control dispatch suppression, stalled display, and Acceptance retry preparation only. It MUST NOT establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration, and its mutation or deletion MUST NOT dirty the managed worktree.

#### Scenario: in-memory stalled evidence is process-lifetime only

- **GIVEN** a validated Acceptance stall exists in the current in-memory state
- **AND** its change ID, Apply revision, and blocker metadata are preserved
- **WHEN** the current Conflux process displays the stalled status
- **THEN** reducer state presents the recorded category, evidence, resumability, and next action
- **AND** display status remains execution `stalled`
- **AND** the managed worktree remains clean

#### Scenario: restart clears stall and re-runs Acceptance

- **GIVEN** a previously stalled in-memory record is gone after restart
- **AND** repository evidence still shows a complete unarchived Apply revision
- **WHEN** Conflux restarts
- **THEN** it does not reconstruct stalled state, PASS, or archive readiness
- **AND** it routes the change to Acceptance rather than Apply or archive

#### Scenario: stale blocker metadata loses routing authority

- **GIVEN** stored Acceptance blocker metadata no longer matches repository identity, worktree identity, active change state, or Apply revision ancestry
- **WHEN** Conflux reconciles runtime and workspace state
- **THEN** the metadata is invalidated
- **AND** reducer state does not remain stalled solely from stale metadata
- **AND** current repository evidence determines the safe next route

#### Scenario: bare GATED produces no stalled metadata

- **GIVEN** Acceptance emits bare GATED or legacy blocked compatibility input without valid structured blocker evidence
- **WHEN** runtime handles the result
- **THEN** it records no stalled blocker metadata
- **AND** it emits no stalled lifecycle transition
- **AND** bounded Acceptance protocol retry handles the result

#### Scenario: Acceptance and Apply blockers remain distinguishable

- **GIVEN** runtime evaluates Acceptance stall state or workspace-local Apply blocker evidence
- **WHEN** it determines explicit retry behavior
- **THEN** Acceptance state is identified by its phase and blocker category
- **AND** Apply-origin, legacy unknown-origin, or non-resumable workspace evidence is not assumed to be Acceptance-generated
- **AND** only a valid resumable Acceptance record authorizes Acceptance-only retry
