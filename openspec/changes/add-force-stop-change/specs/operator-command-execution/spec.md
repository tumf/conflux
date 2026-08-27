## ADDED Requirements

### Requirement: Target-scoped force-stop transaction

The shared operator application transaction MUST provide `ForceStopChange` for exactly one named change. It MUST validate action eligibility from the admitted authoritative revision before side effects, immediately cancel and terminate only managed activity owned by that change, wait for confirmed termination and process reaping, clear that change's queue admission intent, and settle it as stopped without rolling back completed worktree effects. The transaction MUST preserve every unrelated change's processes, marks, queue intent, execution identity, subscription binding, and progress, and MUST NOT change process-wide run mode, scheduler state, or stop state.

The transaction MUST use the managed ownership graph rather than unscoped PID lookup. Unsupported phase, unknown target, terminal target, already-quiescent target, and stale revision MUST settle as typed no-op or failure without cross-target cancellation. Exact idempotent replay MUST return the original result without repeating cancellation.

#### Scenario: One concurrent change is killed

- **GIVEN** changes `alpha` and `beta` have active managed phase processes
- **AND** `alpha` publishes `force_stop_change` as allowed
- **WHEN** the operator force-stops `alpha`
- **THEN** only `alpha`'s managed processes are cancelled, terminated, and reaped
- **AND** `alpha` settles stopped and is no longer queued
- **AND** `beta` keeps its process, marks, queue intent, execution identity, and progress
- **AND** the scheduler and process-wide run mode remain unchanged

#### Scenario: Completed effects are preserved

- **GIVEN** `alpha` completed Apply and is active in a later managed phase
- **WHEN** the operator force-stops `alpha`
- **THEN** the later phase is terminated and reaped before settlement
- **AND** the completed Apply worktree commit remains present
- **AND** the typed result reports `effects_rolled_back: false`

#### Scenario: Ineligible target changes nothing

- **GIVEN** `alpha` is unknown, terminal, unsupported, or already quiescent
- **WHEN** `ForceStopChange` addresses `alpha`
- **THEN** the command returns a typed no-op or failure
- **AND** no managed process, mark, queue intent, execution identity, scheduler state, or process-wide mode changes

#### Scenario: Stale request has no termination side effect

- **GIVEN** the caller's expected revision is stale
- **WHEN** it requests `ForceStopChange` for `alpha`
- **THEN** revision validation fails before cancellation
- **AND** neither `alpha` nor any unrelated change is signalled

#### Scenario: Exact replay does not kill a later episode

- **GIVEN** a `ForceStopChange` command for `alpha` settled successfully
- **AND** `alpha` later starts a new execution episode
- **WHEN** the exact original command is replayed with its idempotency key
- **THEN** the original settled result is returned unchanged
- **AND** the new execution episode is not cancelled
