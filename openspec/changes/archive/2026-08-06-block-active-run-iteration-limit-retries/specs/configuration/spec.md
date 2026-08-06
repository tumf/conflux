## MODIFIED Requirements

### Requirement: Max Iterations Configuration

The configured `max_iterations` value SHALL be enforced by one per-change active-run cumulative Apply-dispatch counter shared by CLI, TUI, and remote-controlled worktree execution. Explicit retry, queue intent, and scheduler wake-up inside that active run SHALL NOT reset or replace its counter. A later scheduler boundary admitted after the prior boundary closes SHALL create fresh active-run state and a fresh counter from current workspace and Git evidence. A fresh process SHALL do the same; no log, API snapshot, local-state file, or durable retry artifact SHALL restore the prior counter.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.cflx.jsonc` contains `"max_iterations": 100`
- **WHEN** one change enters Apply through any executable frontend
- **THEN** that change starts no more than 100 configured Apply-agent dispatches during the active run
- **AND** another change owns an independent total

#### Scenario: Retry cannot replenish the active counter

- **GIVEN** one change has exhausted its Apply-dispatch counter in an active scheduler boundary
- **WHEN** an operator requests retry, queue addition, or scheduler wake-up for that change before the boundary closes
- **THEN** the active counter remains exhausted and unchanged
- **AND** no replacement Apply budget is created for that boundary

#### Scenario: Later scheduler boundary receives a fresh counter

- **GIVEN** a scheduler boundary exhausted one change's Apply-dispatch counter and then closed
- **AND** the preserved workspace still requires processing
- **WHEN** a later scheduler boundary is explicitly admitted in the same process
- **THEN** routing is re-derived from current workspace and Git evidence
- **AND** the later boundary owns a fresh per-change counter
- **AND** it is not a wake-up of the closed scheduler

### Requirement: Iteration Limit Finish Status

When one change exhausts its positive per-change Apply-dispatch budget, the sole budget owner SHALL record a typed `iteration_limit` outcome with the change ID, exact cumulative attempts, and configured maximum. CLI, TUI, and remote-controlled worktree run boundaries SHALL preserve that evidence through their existing sole finish-hook attempt. In a command-capable boundary, shared operator admission SHALL treat the record as a retry gate only while the owning scheduler task reports live through `RunSchedulerPort::is_running()`. The existing task handle SHALL remain live through the finish-hook attempt and terminal publication; task exit SHALL retire the gate without clearing the record or adding a separate closing barrier. A later admission SHALL NOT notify the exited scheduler.

#### Scenario: All frontends preserve typed iteration-limit ownership

- **GIVEN** the same per-change budget is exhausted through CLI, TUI, or remote-controlled execution
- **WHEN** the outcome crosses the respective run boundary
- **THEN** each boundary preserves `iteration_limit`
- **AND** the sole `on_finish` owner receives the exact cumulative Apply count
- **AND** no inner and outer owner invoke `on_finish` twice

#### Scenario: Active evidence survives until finish-hook ownership completes

- **GIVEN** an active boundary recorded typed iteration-limit evidence
- **WHEN** its finish hook is still pending or running
- **THEN** the record remains available to the finish-hook owner
- **AND** command-capable retry admission still treats the owning boundary as limited

#### Scenario: Hook failure does not make the gate permanent

- **GIVEN** the finish-hook owner observed typed iteration-limit evidence
- **WHEN** the hook command returns an error
- **THEN** the boundary reports the hook error through existing behavior
- **AND** scheduler-task exit still retires the active retry gate
- **AND** no durable blocker is created

#### Scenario: Scheduler-task liveness closes the admission race

- **GIVEN** the limited boundary's scheduler task remains live through `on_finish` and terminal publication
- **WHEN** retry races with the task tail
- **THEN** admission is refused while `RunSchedulerPort::is_running()` is true
- **OR** retry is admitted only after task exit makes the old scheduler unavailable
- **AND** no accepted retry is notified into the exhausted or closing scheduler
