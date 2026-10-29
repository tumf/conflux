## ADDED Requirements

### Requirement: Refresh proposal key hint

The Changes panel SHALL display a `p: refresh proposal` key hint only when the focused row's `actions.refresh_proposal` eligibility is allowed. Pressing `p` on such a row SHALL run the shared refresh preview with the run's configured base branch as `source_ref` and open a confirmation popup showing the source ref, source commit, candidate head, blob identities, and added/removed line counts or a no-op notice; `Enter` or `y` SHALL submit the apply with the previewed expectations through the shared operator transaction, and `Esc` SHALL close the popup without submitting anything. On an ineligible row `p` SHALL be ignored and no hint SHALL be shown. The popup is presentation state only and SHALL NOT be used as workflow-control input.

#### Scenario: Eligible row advertises and confirms a refresh

- **GIVEN** the cursor is on an `error` row with a managed worktree and no live process
- **WHEN** the key hints are rendered and `p` is pressed
- **THEN** the hints include `p: refresh proposal`
- **AND** a popup shows the preview facts for the configured base branch
- **AND** `Enter` submits exactly one refresh-proposal command with the previewed expectations

#### Scenario: Ineligible row shows no hint and ignores the key

- **GIVEN** the cursor is on an `applying`, `queued`, `merge wait`, or `archived` row
- **WHEN** the key hints are rendered and `p` is pressed
- **THEN** the hints do not include `p: refresh proposal`
- **AND** no popup opens and no command is submitted

#### Scenario: Cancel submits nothing

- **GIVEN** the refresh confirmation popup is open
- **WHEN** `Esc` is pressed
- **THEN** the popup closes
- **AND** no command, mark, queue, or Git effect occurs
