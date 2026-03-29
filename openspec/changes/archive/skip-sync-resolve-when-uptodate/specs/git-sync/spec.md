## MODIFIED Requirements

### Requirement: git/sync must only run reconciliation when needed before push

The server git sync workflow MUST determine whether reconciliation is required after refreshing remote state and MUST avoid invoking `resolve_command` when the local branch is already synchronized with the target remote branch.

#### Scenario: local and remote branch tips already match

**Given** the server has completed the pull phase for a project branch
**And** the computed local branch SHA equals the current remote branch SHA
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST skip `resolve_command`
**And** it MUST return a successful sync response without attempting a push

#### Scenario: local and remote branch tips differ

**Given** the server has completed the pull phase for a project branch
**And** the computed local branch SHA differs from the current remote branch SHA
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST run `resolve_command` before attempting push
**And** it MUST fail the sync if `resolve_command` exits non-zero

#### Scenario: remote branch does not yet exist for push comparison

**Given** the server has completed the pull phase for a project branch
**And** the remote SHA for push comparison is empty because the remote branch does not yet exist
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST NOT treat the branch as already synchronized
**And** it MUST continue with the existing resolve-before-push flow
