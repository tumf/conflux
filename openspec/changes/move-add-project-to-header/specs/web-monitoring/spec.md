## MODIFIED Requirements

### Requirement: Add Project Button Placement

The dashboard SHALL display the Add Project button as a compact `+` icon in the PROJECTS header row, right-aligned, instead of a full-width button within the project list.

#### Scenario: Desktop sidebar Add Project button

- **GIVEN** the user views the desktop sidebar
- **WHEN** the PROJECTS header is rendered
- **THEN** a `+` icon button is displayed at the right end of the header row
- **AND** clicking it opens the Add Project dialog
- **AND** hovering changes the icon color to indigo (#6366f1)

#### Scenario: Mobile Add Project access

- **GIVEN** the user views the mobile layout
- **WHEN** the projects tab is active
- **THEN** the user can access the Add Project function
