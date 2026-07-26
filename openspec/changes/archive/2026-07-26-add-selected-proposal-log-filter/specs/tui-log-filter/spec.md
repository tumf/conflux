## ADDED Requirements

### Requirement: Selected Proposal Log Filtering

The TUI SHALL provide a presentation-only filter that limits the Changes-view Logs panel to entries structurally associated with the proposal under the cursor. The filter SHALL default to disabled, SHALL use exact `LogEntry.change_id` association, and SHALL NOT mutate buffered logs or influence workflow-control behavior.

#### Scenario: Filter defaults to all logs

- **GIVEN** a newly initialized TUI contains logs for multiple proposals and global orchestration
- **WHEN** the Changes-view Logs panel is rendered before the user toggles filtering
- **THEN** the filter SHALL be disabled
- **AND** all buffered entries SHALL remain eligible for display under the existing scrolling behavior

#### Scenario: Filter shows only the cursor proposal

- **GIVEN** the cursor is on proposal `alpha`
- **AND** buffered entries include `change_id` values for `alpha`, `beta`, and no proposal
- **WHEN** the user presses `f` in the Changes view
- **THEN** the Logs panel SHALL show only entries whose `change_id` is exactly `alpha`
- **AND** the proposal's execution mark and workflow state SHALL remain unchanged

#### Scenario: Active filter follows cursor movement

- **GIVEN** selected-proposal filtering is enabled for proposal `alpha`
- **WHEN** the cursor moves to proposal `beta`
- **THEN** the filter target SHALL immediately become `beta`
- **AND** the Logs panel SHALL return to the newest matching position with auto-scroll enabled

#### Scenario: Disabling the filter restores buffered logs

- **GIVEN** selected-proposal filtering is enabled
- **WHEN** the user presses `f` again in the Changes view
- **THEN** all entries still present in the bounded log buffer SHALL become eligible for display
- **AND** no log entry SHALL have been deleted or rewritten by filtering

#### Scenario: Unidentified remote and global logs are excluded

- **GIVEN** selected-proposal filtering is enabled
- **AND** an entry identifies only a project or has no `change_id`
- **WHEN** the Logs panel is rendered
- **THEN** that entry SHALL NOT be displayed as belonging to the cursor proposal
- **AND** the TUI SHALL NOT infer a proposal ID from message text

#### Scenario: Filter target has no matching logs

- **GIVEN** selected-proposal filtering is enabled
- **AND** the cursor proposal has no matching entries
- **WHEN** the Logs panel is rendered or scrolled
- **THEN** the panel SHALL render safely with no matching log content
- **AND** wrapping, counts, ranges, and scroll bounds SHALL be calculated from the empty filtered set

### Requirement: Selected Proposal Log Filter Hint

The visible Changes-view Logs panel SHALL expose the selected-proposal filter key and current state. The interface MAY shorten the wording for constrained widths but SHALL preserve the `f` key and off/on meaning.

#### Scenario: Filter hint shows disabled state

- **GIVEN** the Changes-view Logs panel is visible
- **AND** selected-proposal filtering is disabled
- **WHEN** the panel title or adjacent help is rendered
- **THEN** the visible UI SHALL identify `f` as the filter toggle
- **AND** it SHALL indicate that filtering is off

#### Scenario: Filter hint shows active target

- **GIVEN** the Changes-view Logs panel is visible
- **AND** selected-proposal filtering is enabled for proposal `alpha`
- **WHEN** the panel title or adjacent help is rendered
- **THEN** the visible UI SHALL identify `f` as the filter toggle
- **AND** it SHALL indicate that filtering is on for `alpha`, or use an equivalent compact active-state label when width is constrained

### Requirement: Structured Proposal Identity for TUI Logs

TUI event handlers SHALL attach `LogEntry.change_id` to proposal-specific lifecycle, completion, skip, stop, and error entries whenever the source event carries a proposal ID. Global orchestration entries SHALL remain unscoped, and proposal identity SHALL NOT be derived by parsing human-readable messages.

#### Scenario: Proposal-specific lifecycle entries carry identity

- **GIVEN** a TUI event reports a start, completion, failure, skip, or stop for proposal `alpha`
- **WHEN** its user-visible `LogEntry` is created
- **THEN** the entry SHALL carry `change_id` equal to `alpha`
- **AND** filtering for `alpha` SHALL not depend on the message wording

#### Scenario: Global entries remain unscoped

- **GIVEN** a TUI event describes orchestration as a whole rather than one proposal
- **WHEN** its user-visible `LogEntry` is created
- **THEN** the entry SHALL have no proposal `change_id`
- **AND** selected-proposal filtering SHALL exclude it
