# Spec Delta: TUI Elapsed Time Tracking

## ADDED Requirements

### Requirement: Parallel Execution Elapsed Time Tracking

The TUI SHALL track elapsed time from the start of apply operation through archive completion in parallel execution mode, ensuring consistent time display across all execution states.

#### Scenario: ApplyStarted event sets started_at

- **GIVEN** a change is in NotQueued or Queued status
- **AND** the change's `started_at` field is `None`
- **WHEN** an `ApplyStarted` event is received for that change
- **THEN** the change's `started_at` field SHALL be set to the current time
- **AND** the change's `queue_status` SHALL be set to `Processing`
- **AND** a log entry SHALL be added with "Apply started: {change_id}"

#### Scenario: ApplyStarted preserves existing started_at

- **GIVEN** a change has `started_at` already set
- **WHEN** an `ApplyStarted` event is received for that change
- **THEN** the change's `started_at` field SHALL NOT be modified
- **AND** the change's `queue_status` SHALL be updated to `Processing`

#### Scenario: ArchiveStarted preserves started_at from ApplyStarted

- **GIVEN** a change has `started_at` set by `ApplyStarted`
- **WHEN** an `ArchiveStarted` event is received for that change
- **THEN** the change's `started_at` field SHALL be preserved (not overwritten)
- **AND** the change's `queue_status` SHALL be set to `Archiving`
- **AND** elapsed time display SHALL continue from the original start time

#### Scenario: ArchiveStarted sets started_at as fallback

- **GIVEN** a change has `started_at` set to `None`
- **WHEN** an `ArchiveStarted` event is received for that change
- **THEN** the change's `started_at` field SHALL be set to the current time
- **AND** the change's `queue_status` SHALL be set to `Archiving`
- **AND** elapsed time display SHALL start from archive start time

**Rationale:** This provides a fallback mechanism for unexpected event ordering or event loss scenarios.

#### Scenario: Parallel execution displays elapsed time during archiving

- **GIVEN** a change in parallel execution with `started_at` set by `ApplyStarted`
- **WHEN** the change transitions to `Archiving` status
- **THEN** the TUI SHALL display elapsed time calculated from `started_at`
- **AND** the elapsed time SHALL NOT display "--"
- **AND** the elapsed time SHALL continue to increment until archive completion

#### Scenario: Parallel execution records final elapsed time

- **GIVEN** a change in parallel execution with `started_at` set
- **WHEN** a `ChangeArchived` event is received
- **THEN** the change's `elapsed_time` field SHALL be set to the duration from `started_at` to now
- **AND** the TUI SHALL display the fixed elapsed time after archive completion
- **AND** the displayed time SHALL represent the total duration from apply start to archive end

## MODIFIED Requirements

None. This change adds new event handling without modifying existing requirements.

## REMOVED Requirements

None. All existing requirements remain valid.

## Cross-References

### Related Capabilities

- **tui-architecture**: Uses existing event handling infrastructure (`src/tui/state/events.rs`)
- **parallel-execution**: Responds to parallel execution events (`ApplyStarted`, `ArchiveStarted`)
- **cli**: Maintains existing elapsed time recording behavior for interrupted changes

### Implementation Notes

**Event Handler Location:**
- `src/tui/state/events.rs`: Add `ApplyStarted` case to `handle_orchestrator_event`
- `src/tui/state/events.rs`: Update `ArchiveStarted` case with fallback logic

**State Structure:**
- `src/tui/state/change.rs`: Uses existing `ChangeState` fields (`started_at`, `elapsed_time`)
- No structural changes required

**Display Logic:**
- `src/tui/render.rs`: Uses existing elapsed time display logic (lines 361-367)
- No changes required; will automatically work once `started_at` is set

**Event Source:**
- `src/parallel/executor.rs`: Already emits `ApplyStarted` event (line 285)
- `src/parallel/mod.rs`: Already emits `ArchiveStarted` event (line 728)

### Backward Compatibility

- ✅ Serial execution flow unchanged (`ProcessingStarted` continues to set `started_at`)
- ✅ Existing tests pass without modification
- ✅ No configuration changes required
- ✅ No API changes to public interfaces

### Testing Strategy

**Unit Tests:**
- Test `ApplyStarted` sets `started_at` when `None`
- Test `ApplyStarted` preserves existing `started_at`
- Test `ArchiveStarted` preserves `started_at` from `ApplyStarted`
- Test `ArchiveStarted` sets `started_at` as fallback when `None`
- Test complete parallel flow: `ApplyStarted` → `ArchiveStarted` → `ChangeArchived`

**Integration Tests:**
- Verify elapsed time displays during parallel execution
- Verify serial execution behavior unchanged
- Verify edge cases (stop/resume, errors)
