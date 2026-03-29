
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


### Requirement: git/sync must only run reconciliation when needed before push

The server git sync workflow MUST determine whether reconciliation is required by comparing the local branch SHA **before** the pull/fetch phase with the remote branch SHA **after** the pull/fetch phase. It MUST avoid invoking `resolve_command` only when the pre-pull local SHA already matches the remote SHA, indicating no new remote changes exist.

#### Scenario: local and remote branch tips already match (no new remote commits)

**Given** the server's local bare repo branch tip matches the remote branch tip before the pull phase
**When** `git/sync` performs the pull phase and then evaluates whether to run pre-push reconciliation
**Then** it MUST skip `resolve_command`
**And** it MUST return a successful sync response without attempting a push

#### Scenario: remote has new commits not yet fetched locally

**Given** the server's local bare repo branch tip does NOT match the remote branch tip before the pull phase (remote is ahead)
**When** `git/sync` performs the pull phase (fast-forwarding the local branch) and then evaluates whether to run pre-push reconciliation
**Then** it MUST run `resolve_command` before attempting push
**And** it MUST fail the sync if `resolve_command` exits non-zero

#### Scenario: remote branch does not yet exist for push comparison

**Given** the server has completed the pull phase for a project branch
**And** the remote SHA for push comparison is empty because the remote branch does not yet exist
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST NOT treat the branch as already synchronized
**And** it MUST continue with the existing resolve-before-push flow

#### Scenario: bare repo is newly cloned (first sync)

**Given** the local bare repo did not exist before this sync invocation and was freshly cloned
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST treat the pre-pull SHA as empty
**And** it MUST NOT skip `resolve_command`
