## REMOVED Requirements

### Requirement: Overview Dashboard Display

The standalone multi-project overview dashboard is removed.

#### Scenario: No multi-project overview is shipped

**Given**: A release build
**When**: Packaged assets are inspected
**Then**: No standalone multi-project dashboard application is present

### Requirement: Statistics Summary Display

Multi-project server statistics presentation is removed.

#### Scenario: No server statistics page is shipped

**Given**: A release build
**When**: User-facing dashboard surfaces are inspected
**Then**: No aggregate server statistics page is present

### Requirement: Activity Timeline Display

The multi-project server activity timeline is removed.

#### Scenario: No server timeline is shipped

**Given**: A release build
**When**: User-facing dashboard surfaces are inspected
**Then**: No multi-project activity timeline is present
