## MODIFIED Requirements

### Requirement: Merge Deferred State Separation

The parallel executor SHALL treat a resolve attempt as complete when the target change has been integrated into the base branch, even if the integration happened via fast-forward and did not create a merge commit.

`Missing merge commits for change_ids` SHALL NOT be used for changes that are already integrated into the base branch via fast-forward.

#### Scenario: Fast-forward resolve is accepted as merged

**Given** a change has completed archive successfully in parallel mode
**And** the resolve command merges the change into the base branch via fast-forward
**When** post-resolve verification runs
**Then** the change is treated as successfully merged
**And** the system does not enqueue another resolve retry for that change

#### Scenario: Missing merge commits only applies to truly incomplete merge state

**Given** a change has completed archive successfully in parallel mode
**And** post-resolve verification finds no required merge commit evidence
**And** the change is not integrated into the base branch
**When** the system prepares the next resolve attempt
**Then** the resolve context may include `Missing merge commits for change_ids`
**And** the listed change_ids exclude fast-forward-integrated changes
