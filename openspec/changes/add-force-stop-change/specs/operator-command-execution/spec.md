## ADDED Requirements

### Requirement: Target-scoped force-stop transaction

The shared operator application transaction MUST provide `ForceStopChange` for exactly one named change. It MUST validate action eligibility from the admitted authoritative revision before side effects, bypass the graceful SIGTERM escalation window, immediately send SIGKILL to only the managed process group owned by that change, wait for confirmed termination and process reaping, atomically clear that change's queue admission intent and execution mark, and settle it as stopped without rolling back completed worktree effects. The transaction MUST preserve every unrelated change's processes, marks, queue intent, execution identity, subscription binding, and progress, and MUST NOT change process-wide run mode, scheduler state, or stop state.

A queued or dependency-blocked admitted target without a live process MUST be eligible for dequeue-only settlement with its execution mark revoked. Applying, accepting, rejecting, archiving, and resolving targets are eligible only while they own live managed activity. Merge-wait, resolve-wait without a live resolver, terminal, rejected, unknown, and unadmitted targets MUST be ineligible with typed reasons. The transaction MUST use the managed ownership graph rather than unscoped PID lookup. Stale revision MUST fail before signalling. Exact idempotent replay MUST return the original result without repeating cancellation or affecting a later execution episode.

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

#### Scenario: Queued target is dequeued without signalling another process

- **GIVEN** `alpha` is admitted and queued or dependency-blocked without a live managed process
- **WHEN** the operator force-stops `alpha`
- **THEN** `alpha` receives dequeue-only stopped settlement
- **AND** its queue intent and execution mark are cleared atomically
- **AND** no process belonging to another change is signalled

#### Scenario: Later mark settlement does not re-admit the target

- **GIVEN** `alpha` was marked and active before targeted force-stop
- **WHEN** its force-stop settles and the owner's later mark settlement runs
- **THEN** `alpha` remains unmarked and not queued
- **AND** no new execution episode is created without a new operator mark

#### Scenario: Ineligible target changes nothing

- **GIVEN** `alpha` is unknown, terminal, rejected, unadmitted, in merge-wait, or in resolve-wait without a live resolver
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

#### Scenario: Stop notification and wait remain truthful

- **GIVEN** `alpha` has a proposal subscription and an observing client wait
- **WHEN** targeted force-stop settles its current execution episode
- **THEN** the subscription emits the ordinary terminal `stopped` event for that exact execution ID
- **AND** client wait releases with `change_requires_action` and exit status 27
