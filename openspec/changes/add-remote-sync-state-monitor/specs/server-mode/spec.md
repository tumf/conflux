## ADDED Requirements

### Requirement: Periodic remote sync-state monitoring

`cflx server` MUST periodically evaluate each registered project's synchronization state against its configured remote branch while the server is running.

The monitoring loop MUST classify the project as one of `up_to_date`, `ahead`, `behind`, `diverged`, or `unknown`.

#### Scenario: project is up to date
- **GIVEN** a registered project whose managed local branch matches the configured remote branch
- **WHEN** the server monitoring interval runs
- **THEN** the server records `sync_state = up_to_date`
- **AND** the server records `ahead_count = 0`
- **AND** the server records `behind_count = 0`

#### Scenario: remote branch is ahead
- **GIVEN** a registered project whose remote branch has commits not present in the managed local branch
- **WHEN** the server monitoring interval runs
- **THEN** the server records `sync_state = behind`
- **AND** the server records `behind_count > 0`
- **AND** the server marks the project as needing sync attention

#### Scenario: local branch is ahead
- **GIVEN** a registered project whose managed local branch has commits not present on the remote branch
- **WHEN** the server monitoring interval runs
- **THEN** the server records `sync_state = ahead`
- **AND** the server records `ahead_count > 0`
- **AND** the server marks the project as needing sync attention

#### Scenario: local and remote branches diverged
- **GIVEN** a registered project whose managed local branch and remote branch both contain unique commits
- **WHEN** the server monitoring interval runs
- **THEN** the server records `sync_state = diverged`
- **AND** the server records `ahead_count > 0`
- **AND** the server records `behind_count > 0`
- **AND** the server marks the project as needing sync attention

#### Scenario: remote check fails
- **GIVEN** the server cannot refresh remote comparison data for a registered project
- **WHEN** the monitoring interval runs
- **THEN** the server records `sync_state = unknown`
- **AND** the server records the check failure details and timestamp

### Requirement: Monitoring must be non-invasive

Periodic sync-state monitoring MUST NOT trigger reconciliation or synchronization side effects by itself.

#### Scenario: monitoring does not invoke sync
- **GIVEN** the server monitoring interval runs for a registered project
- **WHEN** the project is classified as `behind` or `diverged`
- **THEN** the server does not invoke `POST /api/v1/projects/{id}/git/sync`
- **AND** the server does not execute `resolve_command`
- **AND** only sync-state metadata is updated
