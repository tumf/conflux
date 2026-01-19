## MODIFIED Requirements
### Requirement: Processing Item Spinner Animation

The TUI SHALL display an animated spinner next to items with `Processing` or `Accepting` status in running mode.

#### Scenario: Spinner display for processing items
- **WHEN** TUI is in running mode
- **AND** an item has `QueueStatus::Processing`
- **THEN** an animated spinner character is displayed before the progress percentage
- **AND** the spinner cycles through Braille dot characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
- **AND** the display format is "⠋ [XX%]" where ⠋ is the current spinner character

#### Scenario: Spinner display for accepting items
- **WHEN** TUI is in running mode
- **AND** an item has `QueueStatus::Accepting`
- **THEN** an animated spinner character is displayed before the progress percentage
- **AND** the spinner cycles through Braille dot characters (⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏)
- **AND** the display format is "⠋ [accepting]" where ⠋ is the current spinner character

#### Scenario: Spinner animation timing
- **WHEN** TUI is rendering in running mode
- **THEN** the spinner character advances to the next frame approximately every 100ms
- **AND** the spinner cycles continuously until processing completes

#### Scenario: Spinner not shown for non-processing items
- **WHEN** TUI is in running mode
- **AND** an item has status other than `Processing` or `Accepting` (Queued, Completed, Error)
- **THEN** no spinner is displayed for that item
