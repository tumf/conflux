## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Managed-worktree execution SHALL run `acceptance_command` after successful apply and before archive. Every configured frontend SHALL use the same verdict parsing, missing-verdict retry, history, restart, and stalled-hold behavior.

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry was active before process termination
- **AND** the managed workspace remains applied but unarchived
- **WHEN** Conflux restarts
- **THEN** it runs acceptance again from workspace file and Git state
- **AND** it does not require a generated retry checkpoint

### Requirement: Parallel apply runs in worktree

Every change-level apply command MUST run in the selected change's managed worktree. A base-repository or other non-managed execution directory MUST fail before the apply command starts.

#### Scenario: apply outside managed worktree fails

- **GIVEN** a change is selected for execution
- **AND** its apply directory is not its managed worktree
- **WHEN** apply dispatch is attempted
- **THEN** execution fails with the change ID and invalid directory
- **AND** the base repository is not mutated by apply

### Requirement: VCS Backend Auto-Detection

The sole execution path SHALL auto-detect Git when `--vcs` is absent or `auto`. Executable orchestration without a usable Git repository SHALL fail before orchestration side effects.

#### Scenario: No VCS available

- **WHEN** executable orchestration starts outside a usable Git repository
- **THEN** an actionable Git-repository error is displayed
- **AND** the exit code is non-zero
- **AND** no serial fallback starts

### Requirement: AI エージェントクラッシュリカバリー

ApplyまたはArchiveコマンドの異常終了時、managed-worktree executionは既存transport retry、history、fresh repository/handoff evaluation、permission、progress、stall、およびper-change active-run `max_iterations` contractを維持しなければならない（MUST）。

#### Scenario: Apply command failures exhaust one per-change active-run budget

- **GIVEN** `max_iterations` is `3`
- **AND** one change has Apply dispatches before and after an Acceptance FAIL-to-Apply cycle
- **WHEN** the third cumulative configured Apply dispatch completes
- **THEN** no fourth Apply command starts from CLI, TUI, or remote-controlled execution
- **AND** the typed `iteration_limit` diagnostic includes the exact cumulative count

### Requirement: Acceptance follow-up persistence failure must not override primary acceptance failure

Managed-worktree runtime MUST preserve a non-pass Acceptance verdict as the primary outcome when follow-up persistence degrades. Runtime MUST remain the sole writer of the latest numbered Acceptance follow-up, normalize repository-fixable findings into checkbox tasks, preserve external blocker metadata and recoverable unknown content, rehydrate altered runtime findings, and remove only runtime-owned follow-up state after PASS. Persistence degradation MUST remain supplemental unless the primary verdict is indeterminate.

#### Scenario: Follow-up persistence degrades without replacing FAIL

- **GIVEN** Acceptance returns FAIL with actionable findings
- **AND** the canonical tasks file cannot be updated safely
- **WHEN** runtime records the outcome
- **THEN** FAIL remains the primary diagnosis
- **AND** the original file remains unchanged with supplemental persistence diagnostics

### Requirement: Acceptance stalled retry evidence is workspace-local

Ordinary Acceptance retry bookkeeping during managed-worktree execution MUST remain in memory and MUST NOT use `.cflx/acceptance-state.json`, a worktree checkpoint, an Acceptance-origin marker, or an out-of-worktree durable stall record. A validated stalled hold MAY live only in process-local `OrchestratorState`; restart MUST clear it and require Acceptance again unless repository evidence proves archive or base integration.

#### Scenario: Restart clears process-local stall state

- **GIVEN** a prior process held a complete unarchived Apply revision as stalled
- **WHEN** a new process reconciles the managed workspace
- **THEN** it does not restore or infer Acceptance PASS from the old hold
- **AND** it runs Acceptance again

### Requirement: Acceptance retry safeguards are mode-independent

Managed-worktree execution MUST apply one blocker-validation, protocol-retry, finding-normalization, semantic-progress, mixed-blocker, stalled-state, reconciliation, migration, and explicit-retry policy across CLI, TUI, and remote-controlled entrypoints. Bare `gated` or legacy `blocked` input MUST use the fixed two-retry missing-verdict bound with an independent consecutive counter and MUST NOT consume Apply or explicit-CONTINUE budget.

#### Scenario: Every frontend uses the same bare GATED budget

- **GIVEN** equivalent managed-worktree Acceptance invocations emit the same bare GATED sequence through different frontends
- **WHEN** runtime applies protocol retry
- **THEN** each runs at most two Acceptance-only retries after the initial result
- **AND** each returns the same terminal protocol error on the third consecutive result

### Requirement: Acceptance follow-up rendering uses normalized finding scopes

Managed-worktree runtime MUST use the shared normalized finding representation for Acceptance follow-up state. Repository-fixable findings MUST affect task completion; external blockers MUST remain non-checkbox metadata; and every frontend MUST produce equivalent follow-up and prompt context for equivalent observations.

#### Scenario: Equivalent findings render identically

- **GIVEN** equivalent repository and external findings enter through different frontends
- **WHEN** runtime persists follow-up and builds the next Acceptance context
- **THEN** each produces the same repository task identities and external blocker metadata
- **AND** prior attempt history is not replayed

### Requirement: Acceptance finding reconciliation uses stable identity and monotonic completion

Managed-worktree runtime MUST reconcile repository-fixable Acceptance findings by stable identity rather than exact prose. Explicit codes take precedence; otherwise runtime MUST derive deterministic identity from normalized structural fields. Completed findings MUST remain complete during hydration and MAY reopen only when a new Acceptance FAIL explicitly reports the same identity.

#### Scenario: Missing reviewer code uses deterministic fallback identity

- **GIVEN** an Acceptance finding has no explicit stable code
- **WHEN** managed-worktree runtime normalizes it
- **THEN** all frontends derive the same identity from normalized structural fields
- **AND** prose-only changes do not change that identity

### Requirement: Acceptance execution creates no JSON checkpoint

Managed-worktree Acceptance MUST NOT create, read, update, or delete `.cflx/acceptance-state.json`. PASS MAY remain in memory only until archive handoff. After restart, incomplete archive work MUST run Acceptance again unless repository evidence already proves archive or base integration.

#### Scenario: Uninterrupted pass reaches archive without checkpoint

- **GIVEN** Apply completed and Acceptance runs in the same process
- **WHEN** Acceptance returns PASS
- **THEN** archive handoff proceeds for that accepted revision
- **AND** no JSON checkpoint or persisted PASS record is created

### Requirement: Acceptance repair state MUST separate actionable payload from retry identity

Managed-worktree runtime MUST keep the complete latest Acceptance finding payload separate from stable retry identities and semantic fingerprints. Updating comparison identities, semantic baselines, cycle counters, or retry state MUST NOT replace actionable evidence, required changes, or verification expectations.

#### Scenario: Retry identity cannot overwrite actionable payload

- **GIVEN** Acceptance records a detailed finding and runtime derives a comparison identity
- **WHEN** runtime updates retry identity and semantic baseline state
- **THEN** the complete finding remains unchanged
- **AND** the next Apply receives its evidence and verification expectations

### Requirement: Acceptance repair diff MUST cover declared finding work

Before rerunning Acceptance after repair Apply, managed-worktree runtime MUST compare the workspace delta from the finding's FAIL revision through the repair result. Every declared required-change and verification file MUST occur in that delta. Missing coverage MUST stop before Acceptance with an evidenced resumable `acceptance_remediation_mismatch` hold; unrelated progress MUST NOT satisfy coverage.

#### Scenario: Missing declared coverage stops before Acceptance

- **GIVEN** a structured finding declares implementation and verification files
- **AND** repair Apply does not change every declared file
- **WHEN** runtime validates the repair delta
- **THEN** Acceptance is not invoked
- **AND** diagnostics identify missing and unrelated files

### Requirement: Acceptance repair-stop diagnostics MUST be actionable and mode-independent

Managed-worktree execution MUST produce one structured diagnostic contract for `acceptance_remediation_mismatch` and `repeated_acceptance_finding` across all frontends. Diagnostics MUST include open findings, stable IDs, occurrence counts, relevant revisions, declared and actual files, coverage, unrelated changes, remediation evidence, stop reason, resumability, and next action. These holds MUST NOT prove completion, PASS, archive readiness, or merge eligibility.

#### Scenario: Equivalent repair failures expose equivalent evidence

- **GIVEN** equivalent findings and repair diffs enter through different frontends
- **WHEN** runtime detects remediation mismatch or a repeated ID
- **THEN** each chooses the same stop reason and resumability
- **AND** each exposes equivalent structured diagnostics without writing workflow evidence into the worktree

### Requirement: Acceptance command failures MUST use bounded Acceptance-only recovery

Managed-worktree execution MUST treat configured Acceptance command launch or execution failure as Acceptance-only recovery on the same applied clean workspace. It MUST allow at most two retries after the initial failure and return terminal error after the third consecutive failure. This counter MUST remain independent from protocol correction, explicit CONTINUE, FAIL-to-Apply cycles, and the outer cycle ceiling; retry MUST NOT rerun Apply or cleanup-review.

#### Scenario: Acceptance command recovers without rerunning Apply

- **GIVEN** Acceptance command fails after command-queue retry on an applied clean managed workspace
- **AND** the dedicated command-failure budget remains
- **WHEN** runtime handles the failure
- **THEN** it passes bounded diagnostics to the next Acceptance invocation
- **AND** it reruns only the configured Acceptance command
