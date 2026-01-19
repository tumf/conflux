## MODIFIED Requirements
### Requirement: Running Mode Dashboard

TUI SHALL display a dashboard-style UI in running mode.

#### Scenario: Display on processing completion
- **WHEN** all queued changes have been processed
- **THEN** the header status changes to "Ready"
- **AND** the status panel shows only progress and elapsed time
- **AND** TUI maintains display, allowing user to exit with `Ctrl+C`

#### Scenario: Running mode header shows processing count
- **GIVEN** the TUI is in running mode
- **WHEN** one or more changes are processing or archiving
- **THEN** the header shows "Running <count>" where <count> is the number of active operations

#### Scenario: Status line uses selected change progress
- **GIVEN** the TUI is in any mode
- **AND** one or more changes are selected (x)
- **WHEN** the status panel is rendered
- **THEN** the progress bar reflects the total tasks and completed tasks of selected changes
- **AND** the status line shows only the progress bar and elapsed time

#### Scenario: Status line shows accumulated running time
- **GIVEN** the TUI has been in running mode at least once
- **WHEN** the status panel is rendered in Ready or Stopped mode
- **THEN** the elapsed time shows the accumulated running duration

#### Scenario: Header hides status in stopped and error modes
- **GIVEN** the TUI is in stopped or error mode
- **WHEN** the header is rendered
- **THEN** the header shows no status label
