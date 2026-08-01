## MODIFIED Requirements

### Requirement: Acceptance stalled retry evidence is workspace-local

Ordinary Acceptance retry bookkeeping during an active serial or parallel run MUST remain in memory and MUST NOT use `.cflx/acceptance-state.json` or a worktree checkpoint. Acceptance MUST NOT create an Acceptance-origin `APPLY_BLOCKED/marker.md` or another change-directory artifact.

A validated Acceptance stalled hold MUST be stored in the in-memory `OrchestratorState` only. It MUST NOT be persisted to `~/.local/state/cflx/acceptance-stalls/` or any other out-of-worktree durable location. The in-memory state binds change ID, blocker category, evidence, next action, and resumability for the lifetime of the current process.

In-memory state MAY control ordinary dispatch suppression, stalled presentation, explicit retry eligibility, and Acceptance resume phase. It MUST NOT prove implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration. Process restart MUST clear all in-memory stall state. When repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

#### Scenario: stalled hold is process-lifetime only

- **GIVEN** Acceptance records a validated resumable external blocker for a complete Apply revision
- **AND** the managed worktree is clean
- **WHEN** the current Conflux process displays the stalled status
- **THEN** ordinary dispatch starts neither Apply, Acceptance, nor archive
- **AND** the worktree remains clean and the Apply commit remains unchanged
- **AND** no stall file is written under `~/.local/state/cflx/`

#### Scenario: restart clears stall and re-runs acceptance

- **GIVEN** a change was stalled in a previous Conflux process
- **AND** the worktree contains a complete unarchived Apply revision
- **WHEN** a new Conflux process starts and reconciles workspace state
- **THEN** the stalled status is not restored
- **AND** Conflux runs Acceptance again
- **AND** it does not infer prior PASS, enter archive, or rerun Apply solely from missing stall state

#### Scenario: stale stall files are ignored, not consulted or removed

- **GIVEN** files exist under `~/.local/state/cflx/acceptance-stalls/` from a previous version
- **WHEN** a new Conflux process starts and dispatches the same change
- **THEN** no stall file is read and none controls routing
- **AND** the files are left in place so a concurrent older process keeps its own holds
- **AND** no managed worktree is mutated

#### Scenario: explicit retry resumes Acceptance from in-memory hold

- **GIVEN** a valid resumable Acceptance stall exists in the current in-memory state
- **AND** the Apply revision matches
- **WHEN** an operator explicitly retries it
- **THEN** runtime prepares and starts Acceptance without rerunning Apply
- **AND** the in-memory hold is consumed across a successful dispatch-preparation boundary
- **AND** preparation failure retains the blocker evidence and does not dispatch ambiguous work

### Requirement: Acceptance execution creates no JSON checkpoint

Serial and parallel Acceptance execution MUST NOT create, read, update, or delete `.cflx/acceptance-state.json`. Acceptance PASS for an active run MAY be held in memory only until archive handoff. After restart, incomplete archive work MUST be accepted again unless repository evidence already proves archive or base integration.

No out-of-worktree Acceptance stall record exists. In-memory stall state MAY represent a validated temporary external hold bound to the current process lifetime and MUST NOT survive restart.

#### Scenario: uninterrupted pass reaches archive without checkpoint

- **GIVEN** Apply completed and Acceptance runs in the same orchestration process
- **WHEN** Acceptance returns PASS
- **THEN** archive handoff proceeds for that accepted revision
- **AND** neither `.cflx/acceptance-state.json` nor a persisted PASS record exists

#### Scenario: in-memory stall cannot substitute for PASS

- **GIVEN** an in-memory stall state exists
- **WHEN** Conflux evaluates archive readiness
- **THEN** the in-memory state cannot prove PASS or authorize archive
- **AND** Acceptance must pass for the current revision through the normal execution path

#### Scenario: runtime metadata cannot dirty post-archive worktree

- **GIVEN** Acceptance passes and archive artifacts are committed
- **WHEN** post-archive merge verification runs
- **THEN** no Acceptance runtime-state cleanup mutates the managed worktree
- **AND** no manual `MergeWait` is produced solely by runtime stall metadata

#### Scenario: genuine dirty evidence remains a blocker

- **GIVEN** archive artifacts are valid
- **AND** an unrelated user file remains modified
- **WHEN** post-archive merge verification runs
- **THEN** the unrelated dirty worktree remains concrete manual blocker evidence
- **AND** in-memory stall state does not suppress the deferral
