## Requirements

### Requirement: Overview Dashboard Display

The dashboard SHALL display an orchestration overview in the `<main>` area when no project is selected.

#### Scenario: No project selected shows dashboard

- **GIVEN** the user has not selected any project
- **WHEN** the dashboard is loaded
- **THEN** the `<main>` area displays an overview dashboard with aggregate statistics across all projects

#### Scenario: Project selected hides dashboard

- **GIVEN** the overview dashboard is displayed
- **WHEN** the user selects a project
- **THEN** the dashboard is replaced by the project detail view

### Requirement: Statistics Summary Display

The overview dashboard SHALL display aggregate processing statistics fetched from `GET /api/v1/stats/overview`.

#### Scenario: Success and failure counts shown

- **GIVEN** the overview dashboard is displayed
- **WHEN** statistics data is loaded from the server
- **THEN** the dashboard shows total success count, total failure count, and average duration per operation type

### Requirement: Activity Timeline Display

The overview dashboard SHALL display a timeline of recent change events.

#### Scenario: Recent events listed

- **GIVEN** the overview dashboard is displayed
- **WHEN** statistics data is loaded
- **THEN** the dashboard shows the most recent change events with project name, change ID, operation, result, and timestamp
