## MODIFIED Requirements

### Requirement: Server mode dashboard log pane must adapt to project selection state

The server-mode dashboard SHALL display orchestration-wide logs in the Logs pane when no project is selected.
The server-mode dashboard SHALL display only the selected project's logs in the Logs pane when a project is selected.
This behavior SHALL apply consistently in both desktop and mobile dashboard layouts.

#### Scenario: No project selected shows orchestration-wide logs

**Given** the dashboard has received log entries for one or more projects
**And** no project is currently selected
**When** the user opens the Logs pane
**Then** the dashboard displays the aggregated orchestration-wide log stream instead of an empty-state prompt

#### Scenario: Selected project shows project-scoped logs

**Given** the dashboard has received log entries for multiple projects
**And** a project is currently selected
**When** the user opens the Logs pane
**Then** the dashboard displays only the log entries associated with the selected project

### Requirement: Server mode dashboard project cards must support selection toggle

The server-mode dashboard SHALL select a project when the user activates an unselected project card.
The server-mode dashboard SHALL clear the current project selection when the user activates the already selected project card again.
The dashboard SHALL apply the same toggle behavior for pointer activation and keyboard activation using Enter or Space.

#### Scenario: Clicking an unselected project selects it

**Given** no project is selected
**When** the user clicks a project card
**Then** that project becomes the selected project

#### Scenario: Clicking the selected project clears selection

**Given** a project is selected
**When** the user clicks the same project card again
**Then** the selected project is cleared

#### Scenario: Keyboard activation toggles project selection

**Given** a project card has keyboard focus
**When** the user presses Enter or Space on the card
**Then** the dashboard applies the same selection toggle behavior as a pointer click
